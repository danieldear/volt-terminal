use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use vte::Parser;

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

pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Arc<Mutex<Box<dyn std::io::Write + Send>>>,
    pub event_tx: mpsc::UnboundedSender<CoreEvent>,
    _child: Box<dyn portable_pty::Child + Send + Sync>,
}

impl Pty {
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
        let pty_system = native_pty_system();
        let pair = pty_system.openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;

        let mut cmd = CommandBuilder::new(shell);
        for arg in args {
            cmd.arg(arg);
        }
        // Set terminal environment so the shell and programs like starship use
        // full color and correct capabilities regardless of how Volt was launched.
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");
        // Ensure UTF-8 locale for Nerd Font / Unicode rendering
        if std::env::var("LANG").is_err() {
            cmd.env("LANG", "en_US.UTF-8");
        }
        let child = pair.slave.spawn_command(cmd)?;

        let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
        let performer = Arc::new(Mutex::new(Performer::new(cols as usize, rows as usize)));
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Two-stage pipeline: a drain thread empties the kernel PTY buffer as
        // fast as possible (the PTY itself caps at ~200 MB/s on macOS, and any
        // time we spend parsing between reads stalls the writing program), and
        // a parse thread feeds the VTE parser from a bounded queue.
        let mut reader = pair.master.try_clone_reader()?;
        let (buf_tx, buf_rx) = std::sync::mpsc::sync_channel::<Vec<u8>>(PIPELINE_QUEUE_BUFFERS);
        std::thread::spawn(move || {
            let mut buf = [0u8; 64 * 1024];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    // Receiver gone means the parse thread ended; stop draining.
                    Ok(n) => {
                        if buf_tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let performer_clone = Arc::clone(&performer);
        let writer_clone = Arc::clone(&writer);
        let event_tx_clone = event_tx.clone();
        std::thread::spawn(move || {
            let mut parser = Parser::new();
            'reader_loop: loop {
                match buf_rx.recv() {
                    Err(_) => break,
                    Ok(mut data) => {
                        // Coalesce everything already queued into one batch:
                        // kernel PTY reads are typically only a few KB, and the
                        // per-iteration overhead (locking, events, UI wakeup)
                        // dominates when parsing tiny buffers one at a time.
                        while data.len() < COALESCE_LIMIT_BYTES {
                            match buf_rx.try_recv() {
                                Ok(more) => data.extend_from_slice(&more),
                                Err(_) => break,
                            }
                        }
                        let mut read_events: Vec<CoreEvent> = Vec::new();
                        let mut read_writes: Vec<Vec<u8>> = Vec::new();
                        let mut read_dirty = false;

                        for chunk in data.chunks(PARSE_LOCK_CHUNK_BYTES) {
                            let (pending_events, pending_writes, chunk_dirty) = {
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
                                parser.advance(&mut *p, chunk);
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

                        if read_dirty {
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
                _child: child,
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
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
