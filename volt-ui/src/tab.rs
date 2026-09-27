/// Per-tab and per-pane state for the Volt terminal.
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use winit::event_loop::EventLoopProxy;

use volt_config::Config;
use volt_core::events::CoreEvent;
use volt_core::performer::Performer;
use volt_core::pty::Pty;

use crate::app::VoltEvent;
use crate::pane_tree::PaneTree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSplitDirection {
    Vertical,
    Horizontal,
}

pub struct TerminalPane {
    pub pty: Pty,
    pub performer: Arc<Mutex<Performer>>,
    pub event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    pub title: String,
    pub cwd: Option<PathBuf>,
    pub running: bool,
    /// How many lines the user has scrolled back into the scrollback buffer.
    /// 0 means the live view (bottom of output).
    pub scroll_view_offset: usize,
    /// Pane-scoped override from "Change Terminal Title...".
    /// Takes precedence over OSC title/cwd until cleared.
    pub custom_title: Option<String>,
    /// When true, keyboard/paste input to this pane's PTY is suppressed.
    /// Set via the context menu's "Terminal Read-only" toggle.
    pub read_only: bool,
}

pub struct TerminalTab {
    pub tree: PaneTree,
    /// Tab-scoped override; unlike a pane title it survives pane focus changes.
    pub custom_title: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum SelectionMode {
    Linear,
    Block,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub pane_id: usize,
    pub mode: SelectionMode,
    pub start_col: usize,
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl Selection {
    pub fn normalized(self) -> Self {
        match self.mode {
            SelectionMode::Linear => {
                if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
                    self
                } else {
                    Self {
                        pane_id: self.pane_id,
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
        let (pty, performer, event_rx) = Pty::spawn(
            &config.shell.program,
            &config.shell.args,
            cols,
            rows,
            move || {
                if !wake_pending.swap(true, Ordering::AcqRel) {
                    if let Err(err) = proxy.send_event(VoltEvent::PtyData) {
                        eprintln!("volt-ui: failed to send PtyData event: {err}");
                    }
                }
            },
        )?;
        if let Ok(mut p) = performer.lock() {
            p.set_scrollback_limit(config.terminal.scrollback_lines);
        }
        Ok(Self {
            pty,
            performer,
            event_rx,
            title: "~".to_string(),
            cwd: std::env::current_dir().ok(),
            running: false,
            scroll_view_offset: 0,
            custom_title: None,
            read_only: false,
        })
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
        let title = self.title.trim();
        if !title.is_empty() && title != "~" {
            return title.to_string();
        }
        self.cwd
            .as_deref()
            .map(|p| {
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
        let primary = TerminalPane::spawn(config, cols, rows, proxy, wake_pending)?;
        Ok(Self {
            tree: PaneTree::new(primary),
            custom_title: None,
        })
    }

    pub fn pane_count(&self) -> usize {
        self.tree.pane_count()
    }

    pub fn is_busy(&self) -> bool {
        self.tree
            .leaf_ids()
            .into_iter()
            .any(|id| self.tree.find_leaf(id).is_some_and(|p| p.running))
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
