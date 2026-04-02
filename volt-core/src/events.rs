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
    PtyClosed,
}
