/// Per-tab and per-pane state for the Volt terminal.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use winit::event_loop::EventLoopProxy;

use volt_config::Config;
use volt_core::events::CoreEvent;
use volt_core::grid::Grid;
use volt_core::performer::Performer;
use volt_core::pty::Pty;
use volt_renderer::tab_color::TabColor;

use crate::app::VoltEvent;
use crate::pane_tree::PaneTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSplitDirection {
    Vertical,
    Horizontal,
}

/// Shell-reported exit codes and a throttled foreground fallback. A native
/// process-group observation can prove busy/idle, never success/failure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CommandStatus {
    pub running: bool,
    pub last_exit_code: Option<i32>,
    pub shell_reports: bool,
}
impl CommandStatus {
    pub fn started(&mut self) {
        self.running = true;
        self.last_exit_code = None;
        self.shell_reports = true;
    }
    pub fn finished(&mut self, code: i32) {
        self.running = false;
        self.last_exit_code = (code >= 0).then_some(code);
        self.shell_reports = true;
    }
    pub fn observe_foreground(&mut self, running: bool) {
        if !self.shell_reports {
            self.running = running;
            self.last_exit_code = None;
        }
    }
    pub fn tab_status(self) -> volt_renderer::TabStatus {
        if self.running {
            volt_renderer::TabStatus::Running
        } else if self.last_exit_code.is_some_and(|code| code > 0) {
            volt_renderer::TabStatus::Failed
        } else {
            volt_renderer::TabStatus::Idle
        }
    }
}

pub struct TerminalPane {
    pub pty: Pty,
    pub performer: Arc<Mutex<Performer>>,
    pub event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub status: CommandStatus,
    /// How many lines the user has scrolled back into the scrollback buffer.
    /// 0 means the live view (bottom of output).
    pub scroll_view_offset: usize,
    /// Pane-scoped override from "Change Terminal Title...".
    /// Takes precedence over OSC title/cwd until cleared.
    pub custom_title: Option<String>,
    /// When true, keyboard/paste input to this pane's PTY is suppressed.
    /// Set via the context menu's "Terminal Read-only" toggle.
    pub read_only: bool,
    /// The last project task typed into this pane, and how it went.
    /// Boxed: most panes never run a task, and panes are moved around in trees.
    pub task_run: Option<Box<crate::tasks::TaskRun>>,
    pub task_results: Vec<crate::tasks::TaskRun>,
}

pub struct TerminalTab {
    pub tree: PaneTree,
    /// Tab-scoped override; unlike a pane title it survives pane focus changes.
    pub custom_title: Option<String>,
    /// User-selected accent, independent of running/idle state.
    pub color: Option<TabColor>,
}

#[derive(Debug, Clone, Copy)]
pub enum SelectionMode {
    Linear,
    Block,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub pane_id: usize,
    /// Prevent a main-screen selection from highlighting unrelated alt-screen
    /// cells (and vice versa) when a TUI switches screens.
    pub alternate_screen: bool,
    pub mode: SelectionMode,
    pub start_col: usize,
    /// Absolute history + live row number, not a viewport coordinate.
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl Selection {
    pub fn at_viewport(
        pane_id: usize,
        alternate_screen: bool,
        mode: SelectionMode,
        grid: &Grid,
        offset: usize,
        col: usize,
        row: usize,
    ) -> Option<Self> {
        let row = grid.viewport_absolute_row(offset, row)?;
        Some(Self {
            pane_id,
            alternate_screen,
            mode,
            start_col: col,
            start_row: row,
            end_col: col,
            end_row: row,
        })
    }

    /// Shift-click and drag keep the original anchor, even after scrolling.
    pub fn extend_to(
        &mut self,
        pane_id: usize,
        alternate_screen: bool,
        grid: &Grid,
        offset: usize,
        col: usize,
        row: usize,
    ) -> bool {
        let origin = grid.scrollback_origin();
        let retained_end = origin.saturating_add(grid.scrollback_len() + grid.rows);
        if self.pane_id != pane_id
            || self.alternate_screen != alternate_screen
            || self.start_row < origin
            || self.start_row >= retained_end
        {
            return false;
        }
        let Some(end_row) = grid.viewport_absolute_row(offset, row) else {
            return false;
        };
        self.end_col = col;
        self.end_row = end_row;
        true
    }

    /// Clip absolute selection endpoints to the visible pane. For a linear
    /// selection, a clipped first/last row occupies the entire visible row.
    pub fn visible_range(
        self,
        grid: &Grid,
        offset: usize,
        pane_rows: usize,
    ) -> Option<((usize, usize), (usize, usize))> {
        let sel = self.normalized();
        let origin = grid.scrollback_origin();
        let last = origin.checked_add(grid.scrollback_len() + grid.rows - 1)?;
        if sel.start_row < origin || sel.end_row > last || pane_rows == 0 {
            return None;
        }
        let first_visible = grid.viewport_absolute_row(offset, 0)?;
        let last_visible = first_visible.checked_add(pane_rows - 1)?;
        let first = sel.start_row.max(first_visible);
        let last = sel.end_row.min(last_visible);
        if first > last {
            return None;
        }
        let (start_col, end_col) = match sel.mode {
            SelectionMode::Linear => (
                if first > sel.start_row {
                    0
                } else {
                    sel.start_col
                },
                if last < sel.end_row {
                    grid.cols - 1
                } else {
                    sel.end_col
                },
            ),
            SelectionMode::Block => (sel.start_col, sel.end_col),
        };
        Some((
            (start_col, first - first_visible),
            (end_col, last - first_visible),
        ))
    }

    /// Copy the selected rows from retained history, not just the viewport.
    pub fn text(self, grid: &Grid) -> Option<String> {
        let sel = self.normalized();
        let origin = grid.scrollback_origin();
        let history = grid.scrollback_len();
        let total = history + grid.rows;
        let start = sel.start_row.checked_sub(origin)?;
        let end = sel.end_row.checked_sub(origin)?;
        if start >= total || end >= total {
            return None;
        }
        let mut out = String::new();
        for row in start..=end {
            let (start_col, end_col) = match sel.mode {
                SelectionMode::Block => (
                    sel.start_col.min(grid.cols - 1),
                    sel.end_col.min(grid.cols - 1),
                ),
                SelectionMode::Linear => (
                    if row == start { sel.start_col } else { 0 },
                    if row == end {
                        sel.end_col.min(grid.cols - 1)
                    } else {
                        grid.cols - 1
                    },
                ),
            };
            let cells = if row < history {
                grid.scrollback_row(row)
            } else {
                grid.row_cells(row - history)
            };
            let mut line = String::new();
            if start_col <= end_col && start_col < grid.cols {
                for cell in &cells[start_col..=end_col] {
                    grid.push_cell_text(&mut line, cell);
                }
            }
            if matches!(sel.mode, SelectionMode::Block) {
                out.push_str(&line);
            } else {
                out.push_str(line.trim_end_matches(' '));
            }
            if row != end {
                out.push('\n');
            }
        }
        Some(out)
    }

    pub fn normalized(self) -> Self {
        match self.mode {
            SelectionMode::Linear => {
                if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
                    self
                } else {
                    Self {
                        pane_id: self.pane_id,
                        alternate_screen: self.alternate_screen,
                        mode: self.mode,
                        start_col: self.end_col,
                        start_row: self.end_row,
                        end_col: self.start_col,
                        end_row: self.start_row,
                    }
                }
            }
            SelectionMode::Block => Self {
                pane_id: self.pane_id,
                alternate_screen: self.alternate_screen,
                mode: self.mode,
                start_col: self.start_col.min(self.end_col),
                start_row: self.start_row.min(self.end_row),
                end_col: self.start_col.max(self.end_col),
                end_row: self.start_row.max(self.end_row),
            },
        }
    }
}

impl TerminalPane {
    pub fn spawn(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        Self::spawn_in(config, cols, rows, proxy, wake_pending, None)
    }

    pub fn spawn_in(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
        cwd: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let (shell_program, shell_args) = crate::shell_startup::prepare(config)?;
        let (pty, performer, event_rx) =
            Pty::spawn_in(&shell_program, &shell_args, cols, rows, cwd, move || {
                if !wake_pending.swap(true, Ordering::AcqRel) {
                    if let Err(err) = proxy.send_event(VoltEvent::PtyData) {
                        eprintln!("volt-ui: failed to send PtyData event: {err}");
                    }
                }
            })?;
        if let Ok(mut p) = performer.lock() {
            p.set_scrollback_limit(config.terminal.scrollback_lines);
        }
        Ok(Self {
            pty,
            performer,
            event_rx,
            title: "~".to_string(),
            cwd: cwd
                .map(Path::to_path_buf)
                .or_else(|| std::env::current_dir().ok()),
            status: CommandStatus::default(),
            scroll_view_offset: 0,
            custom_title: None,
            read_only: false,
            task_run: None,
            task_results: Vec::new(),
        })
    }

    pub fn set_task_run(&mut self, next: Option<crate::tasks::TaskRun>) {
        if let Some(previous) = self.task_run.take() {
            crate::tasks::remember_run(&mut self.task_results, *previous);
        }
        self.task_run = next.map(Box::new);
    }

    /// OSC 7 may describe a remote machine. Only the owned local process is
    /// authoritative for spawning another local shell; never trust terminal text.
    pub fn local_cwd(&self) -> Option<PathBuf> {
        self.pty
            .child_pid()
            .and_then(crate::workspace_panel::local_process_cwd)
            .filter(|p| p.is_absolute())
    }

    pub fn display_title(&self) -> String {
        let raw = if let Some(custom) = self.custom_title.as_deref().map(str::trim) {
            if !custom.is_empty() {
                custom.to_string()
            } else {
                self.fallback_title()
            }
        } else {
            self.fallback_title()
        };
        if raw.trim().is_empty() {
            return "~".to_string();
        }
        truncate_last_chars(&raw, 20)
    }

    fn fallback_title(&self) -> String {
        pane_fallback_title(&self.title, self.cwd.as_deref(), self.status.running)
    }
}

fn pane_fallback_title(title: &str, cwd: Option<&Path>, running: bool) -> String {
    let title = title.trim();
    if running && !title.is_empty() && title != "~" {
        return title.to_string();
    }
    cwd.map(|p| {
        if let Some(home) = dirs::home_dir() {
            if p == home {
                return "~".to_string();
            }
            if let Ok(rel) = p.strip_prefix(&home) {
                if let Some(name) = rel.file_name() {
                    return format!("~/{}", name.to_string_lossy());
                }
                return "~".to_string();
            }
        }
        p.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| p.to_string_lossy().to_string())
    })
    .unwrap_or_else(|| "~".to_string())
}

/// Truncate `s` to its last `max_chars` characters, prefixed with `...` if
/// anything was cut. Operates on char (not byte) boundaries throughout —
/// slicing by `s.len()` (a byte count) at a fixed offset can land mid
/// multi-byte character and panic; this walks `char_indices()` instead.
fn truncate_last_chars(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        return s.to_string();
    }
    let skip = char_count - max_chars;
    match s.char_indices().nth(skip) {
        Some((byte_idx, _)) => format!("...{}", &s[byte_idx..]),
        None => s.to_string(),
    }
}

impl TerminalTab {
    pub fn spawn(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        Self::spawn_in(config, cols, rows, proxy, wake_pending, None)
    }

    pub fn spawn_in(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
        cwd: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let primary = TerminalPane::spawn_in(config, cols, rows, proxy, wake_pending, cwd)?;
        Ok(Self {
            tree: PaneTree::new(primary),
            custom_title: None,
            color: None,
        })
    }

    pub fn pane_count(&self) -> usize {
        self.tree.pane_count()
    }

    pub fn is_busy(&self) -> bool {
        self.tree
            .leaf_ids()
            .into_iter()
            .any(|id| self.tree.find_leaf(id).is_some_and(|p| p.status.running))
    }

    pub fn status(&self) -> volt_renderer::TabStatus {
        if self.is_busy() {
            volt_renderer::TabStatus::Running
        } else {
            self.active_pane().status.tab_status()
        }
    }

    pub fn active_pane(&self) -> &TerminalPane {
        self.tree.active_pane().expect("active pane missing")
    }

    pub fn active_pane_mut(&mut self) -> &mut TerminalPane {
        self.tree.active_pane_mut().expect("active pane missing")
    }

    pub fn display_title(&self, index: usize) -> String {
        tab_display_title(
            self.custom_title.as_deref(),
            &self.active_pane().display_title(),
            index,
        )
    }
}

fn tab_display_title(custom: Option<&str>, pane_title: &str, index: usize) -> String {
    if let Some(title) = custom.map(str::trim).filter(|title| !title.is_empty()) {
        return truncate_last_chars(title, 20);
    }
    if pane_title == "~" {
        format!("Tab {index}")
    } else {
        pane_title.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_row(grid: &mut Grid, row: usize, ch: char) {
        for col in 0..grid.cols {
            let mut cell = volt_core::cell::Cell::default();
            cell.set_char(ch);
            grid.put_char(col, row, cell);
        }
    }

    #[test]
    fn directory_title_tracks_cwd_without_overriding_custom_or_busy_titles() {
        assert_eq!(
            pane_fallback_title("old prompt title", Some(Path::new("/tmp/first")), false),
            "first"
        );
        assert_eq!(
            pane_fallback_title("old prompt title", Some(Path::new("/tmp/second")), false),
            "second"
        );
        assert_eq!(
            pane_fallback_title("nvim", Some(Path::new("/tmp/second")), true),
            "nvim"
        );
        assert_eq!(
            tab_display_title(Some("Pinned title"), "second", 1),
            "Pinned title"
        );
    }

    #[test]
    fn command_status_preserves_errors_and_never_guesses_success() {
        use volt_renderer::TabStatus;
        let mut status = CommandStatus::default();
        assert_eq!(status.tab_status(), TabStatus::Idle);
        status.observe_foreground(true);
        assert_eq!(status.tab_status(), TabStatus::Running);
        status.observe_foreground(false);
        assert_eq!(status.last_exit_code, None);
        assert_eq!(status.tab_status(), TabStatus::Idle);
        status.started();
        status.finished(2);
        assert_eq!(status.tab_status(), TabStatus::Failed);
        status.observe_foreground(false);
        assert_eq!(status.tab_status(), TabStatus::Failed);
        status.started();
        assert_eq!(status.last_exit_code, None);
        assert_eq!(status.tab_status(), TabStatus::Running);
        status.finished(0);
        assert_eq!(status.tab_status(), TabStatus::Idle);
        status.finished(-1);
        assert_eq!(status.last_exit_code, None);
        assert_eq!(status.tab_status(), TabStatus::Idle);
    }

    #[test]
    fn multiline_selection_survives_repeated_scroll_and_copies_offscreen_rows() {
        let mut grid = Grid::new(4, 3);
        grid.set_scrollback_limit(3);
        for (row, ch) in ['a', 'b', 'c'].into_iter().enumerate() {
            fill_row(&mut grid, row, ch);
        }
        for ch in ['d', 'e', 'f'] {
            grid.scroll_up(0, 2, 1);
            fill_row(&mut grid, 2, ch);
        }
        let mut sel =
            Selection::at_viewport(7, false, SelectionMode::Linear, &grid, 3, 1, 0).unwrap();
        assert!(sel.extend_to(7, false, &grid, 0, 1, 1));
        assert_eq!(
            sel.text(&grid).as_deref(),
            Some("aaa\nbbbb\ncccc\ndddd\nee")
        );
        assert_eq!(sel.visible_range(&grid, 3, 3), Some(((1, 0), (3, 2))));
        assert_eq!(sel.visible_range(&grid, 2, 3), Some(((0, 0), (3, 2))));
        assert_eq!(sel.visible_range(&grid, 0, 3), Some(((0, 0), (1, 1))));
        assert_eq!(
            sel.text(&grid).as_deref(),
            Some("aaa\nbbbb\ncccc\ndddd\nee")
        );

        grid.scroll_up(0, 2, 1); // Evicts the selected anchor, never copy wrong text.
        assert!(sel.text(&grid).is_none());
        assert!(sel.visible_range(&grid, 3, 3).is_none());
    }

    #[test]
    fn shift_extension_keeps_anchor_and_rejects_other_panes() {
        let mut grid = Grid::new(4, 3);
        grid.scroll_up(0, 2, 1);
        grid.scroll_up(0, 2, 1);
        let mut sel =
            Selection::at_viewport(1, false, SelectionMode::Linear, &grid, 0, 2, 1).unwrap();
        let anchor = (sel.start_col, sel.start_row);
        assert!(!sel.extend_to(2, false, &grid, 2, 0, 0));
        assert!(!sel.extend_to(1, true, &grid, 2, 0, 0));
        assert_eq!((sel.start_col, sel.start_row), anchor);
        assert!(sel.extend_to(1, false, &grid, 2, 0, 0));
        assert_eq!((sel.start_col, sel.start_row), anchor);
        assert_eq!(sel.normalized().start_row, 0);
    }

    #[test]
    fn block_selection_keeps_columns_and_survives_new_output() {
        let mut grid = Grid::new(4, 2);
        grid.set_scrollback_limit(3);
        fill_row(&mut grid, 0, 'a');
        fill_row(&mut grid, 1, 'b');
        grid.scroll_up(0, 1, 1);
        fill_row(&mut grid, 1, 'c');
        let mut sel =
            Selection::at_viewport(1, false, SelectionMode::Block, &grid, 1, 1, 0).unwrap();
        assert!(sel.extend_to(1, false, &grid, 0, 2, 1));
        assert_eq!(sel.text(&grid).as_deref(), Some("aa\nbb\ncc"));
        assert_eq!(sel.visible_range(&grid, 1, 2), Some(((1, 0), (2, 1))));
        grid.scroll_up(0, 1, 1);
        fill_row(&mut grid, 1, 'd');
        assert_eq!(sel.text(&grid).as_deref(), Some("aa\nbb\ncc"));
        assert_eq!(sel.visible_range(&grid, 2, 2), Some(((1, 0), (2, 1))));
    }

    #[test]
    fn truncate_last_chars_leaves_short_strings_alone() {
        assert_eq!(truncate_last_chars("short", 20), "short");
    }

    #[test]
    fn truncate_last_chars_cuts_at_char_boundary_not_byte_offset() {
        // 10 copies of a 3-byte CJK char = 30 bytes / 10 chars. The old
        // byte-offset slice (raw.len() - 20 = byte 10) landed mid-character
        // and panicked; this must instead cut at the 10th-from-end *char*.
        let s = "日".repeat(10);
        let out = truncate_last_chars(&s, 6);
        assert_eq!(out, format!("...{}", "日".repeat(6)));
    }

    #[test]
    fn truncate_last_chars_exact_length_is_unprefixed() {
        assert_eq!(truncate_last_chars("abcde", 5), "abcde");
    }

    #[test]
    fn tab_override_does_not_follow_active_pane_title() {
        assert_eq!(tab_display_title(Some("Project"), "shell A", 1), "Project");
        assert_eq!(tab_display_title(Some("Project"), "shell B", 1), "Project");
        assert_eq!(tab_display_title(None, "shell B", 1), "shell B");
    }
}
