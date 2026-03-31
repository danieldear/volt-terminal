use std::io::Read;
use std::sync::{Arc, Mutex};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use tokio::sync::mpsc;
use vte::Parser;

use crate::events::CoreEvent;
use crate::performer::Performer;

pub struct Pty {
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    pub event_tx: mpsc::UnboundedSender<CoreEvent>,
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
    ) -> anyhow::Result<(Self, Arc<Mutex<Performer>>, mpsc::UnboundedReceiver<CoreEvent>)> {
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
        let _child = pair.slave.spawn_command(cmd)?;

        let writer = pair.master.take_writer()?;
        let performer = Arc::new(Mutex::new(Performer::new(cols as usize, rows as usize)));
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Background reader thread — reads PTY output, feeds VTE parser, fires events
        let mut reader = pair.master.try_clone_reader()?;
        let performer_clone = Arc::clone(&performer);
        let event_tx_clone = event_tx.clone();
        std::thread::spawn(move || {
            let mut parser = Parser::new();
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut p = performer_clone.lock().unwrap();
                        for &b in &buf[..n] {
                            parser.advance(&mut *p, b);
                        }
                        let _ = event_tx_clone.send(CoreEvent::GridUpdated);
                    }
                }
            }
        });

        Ok((
            Self {
                master: pair.master,
                writer,
                event_tx,
            },
            performer,
            event_rx,
        ))
    }

    /// Write bytes to the PTY (keyboard input)
    pub fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.writer.write_all(bytes)
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
        // Spawn a shell, write "exit\n", verify no panic and channel gets an event
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let (mut pty, _performer, mut rx) = Pty::spawn(&shell, &[], 80, 24).unwrap();
        pty.write(b"exit\n").unwrap();
        // Give the reader thread a moment to process output
        std::thread::sleep(std::time::Duration::from_millis(200));
        // At least one GridUpdated event should have arrived
        assert!(rx.try_recv().is_ok());
    }
}
