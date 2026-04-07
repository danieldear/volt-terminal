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
}

pub struct TerminalTab {
    pub tree: PaneTree,
}

#[derive(Debug, Clone, Copy)]
pub struct Selection {
    pub pane_id: usize,
    pub start_col: usize,
    pub start_row: usize,
    pub end_col: usize,
    pub end_row: usize,
}

impl Selection {
    pub fn normalized(self) -> Self {
        if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
            self
        } else {
            Self {
                pane_id: self.pane_id,
                start_col: self.end_col,
                start_row: self.end_row,
                end_col: self.start_col,
                end_row: self.start_row,
            }
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
        Ok(Self {
            pty,
            performer,
            event_rx,
            title: "~".to_string(),
            cwd: None,
            running: false,
        })
    }

    pub fn display_title(&self) -> String {
        let title = self.title.trim();
        let raw = if !title.is_empty() && title != "~" {
            title.to_string()
        } else {
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
        };
        if raw.trim().is_empty() {
            return "~".to_string();
        }
        if raw.len() > 20 {
            format!("...{}", &raw[raw.len() - 20..])
        } else {
            raw
        }
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
        })
    }

    pub fn pane_count(&self) -> usize {
        self.tree.pane_count()
    }

    pub fn is_busy(&self) -> bool {
        self.tree.leaf_ids().into_iter().any(|id| {
            self.tree.find_leaf(id).is_some_and(|p| p.running)
        })
    }

    pub fn active_pane(&self) -> &TerminalPane {
        self.tree.active_pane().expect("active pane missing")
    }

    pub fn active_pane_mut(&mut self) -> &mut TerminalPane {
        self.tree.active_pane_mut().expect("active pane missing")
    }

    pub fn display_title(&self, index: usize) -> String {
        let _ = index;
        self.active_pane().display_title()
    }
}

