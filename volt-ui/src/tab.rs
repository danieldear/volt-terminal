/// Per-tab and per-pane state for the Volt terminal.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use winit::event_loop::EventLoopProxy;

use volt_config::Config;
use volt_core::events::CoreEvent;
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

pub use volt_core::selection::{Selection, SelectionMode};

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

    /// Rearrange the focused process without changing tab metadata or focus.
    pub fn swap_active_pane(&mut self, target_id: usize) -> crate::pane_tree::RearrangeResult {
        self.tree.swap_active_pane(target_id)
    }

    /// Place the focused process left/above (`insert_before`) or right/below
    /// the destination. Caller recomputes layout and applies normal PTY resize.
    pub fn move_active_pane(
        &mut self,
        target_id: usize,
        direction: PaneSplitDirection,
        insert_before: bool,
    ) -> crate::pane_tree::RearrangeResult {
        self.tree
            .move_active_pane(target_id, direction, insert_before)
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
