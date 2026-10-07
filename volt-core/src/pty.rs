use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{ErrorKind, Read};
#[cfg(unix)]
use std::os::{
    fd::{AsRawFd, FromRawFd},
    unix::net::UnixStream,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use vte::Parser;

use crate::diagnostics::{Counter, PerfStats, Stage};
use crate::events::CoreEvent;
use crate::performer::Performer;

fn send_event(tx: &mpsc::UnboundedSender<CoreEvent>, event: CoreEvent) {
    if let Err(err) = tx.send(event) {
        eprintln!("volt-core: failed to send core event: {err}");
    }
}

/// How many bytes to parse per Mutex<Performer> acquisition.
/// Smaller = lower latency / less contention with UI reads.
/// Larger = fewer lock round-trips under heavy throughput.
const PARSE_LOCK_CHUNK_BYTES: usize = 16 * 1024;

/// Bounded depth of the drain → parse queue. Kernel PTY reads are often only
/// a few KB, so the depth must be generous or the drain thread blocks on the
/// queue (stalling the writing program) whenever the parser is mid-batch.
const PIPELINE_QUEUE_BUFFERS: usize = 256;

/// Stop coalescing queued buffers into a parse batch beyond this size.
const COALESCE_LIMIT_BYTES: usize = 1024 * 1024;

/// Persistent macOS readiness registration. Rebuilding poll's descriptor wait
/// on every small PTY read is expensive; kqueue retains both registrations.
/// Level-triggered reads preserve unread tail bytes, and shutdown stays wakeable.
#[cfg(target_os = "macos")]
struct PtyReadiness {
    queue: std::os::fd::OwnedFd,
    shutdown_fd: std::os::fd::RawFd,
}

#[cfg(target_os = "macos")]
impl PtyReadiness {
    fn new(reader: std::os::fd::RawFd, shutdown: std::os::fd::RawFd) -> std::io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // Own immediately so any later setup failure closes the queue.
        let queue = unsafe { std::os::fd::OwnedFd::from_raw_fd(fd) };
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let changes = [reader, shutdown].map(|fd| libc::kevent {
            ident: fd as _,
            filter: libc::EVFILT_READ,
            flags: libc::EV_ADD | libc::EV_ENABLE,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        });
        if unsafe {
            libc::kevent(
                fd,
                changes.as_ptr(),
                2,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            queue,
            shutdown_fd: shutdown,
        })
    }

    /// False means shutdown; if output and shutdown arrive together, stop first.
    fn wait(&self) -> std::io::Result<bool> {
        let mut events = [libc::kevent {
            ident: 0,
            filter: 0,
            flags: 0,
            fflags: 0,
            data: 0,
            udata: std::ptr::null_mut(),
        }; 2];
        let count = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                2,
                std::ptr::null(),
            )
        };
        if count < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let events = &events[..count as usize];
        if events
            .iter()
            .any(|event| event.ident == self.shutdown_fd as _)
        {
            return Ok(false);
        }
        for event in events {
            if event.flags & libc::EV_ERROR != 0 {
                return Err(std::io::Error::from_raw_os_error(event.data as i32));
            }
        }
        Ok(true)
    }
}

pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    pub event_tx: mpsc::UnboundedSender<CoreEvent>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    stopped: Arc<AtomicBool>,
    #[cfg(unix)]
    shutdown: UnixStream,
}

impl Pty {
    /// A foreground job owns input when the terminal's process group differs
    /// from the shell's. Works without shell-integration hooks; queried on
    /// explicit execution and a throttled UI timer, never in parsing/render loops.
    #[cfg(unix)]
    pub fn foreground_job_running(&self) -> Option<bool> {
        let fd = self.master.as_raw_fd()?;
        let pid = libc::pid_t::try_from(self.child_pid()?).ok()?;
        let foreground = unsafe { libc::tcgetpgrp(fd) };
        let shell_group = unsafe { libc::getpgid(pid) };
        (foreground > 0 && shell_group > 0).then_some(foreground != shell_group)
    }

    /// A password-style prompt usually keeps canonical input on while
    /// disabling terminal echo. Full-screen TUIs often disable both; treating
    /// every raw-mode program as a password prompt would hold macOS Secure
    /// Event Input for the entire editor session. This is only a heuristic:
    /// callers must also offer a manual override for other prompt styles.
    #[cfg(target_os = "macos")]
    pub fn likely_password_prompt(&self) -> Option<bool> {
        let fd = self.master.as_raw_fd()?;
        let mut termios = std::mem::MaybeUninit::<libc::termios>::uninit();
        if unsafe { libc::tcgetattr(fd, termios.as_mut_ptr()) } != 0 {
            return None;
        }
        let termios = unsafe { termios.assume_init() };
        Some(termios.c_lflag & libc::ECHO == 0 && termios.c_lflag & libc::ICANON != 0)
    }

    /// Owned shell process identity for off-thread local workspace discovery.
    pub fn child_pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(|child| child.process_id())
    }

    /// Spawn a shell in a PTY. Returns:
    /// - the `Pty` handle (for writing keyboard input and resizing)
    /// - an `Arc<Mutex<Performer>>` shared with the reader thread
    /// - an `UnboundedReceiver<CoreEvent>` for the UI to poll
    pub fn spawn(
        shell: &str,
        args: &[String],
        cols: u16,
        rows: u16,
        on_data: impl Fn() + Send + 'static,
    ) -> anyhow::Result<(
        Self,
        Arc<Mutex<Performer>>,
        mpsc::UnboundedReceiver<CoreEvent>,
    )> {
        Self::spawn_in(shell, args, cols, rows, None, on_data)
    }

    /// Set the child's working directory without writing a command into its shell.
    /// An invalid explicit directory is an error, never silently substituted.
    pub fn spawn_in(
        shell: &str,
        args: &[String],
        cols: u16,
        rows: u16,
        cwd: Option<&std::path::Path>,
        on_data: impl Fn() + Send + 'static,
    ) -> anyhow::Result<(
        Self,
        Arc<Mutex<Performer>>,
        mpsc::UnboundedReceiver<CoreEvent>,
    )> {
        let cols = cols.max(1);
        let rows = rows.max(1);
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        if let Some(cwd) = cwd {
            anyhow::ensure!(
                cwd.is_absolute() && cwd.is_dir(),
                "invalid shell working directory"
            );
            cmd.cwd(cwd);
        }
        for arg in args {
            cmd.arg(arg);
        }
        // Set terminal environment so the shell and programs like starship use
        // full color and correct capabilities regardless of how Volt was launched.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        cmd.env("TERM_PROGRAM", "volt");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
        // Ensure UTF-8 locale for Nerd Font / Unicode rendering
        if std::env::var("LANG").is_err() {
            cmd.env("LANG", "en_US.UTF-8");
        }

        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let performer = Arc::new(Mutex::new(Performer::new(cols as usize, rows as usize)));
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Two-stage pipeline: a drain thread empties the kernel PTY buffer as
        // fast as possible (the PTY itself caps at ~200 MB/s on macOS, and any
        // time we spend parsing between reads stalls the writing program), and
        // a parse thread feeds the VTE parser from a bounded queue.
        #[cfg(not(unix))]
        let mut reader = pair.master.try_clone_reader()?;
        #[cfg(unix)]
        let (shutdown, shutdown_rx) = UnixStream::pair()?;
        #[cfg(unix)]
        let mut reader = {
            let fd = pair
                .master
                .as_raw_fd()
                .ok_or_else(|| anyhow::anyhow!("PTY missing Unix fd"))?;
            // Own a duplicate: poll must never observe a dropped/reused master fd.
            let fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
            if fd < 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            unsafe { std::fs::File::from_raw_fd(fd) }
        };
        #[cfg(target_os = "macos")]
        let readiness = PtyReadiness::new(reader.as_raw_fd(), shutdown_rx.as_raw_fd())?;
        // Complete fallible fd setup before starting the child.
        let child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);
        let stopped = Arc::new(AtomicBool::new(false));
        let reader_stopped = Arc::clone(&stopped);
        let (buf_tx, buf_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(PIPELINE_QUEUE_BUFFERS);
        let mut read_perf = PerfStats::from_env("pty-reader");
        let mut parse_perf = PerfStats::from_env("pty-parser");
        // Diagnostics count queued bytes plus at most one pending send. These
        // atomics are absent unless profiling is explicitly enabled.
        let pending_bytes = read_perf
            .as_ref()
            .map(|_| Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        let parser_pending_bytes = pending_bytes.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 64 * 1024];
            loop {
                if reader_stopped.load(Ordering::Relaxed) {
                    break;
                }
                let wait_start = PerfStats::start(&read_perf);
                #[cfg(target_os = "macos")]
                {
                    match readiness.wait() {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                    // Keep the shutdown descriptor owned by this worker while
                    // its raw fd remains registered with kqueue.
                    let _ = &shutdown_rx;
                }
                #[cfg(all(unix, not(target_os = "macos")))]
                {
                    let mut fds = [
                        libc::pollfd {
                            fd: reader.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        },
                        libc::pollfd {
                            fd: shutdown_rx.as_raw_fd(),
                            events: libc::POLLIN,
                            revents: 0,
                        },
                    ];
                    // Sleep until output OR pane closure; no periodic idle wakeups.
                    let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as _, -1) };
                    if result < 0 {
                        if std::io::Error::last_os_error().kind() == ErrorKind::Interrupted {
                            continue;
                        }
                        break;
                    }
                    if fds[1].revents != 0 {
                        break;
                    }
                }
                if let (Some(stats), Some(start)) = (read_perf.as_mut(), wait_start) {
                    stats.finish(Stage::ReadWait, start);
                }
                let read_start = PerfStats::start(&read_perf);
                let read_result = reader.read(&mut buf);
                if let (Some(stats), Some(start)) = (read_perf.as_mut(), read_start) {
                    stats.finish(Stage::Read, start);
                }
                match read_result {
                    Ok(0) => break,
                    Err(err) if err.kind() == ErrorKind::Interrupted => continue,
                    Err(_) => break,
                    // Receiver gone means the parse thread ended; stop draining.
                    Ok(n) => {
                        if let Some(stats) = read_perf.as_mut() {
                            stats.add(Counter::Bytes, n as u64);
                            stats.add(Counter::Reads, 1);
                            if let Some(pending) = pending_bytes.as_ref() {
                                stats.peak(
                                    Counter::PendingBytesPeak,
                                    (pending.fetch_add(n, Ordering::Relaxed) + n) as u64,
                                );
                            }
                        }
                        let send_start = PerfStats::start(&read_perf);
                        let sent = buf_tx.send(buf[..n].to_vec());
                        if let (Some(stats), Some(start)) = (read_perf.as_mut(), send_start) {
                            stats.finish(Stage::QueueSend, start);
                        }
                        if sent.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let performer_clone = Arc::clone(&performer);
        let writer_clone = Arc::clone(&writer);
        let event_tx_clone = event_tx.clone();
        let parser_stopped = Arc::clone(&stopped);
        std::thread::spawn(move || {
            let mut parser = Parser::new();
            'reader_loop: loop {
                let queue_start = PerfStats::start(&parse_perf);
                let received = buf_rx.recv();
                if let (Some(stats), Some(start)) = (parse_perf.as_mut(), queue_start) {
                    stats.finish(Stage::QueueWait, start);
                }
                match received {
                    Err(_) => break,
                    Ok(mut data) => {
                        if let Some(pending) = parser_pending_bytes.as_ref() {
                            pending.fetch_sub(data.len(), Ordering::Relaxed);
                        }
                        if let Some(stats) = parse_perf.as_mut() {
                            stats.add(Counter::Batches, 1);
                        }

                        if parser_stopped.load(Ordering::Relaxed) {
                            break;
                        }
                        // Coalesce everything already queued into one batch:
                        // kernel PTY reads are typically only a few KB, and the
                        // per-iteration overhead (locking, events, UI wakeup)
                        // dominates when parsing tiny buffers one at a time.
                        while data.len() < COALESCE_LIMIT_BYTES {
                            match buf_rx.try_recv() {
                                Ok(more) => {
                                    if let Some(pending) = parser_pending_bytes.as_ref() {
                                        pending.fetch_sub(more.len(), Ordering::Relaxed);
                                    }
                                    data.extend_from_slice(&more);
                                }
                                Err(_) => break,
                            }
                        }
                        let mut read_events: Vec<CoreEvent> = Vec::new();
                        let mut read_writes: Vec<Vec<u8>> = Vec::new();
                        let mut read_dirty = false;

                        for chunk in data.chunks(PARSE_LOCK_CHUNK_BYTES) {
                            if parser_stopped.load(Ordering::Relaxed) {
                                break 'reader_loop;
                            }
                            let (pending_events, pending_writes, chunk_dirty) = {
                                let lock_start = PerfStats::start(&parse_perf);
                                let mut p = match performer_clone.lock() {
                                    Ok(performer) => performer,
                                    Err(poisoned) => {
                                        // Another thread panicked; recover the guard
                                        // and report via the event channel instead of
                                        // panicking or going fully silent.
                                        let msg = format!("performer mutex poisoned: {poisoned}");
                                        send_event(&event_tx_clone, CoreEvent::PtyError(msg));
                                        poisoned.into_inner()
                                    }
                                };
                                if let (Some(stats), Some(start)) =
                                    (parse_perf.as_mut(), lock_start)
                                {
                                    stats.finish(Stage::ParseLock, start);
                                }
                                let parse_start = PerfStats::start(&parse_perf);
                                parser.advance(&mut *p, chunk);
                                if let (Some(stats), Some(start)) =
                                    (parse_perf.as_mut(), parse_start)
                                {
                                    stats.finish(Stage::Parse, start);
                                    stats.add(Counter::Chunks, 1);
                                    stats.add(Counter::Bytes, chunk.len() as u64);
                                }
                                let pending_events = if p.pending_events.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut p.pending_events))
                                };
                                let pending_writes = if p.pending_writes.is_empty() {
                                    None
                                } else {
                                    Some(std::mem::take(&mut p.pending_writes))
                                };
                                let dirty = p.display_dirty;
                                let _ = p.take_damage_rows();
                                p.display_dirty = false;
                                (pending_events, pending_writes, dirty)
                            };

                            if let Some(events) = pending_events {
                                read_events.extend(events);
                            }
                            if let Some(writes) = pending_writes {
                                read_writes.extend(writes);
                            }
                            if chunk_dirty {
                                read_dirty = true;
                            }
                        }

                        // OSC title/cwd/status can arrive without printable
                        // output. Wake the UI for these too, not just grid damage.
                        let notify_ui = read_dirty || !read_events.is_empty();
                        for event in read_events {
                            send_event(&event_tx_clone, event);
                        }

                        if !read_writes.is_empty() {
                            use std::io::Write;
                            let mut w = match writer_clone.lock() {
                                Ok(writer) => writer,
                                Err(poisoned) => {
                                    send_event(
                                        &event_tx_clone,
                                        CoreEvent::PtyError(format!(
                                            "PTY writer mutex poisoned: {poisoned}"
                                        )),
                                    );
                                    poisoned.into_inner()
                                }
                            };
                            for bytes in read_writes {
                                if let Err(err) = w.write_all(&bytes) {
                                    send_event(
                                        &event_tx_clone,
                                        CoreEvent::PtyError(format!(
                                            "failed to write PTY response bytes: {err}"
                                        )),
                                    );
                                    break 'reader_loop;
                                }
                            }
                            if let Err(err) = w.flush() {
                                send_event(
                                    &event_tx_clone,
                                    CoreEvent::PtyError(format!(
                                        "failed to flush PTY response bytes: {err}"
                                    )),
                                );
                            }
                        }

                        if notify_ui {
                            if let Some(stats) = parse_perf.as_mut() {
                                stats.add(Counter::Notifications, 1);
                            }
                            on_data();
                        }
                    }
                }
            }
            send_event(&event_tx_clone, CoreEvent::PtyClosed);
            on_data();
        });

        Ok((
            Self {
                master: pair.master,
                writer,
                event_tx,
                child: Some(child),
                stopped,
                #[cfg(unix)]
                shutdown,
            },
            performer,
            event_rx,
        ))
    }

    /// Write bytes to the PTY (keyboard input)
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        // Recover from a poisoned mutex rather than panicking — the writer
        // state is still valid even if another thread panicked holding it.
        let mut w = self
            .writer
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        w.write_all(bytes)
    }

    /// Resize the PTY
    pub fn resize(&mut self, cols: u16, rows: u16) -> anyhow::Result<()> {
        self.master.resize(PtySize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        #[cfg(unix)]
        {
            use std::io::Write;
            let _ = self.shutdown.write_all(&[1]);
        }
        if let Some(mut child) = self.child.take() {
            // Do not wait on the UI thread. Keep the child unreaped until after
            // signalling, so its PID cannot be reused for an unrelated process.
            std::thread::spawn(move || {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                let _ = child.kill(); // SIGHUP on Unix, matching terminal closure.
                for _ in 0..20 {
                    if matches!(child.try_wait(), Ok(Some(_))) {
                        return;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                #[cfg(unix)]
                if let Some(pid) = child.process_id() {
                    // Escalate only the owned, still-unreaped child, not arbitrary
                    // descendants or process groups which may outlive the pane.
                    unsafe {
                        libc::kill(pid as libc::pid_t, libc::SIGKILL);
                    }
                }
                let _ = child.wait();
            });
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn status_only_output_wakes_the_ui_before_any_grid_text() {
        let (tx, rx) = std::sync::mpsc::channel();
        let args = vec![
            "-c".into(),
            "printf '\\033]133;C\\007'; read -r reply; printf '\\033]133;D;1\\007'".into(),
        ];
        let (mut pty, _, mut events) = Pty::spawn("/bin/sh", &args, 20, 2, move || {
            let _ = tx.send(());
        })
        .unwrap();
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .expect("a pure OSC command event must wake the UI");
        assert!(matches!(events.try_recv(), Ok(CoreEvent::CommandStarted)));
        pty.write(b"done\n").unwrap();
    }

    #[test]
    fn foreground_job_detection_recovers_without_osc_hooks() {
        let (mut pty, _, _rx) = Pty::spawn("/bin/sh", &["-i".into()], 80, 24, || {}).unwrap();
        let wait_for = |pty: &Pty, expected| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if pty.foreground_job_running() == Some(expected) {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "foreground state did not become {expected}"
                );
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        };
        wait_for(&pty, false);
        pty.write(b"sleep 2\n").unwrap();
        wait_for(&pty, true);
        wait_for(&pty, false);
        // An ordinary Enter must not make the PTY permanently busy.
        pty.write(b"\n").unwrap();
        wait_for(&pty, false);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn canonical_no_echo_is_detected_without_treating_raw_tui_as_password() {
        let (mut pty, _, _rx) = Pty::spawn(
            "/bin/sh",
            &["-c".into(), "stty -echo; read answer; stty echo".into()],
            80,
            24,
            || {},
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while pty.likely_password_prompt() != Some(true) {
            assert!(
                std::time::Instant::now() < deadline,
                "no-echo prompt not observed"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        pty.write(b"test\n").unwrap();
        while pty.likely_password_prompt() == Some(true) {
            assert!(
                std::time::Instant::now() < deadline,
                "echo was not restored"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let (pty, _, _rx_raw) = Pty::spawn(
            "/bin/sh",
            &["-c".into(), "stty raw -echo; read answer".into()],
            80,
            24,
            || {},
        )
        .unwrap();
        // Wait for the raw-mode setup before asserting; the initial shell
        // state also returns false, so check ICANON directly on the master.
        while pty
            .master
            .as_raw_fd()
            .and_then(|fd| {
                let mut t = std::mem::MaybeUninit::<libc::termios>::uninit();
                (unsafe { libc::tcgetattr(fd, t.as_mut_ptr()) } == 0)
                    .then(|| unsafe { t.assume_init() })
            })
            .is_some_and(|t| t.c_lflag & libc::ICANON != 0)
        {
            assert!(
                std::time::Instant::now() < deadline,
                "raw mode not observed"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(pty.likely_password_prompt(), Some(false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn readiness_drains_tail_before_eof_and_shutdown_takes_priority() {
        use std::io::Write;
        let (mut reader, mut output) = UnixStream::pair().unwrap();
        let (shutdown_rx, mut shutdown_tx) = UnixStream::pair().unwrap();
        let readiness = PtyReadiness::new(reader.as_raw_fd(), shutdown_rx.as_raw_fd()).unwrap();
        assert_ne!(
            unsafe { libc::fcntl(readiness.queue.as_raw_fd(), libc::F_GETFD) } & libc::FD_CLOEXEC,
            0
        );
        output.write_all(b"abcdef").unwrap();
        drop(output);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = [0; 3];
            for expected in [b"abc", b"def"] {
                assert!(readiness.wait().unwrap());
                reader.read_exact(&mut bytes).unwrap();
                assert_eq!(&bytes, expected);
            }
            assert!(readiness.wait().unwrap());
            assert_eq!(reader.read(&mut bytes).unwrap(), 0);
            // EOF remains readable, but shutdown must win over it.
            shutdown_tx.write_all(&[1]).unwrap();
            assert!(!readiness.wait().unwrap());
            drop(shutdown_rx);
            tx.send(()).unwrap();
        });
        rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn readiness_shutdown_wakes_without_pty_output() {
        use std::io::Write;
        let (reader, _output) = UnixStream::pair().unwrap();
        let (shutdown_rx, mut shutdown_tx) = UnixStream::pair().unwrap();
        let readiness = PtyReadiness::new(reader.as_raw_fd(), shutdown_rx.as_raw_fd()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = readiness.wait().unwrap();
            drop((reader, shutdown_rx));
            tx.send(result).unwrap();
        });
        shutdown_tx.write_all(&[1]).unwrap();
        assert!(!rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn drop_terminates_hup_ignoring_child_and_releases_pipeline() {
        let (pty, performer, mut rx) = Pty::spawn(
            "/bin/sh",
            &[
                "-c".into(),
                "trap '' HUP; printf ready; while :; do :; done".into(),
            ],
            10,
            2,
            || {},
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while performer.lock().unwrap().grid.cell(0, 0).c() != 'r' {
            assert!(std::time::Instant::now() < deadline, "child not ready");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let pid = pty.child.as_ref().unwrap().process_id().unwrap() as libc::pid_t;
        let start = std::time::Instant::now();
        drop(pty);
        assert!(
            start.elapsed() < std::time::Duration::from_secs(1),
            "UI blocked in drop"
        );
        let mut closed = false;
        while std::time::Instant::now() < deadline {
            while let Ok(event) = rx.try_recv() {
                closed |= matches!(event, CoreEvent::PtyClosed);
            }
            // ESRCH includes reaping: a zombie would still be addressable.
            let gone = unsafe { libc::kill(pid, 0) } == -1
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
            if gone && closed && Arc::strong_count(&performer) == 1 {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("PTY child/pipeline survived drop");
    }

    #[test]
    fn repeated_output_exit_drains_tail_and_releases_workers() {
        for _ in 0..12 {
            let args = vec!["-c".into(), "read start; i=0; while [ $i -lt 2000 ]; do printf 'row %s 日本語\\n' \"$i\"; i=$((i+1)); done; printf 'FINAL-MARKER'".into()];
            let (mut pty, performer, mut rx) = Pty::spawn("/bin/sh", &args, 40, 5, || {}).unwrap();
            performer.lock().unwrap().set_scrollback_limit(64);
            pty.write(b"start\n").unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                if matches!(rx.try_recv(), Ok(CoreEvent::PtyClosed)) {
                    break;
                }
                assert!(std::time::Instant::now() < deadline, "PTY did not close");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            {
                let p = performer.lock().unwrap();
                assert_eq!(p.grid.scrollback_len(), 64);
                assert!((0..p.grid.rows).any(|r| p
                    .grid
                    .row_text(p.grid.row_cells(r))
                    .contains("FINAL-MARKER")));
            }
            drop(pty);
            while Arc::strong_count(&performer) != 1 {
                assert!(std::time::Instant::now() < deadline, "worker leaked");
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    }

    #[test]
    fn spawn_in_uses_child_cwd_and_does_not_change_parent() {
        let parent = std::env::current_dir().unwrap();
        let cwd = std::env::temp_dir().canonicalize().unwrap();
        let (pty, performer, mut rx) = Pty::spawn_in(
            "/bin/sh",
            &["-c".into(), "printf 'CWD:%s' \"$PWD\"".into()],
            512,
            2,
            Some(&cwd),
            || {},
        )
        .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if matches!(rx.try_recv(), Ok(CoreEvent::PtyClosed)) {
                break;
            }
            assert!(std::time::Instant::now() < deadline, "child did not exit");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let p = performer.lock().unwrap();
        assert!(p
            .grid
            .row_text(p.grid.row_cells(0))
            .starts_with(&format!("CWD:{}", cwd.display())));
        assert_eq!(std::env::current_dir().unwrap(), parent);
        drop(p);
        drop(pty);
    }

    #[test]
    fn spawn_in_rejects_relative_or_missing_directories() {
        for cwd in [
            std::path::Path::new("relative"),
            std::path::Path::new("/volt-definitely-missing-directory"),
        ] {
            assert!(Pty::spawn_in("/bin/sh", &[], 80, 24, Some(cwd), || {}).is_err());
        }
    }

    #[test]
    fn test_pty_spawn_and_write() {
        // Spawn a shell, write "exit\n", verify no panic and channel gets an event.
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let (mut pty, _performer, mut rx) = Pty::spawn(&shell, &[], 80, 24, || {}).unwrap();
        pty.write(b"exit\n").unwrap();
        let start = std::time::Instant::now();
        let mut got_event = false;
        while start.elapsed() < std::time::Duration::from_secs(3) {
            if rx.try_recv().is_ok() {
                got_event = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(got_event);
    }
}
