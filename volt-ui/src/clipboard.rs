//! On-demand system clipboard access. No shell, payload logging, or idle work.
//!
//! Calls are synchronous and bounded by a two-second operation deadline plus
//! at most 100 ms of error cleanup. Call from a worker, not the UI event loop.
//! Clipboard bytes are not decoded, normalized, or included in error messages.
use std::io;

pub type Result<T> = io::Result<T>;

/// Read the regular clipboard, rejecting (never truncating) oversized content.
pub fn read(max_bytes: usize) -> Result<Vec<u8>> {
    platform::access(None, max_bytes)
}

/// Write bytes verbatim. Reject oversized input before launching any utility.
pub fn write(bytes: &[u8], max_bytes: usize) -> Result<()> {
    if bytes.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "clipboard input exceeds byte limit",
        ));
    }
    platform::access(Some(bytes), max_bytes).map(|_| ())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod platform {
    use super::*;
    pub(super) fn access(_: Option<&[u8]>, _: usize) -> Result<Vec<u8>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "clipboard platform is unsupported",
        ))
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
mod platform {
    use super::*;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    const DEADLINE: Duration = Duration::from_secs(2);
    const CLEANUP: Duration = Duration::from_millis(100);
    const TICK: Duration = Duration::from_millis(2);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Utility {
        program: &'static str,
        args: &'static [&'static str],
    }

    fn candidates(macos: bool, wayland: bool, x11: bool, write: bool) -> Vec<Utility> {
        if macos {
            return vec![Utility {
                program: if write {
                    "/usr/bin/pbcopy"
                } else {
                    "/usr/bin/pbpaste"
                },
                args: &[],
            }];
        }
        let mut choices = Vec::new();
        if wayland {
            choices.push(if write {
                Utility {
                    program: "wl-copy",
                    args: &["--type", "text/plain;charset=utf-8"],
                }
            } else {
                Utility {
                    program: "wl-paste",
                    args: &["--no-newline", "--type", "text"],
                }
            });
        }
        if x11 {
            choices.push(Utility {
                program: "xclip",
                args: if write {
                    &["-selection", "clipboard", "-in"]
                } else {
                    &["-selection", "clipboard", "-out"]
                },
            });
            choices.push(Utility {
                program: "xsel",
                args: if write {
                    &["--clipboard", "--input"]
                } else {
                    &["--clipboard", "--output"]
                },
            });
        }
        choices
    }

    pub(super) fn access(input: Option<&[u8]>, max_bytes: usize) -> Result<Vec<u8>> {
        let present = |key| std::env::var_os(key).is_some_and(|v| !v.is_empty());
        let choices = candidates(
            cfg!(target_os = "macos"),
            present("WAYLAND_DISPLAY"),
            present("DISPLAY"),
            input.is_some(),
        );
        let commands = choices.into_iter().map(|utility| {
            let mut command = Command::new(utility.program);
            command.args(utility.args);
            command
        });
        run_candidates(commands, input, max_bytes, DEADLINE)
    }

    fn run_candidates(
        commands: impl IntoIterator<Item = Command>,
        input: Option<&[u8]>,
        max_bytes: usize,
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        if input.is_some_and(|b| b.len() > max_bytes) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "clipboard input exceeds byte limit",
            ));
        }
        let deadline = Instant::now() + timeout;
        for mut command in commands {
            match run(&mut command, input, max_bytes, deadline) {
                // Only a missing executable permits fallback. A utility error,
                // disconnected display, permission error, or timeout does not.
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                result => return result,
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no clipboard utility available for the active display",
        ))
    }

    fn nonblocking(pipe: &impl AsRawFd) -> Result<()> {
        let fd = pipe.as_raw_fd();
        // SAFETY: fd belongs to a live pipe, and these fcntl operations only
        // query/set its flags. No pointer arguments or ownership transfer.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::other("clipboard pipe setup failed"));
        }
        Ok(())
    }

    struct Process {
        child: Child,
        complete: bool,
    }

    impl Drop for Process {
        fn drop(&mut self) {
            if self.complete {
                return;
            }
            // Kill the new process group as well as the direct child: inherited
            // pipes held by descendants must not strand a read or write. A
            // successful copy deliberately leaves the utility's clipboard owner
            // alive; it is not a Volt-owned idle watcher.
            // SAFETY: child was spawned in its own group; negative PID targets
            // that group, never Volt's process group.
            unsafe {
                libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL);
            }
            let _ = self.child.kill();
            let until = Instant::now() + CLEANUP;
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) | Err(_) => break,
                    Ok(None) if Instant::now() < until => std::thread::sleep(TICK),
                    Ok(None) => break,
                }
            }
        }
    }

    fn run(
        command: &mut Command,
        input: Option<&[u8]>,
        max_bytes: usize,
        deadline: Instant,
    ) -> Result<Vec<u8>> {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "clipboard operation timed out",
            ));
        }
        command
            .process_group(0)
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(if input.is_some() {
                Stdio::null()
            } else {
                Stdio::piped()
            })
            // Kernel discards stderr: no payload retention, drain deadlock, or
            // unbounded diagnostic allocation, even from a noisy utility.
            .stderr(Stdio::null());
        let child = command
            .spawn()
            .map_err(|error| io::Error::new(error.kind(), "clipboard utility could not start"))?;
        let mut process = Process {
            child,
            complete: false,
        };
        let mut stdin = process.child.stdin.take();
        let mut stdout = process.child.stdout.take();
        if let Some(pipe) = &stdin {
            nonblocking(pipe)?;
        }
        if let Some(pipe) = &stdout {
            nonblocking(pipe)?;
        }
        let mut result = Vec::new();
        let mut written = 0;
        let mut status = None;
        loop {
            let mut progressed = false;
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "clipboard operation timed out",
                ));
            }
            if let Some(pipe) = &mut stdin {
                let bytes = input.unwrap_or_default();
                if written < bytes.len() {
                    let end = bytes.len().min(written.saturating_add(8192));
                    match pipe.write(&bytes[written..end]) {
                        Ok(0) => {
                            return Err(io::Error::new(
                                io::ErrorKind::WriteZero,
                                "clipboard write stalled",
                            ))
                        }
                        Ok(count) => {
                            written += count;
                            progressed = true;
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                            ) => {}
                        Err(_) => {
                            return Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "clipboard write failed",
                            ))
                        }
                    }
                }
                if written == bytes.len() {
                    stdin = None;
                } // EOF, including empty writes.
            }
            if let Some(pipe) = &mut stdout {
                let mut buffer = [0_u8; 8192];
                // One bounded chunk per iteration: floods cannot starve the
                // deadline or child-status check. Probe one byte beyond the cap.
                let capacity = buffer
                    .len()
                    .min(max_bytes.saturating_sub(result.len()).saturating_add(1));
                match pipe.read(&mut buffer[..capacity]) {
                    Ok(0) => stdout = None,
                    Ok(count) => {
                        if count > max_bytes.saturating_sub(result.len()) {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "clipboard output exceeds byte limit",
                            ));
                        }
                        result
                            .try_reserve(count)
                            .map_err(|_| io::Error::other("clipboard allocation failed"))?;
                        result.extend_from_slice(&buffer[..count]);
                        progressed = true;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(_) => return Err(io::Error::other("clipboard read failed")),
                }
            }
            if status.is_none() {
                status = process
                    .child
                    .try_wait()
                    .map_err(|_| io::Error::other("clipboard process wait failed"))?;
            }
            if let Some(exit) = status {
                if !exit.success() {
                    return Err(io::Error::other("clipboard utility failed"));
                }
                if stdin.is_none() && stdout.is_none() {
                    process.complete = true;
                    return Ok(result);
                }
            }
            if !progressed {
                std::thread::sleep(TICK);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Mock {
            dir: PathBuf,
            executable: PathBuf,
        }
        impl Mock {
            fn new(script: &str) -> Self {
                static ID: AtomicUsize = AtomicUsize::new(0);
                let dir = std::env::temp_dir().join(format!(
                    "volt-clipboard-{}-{}",
                    std::process::id(),
                    ID.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir(&dir).unwrap();
                let executable = dir.join("mock utility");
                // A fixed fixture script is an executable mock, not the backend
                // evaluating clipboard bytes as a shell command.
                fs::write(&executable, format!("#!/bin/sh\n{script}\n")).unwrap();
                fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
                Self { dir, executable }
            }
            fn command(&self) -> Command {
                Command::new(&self.executable)
            }
            fn execute(&self, input: Option<&[u8]>, cap: usize) -> Result<Vec<u8>> {
                // Functional tests must allow loaded CI/process-startup scheduling.
                self.execute_with_timeout(input, cap, Duration::from_secs(3))
            }
            fn execute_with_timeout(
                &self,
                input: Option<&[u8]>,
                cap: usize,
                timeout: Duration,
            ) -> Result<Vec<u8>> {
                run_candidates([self.command()], input, cap, timeout)
            }
        }
        impl Drop for Mock {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.dir);
            }
        }

        #[test]
        fn read_exact_limit_and_empty() {
            assert_eq!(
                Mock::new("printf 'abcd'").execute(None, 4).unwrap(),
                b"abcd"
            );
            assert!(Mock::new("exit 0").execute(None, 0).unwrap().is_empty());
            assert_eq!(
                Mock::new("printf x").execute(None, 0).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        #[test]
        fn read_overflow_is_not_truncated() {
            assert_eq!(
                Mock::new("printf 'abcde'")
                    .execute(None, 4)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
        #[test]
        fn write_raw_bytes_not_shell_syntax_and_empty() {
            let mock = Mock::new("cat > \"$1\"");
            let output = mock.dir.join("bytes");
            let bytes = b"$(touch PWNED); `id`\n'\"\\\0\xff";
            let mut command = mock.command();
            command.arg(&output);
            run_candidates([command], Some(bytes), bytes.len(), Duration::from_secs(1)).unwrap();
            assert_eq!(fs::read(&output).unwrap(), bytes);
            let mut command = mock.command();
            command.arg(&output);
            run_candidates([command], Some(b""), 0, Duration::from_secs(1)).unwrap();
            assert_eq!(fs::read(&output).unwrap(), b"");
        }
        #[test]
        fn oversized_write_never_spawns() {
            let mock = Mock::new("touch \"$1\"");
            let marker = mock.dir.join("spawned");
            let mut command = mock.command();
            command.arg(&marker);
            assert_eq!(
                run_candidates([command], Some(b"ab"), 1, DEADLINE)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
            assert!(!marker.exists());
            assert_eq!(
                super::super::write(b"ab", 1).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        #[test]
        fn missing_utility_falls_back_but_failure_does_not() {
            let good = Mock::new("printf ok");
            let missing = || Command::new(good.dir.join("does-not-exist"));
            assert_eq!(
                run_candidates([missing(), good.command()], None, 2, DEADLINE).unwrap(),
                b"ok"
            );
            assert_eq!(
                run_candidates([missing()], None, 2, DEADLINE)
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::NotFound
            );
            let bad = Mock::new("printf 'secret diagnostic' >&2; exit 3");
            let error =
                run_candidates([bad.command(), good.command()], None, 2, DEADLINE).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::Other);
            assert!(!error.to_string().contains("secret"));
        }
        #[test]
        fn selection_respects_display_and_platform() {
            assert!(candidates(false, false, false, false).is_empty());
            for write in [false, true] {
                let wayland = candidates(false, true, false, write);
                assert_eq!(wayland.len(), 1);
                assert!(wayland[0].program.starts_with("wl-"));
                let x11 = candidates(false, false, true, write);
                assert_eq!(
                    x11.iter().map(|u| u.program).collect::<Vec<_>>(),
                    ["xclip", "xsel"]
                );
                assert_eq!(candidates(false, true, true, write).len(), 3);
                assert_eq!(candidates(true, true, true, write).len(), 1);
            }
        }
        #[test]
        fn stderr_flood_cannot_deadlock() {
            let mock = Mock::new("dd if=/dev/zero bs=8192 count=64 >&2 2>/dev/null; printf ok");
            assert_eq!(mock.execute(None, 2).unwrap(), b"ok");
        }
        #[test]
        fn early_exit_during_large_write_is_error() {
            let mock = Mock::new("exit 0");
            let bytes = vec![b'x'; 1024 * 1024];
            assert!(mock.execute(Some(&bytes), bytes.len()).is_err());
        }
        #[test]
        fn timeouts_bound_reads_writes_and_inherited_pipe_drain() {
            for script in ["sleep 10", "sleep 10 & exit 0", "exec sleep 10"] {
                let mock = Mock::new(script);
                let start = Instant::now();
                assert_eq!(
                    mock.execute_with_timeout(None, 1, Duration::from_millis(200))
                        .unwrap_err()
                        .kind(),
                    io::ErrorKind::TimedOut
                );
                assert!(start.elapsed() < Duration::from_secs(2));
            }
            let mock = Mock::new("exec sleep 10");
            let bytes = vec![b'x'; 1024 * 1024];
            let start = Instant::now();
            assert_eq!(
                mock.execute_with_timeout(Some(&bytes), bytes.len(), Duration::from_millis(200))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::TimedOut
            );
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        #[test]
        fn timeout_kills_and_reaps_direct_child() {
            let mock = Mock::new("echo $$ > \"$1\"; exec sleep 10");
            let marker = mock.dir.join("pid");
            let mut command = mock.command();
            command.arg(&marker);
            assert_eq!(
                run_candidates([command], None, 1, Duration::from_millis(200))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::TimedOut
            );
            let pid: libc::pid_t = fs::read_to_string(marker).unwrap().trim().parse().unwrap();
            // SAFETY: signal zero queries existence, sending no signal.
            assert_eq!(unsafe { libc::kill(pid, 0) }, -1);
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
            let mut status = 0;
            // SAFETY: status points to valid writable storage; WNOHANG never blocks.
            assert_eq!(
                unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
                -1
            );
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(libc::ECHILD)
            );
        }
    }
}
