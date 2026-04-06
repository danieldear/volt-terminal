use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum CoreEvent {
    GridUpdated {
        damaged_rows: Option<(usize, usize)>,
    },
    CwdChanged(PathBuf),
    TitleChanged(String),
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
