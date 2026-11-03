use std::path::PathBuf;

#[derive(Clone)]
pub enum CoreEvent {
    /// Bounded untrusted OSC 1337 bytes; host opt-in policy and decoder required.
    /// Position is captured at dispatch, not when the host drains the event.
    InlineImageRequested {
        parameters: Vec<u8>,
        alternate_screen: bool,
        epoch: u64,
        row: u64,
        column: u32,
    },
    /// Untrusted terminal-output request; host policy must approve before use.
    ClipboardWriteRequested(Vec<u8>),
    SynchronizedOutput {
        deadline: Option<std::time::Instant>,
    },
    GridUpdated {
        damaged_rows: Option<(usize, usize)>,
    },
    CwdChanged(PathBuf),
    TitleChanged(String),
    CommandStarted,
    CommandFinished {
        exit_code: i32,
        duration_ms: u64,
    },
    /// A non-fatal error occurred in the PTY reader thread (e.g. mutex
    /// poisoned, write failed).  The message is shown as an in-terminal
    /// alert so users see it instead of it silently going to stderr.
    PtyError(String),
    PtyClosed,
}

impl std::fmt::Debug for CoreEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClipboardWriteRequested(bytes) => f
                .debug_struct("ClipboardWriteRequested")
                .field("bytes", &bytes.len())
                .field("payload", &"<redacted>")
                .finish(),
            Self::InlineImageRequested {
                parameters,
                alternate_screen,
                epoch,
                row,
                column,
            } => f
                .debug_struct("InlineImageRequested")
                .field("bytes", &parameters.len())
                .field("alternate_screen", alternate_screen)
                .field("epoch", epoch)
                .field("row", row)
                .field("column", column)
                .finish(),
            Self::SynchronizedOutput { deadline } => f
                .debug_struct("SynchronizedOutput")
                .field("deadline", deadline)
                .finish(),
            Self::GridUpdated { damaged_rows } => f
                .debug_struct("GridUpdated")
                .field("damaged_rows", damaged_rows)
                .finish(),
            Self::CwdChanged(path) => f.debug_tuple("CwdChanged").field(path).finish(),
            Self::TitleChanged(title) => f.debug_tuple("TitleChanged").field(title).finish(),
            Self::CommandStarted => f.write_str("CommandStarted"),
            Self::CommandFinished {
                exit_code,
                duration_ms,
            } => f
                .debug_struct("CommandFinished")
                .field("exit_code", exit_code)
                .field("duration_ms", duration_ms)
                .finish(),
            Self::PtyError(message) => f.debug_tuple("PtyError").field(message).finish(),
            Self::PtyClosed => f.write_str("PtyClosed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CoreEvent;
    #[test]
    fn clipboard_payload_debug_is_redacted() {
        let event = CoreEvent::ClipboardWriteRequested(b"credential-value".to_vec());
        let debug = format!("{event:?}");
        assert!(debug.contains("redacted"));
        assert!(!debug.contains("credential"));
    }
}
