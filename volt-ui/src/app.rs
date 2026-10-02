use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActiveEventLoopExtMacOS, WindowExtMacOS};
use winit::window::{Window, WindowId};

use volt_config::{config_path_to_edit, sample_config_toml, Config, Theme, ThemeRegistry};
use volt_core::events::CoreEvent;
use volt_core::grid::Grid;
use volt_core::performer::MouseTrackingMode;

use volt_renderer::{Renderer, TabEntry};

#[cfg(target_os = "macos")]
use crate::display_link::DisplayLinkScheduler;
use crate::pane_tree::RemoveResult;
use crate::tab::{PaneSplitDirection, Selection, SelectionMode, TerminalPane, TerminalTab};
use crate::tab_layout::TabLayout;
use crate::workspace_panel::WorkspacePanel;

// ── user events (used to wake the event loop from background threads) ────────

#[derive(Debug, Clone)]
pub enum VoltEvent {
    /// PTY reader thread produced output — request a redraw.
    PtyData,
    WorkspaceUpdated,
    #[cfg(target_os = "macos")]
    NativeText {
        window_id: WindowId,
        text: String,
    },
    #[cfg(target_os = "macos")]
    /// Display link tick — redraw on display cadence when needed.
    DisplayLinkTick,
    #[cfg(target_os = "macos")]
    /// Request to open a new terminal window (used for native macOS tab creation).
    CreateNewWindow,
    #[cfg(target_os = "macos")]
    /// A native menu bar or context menu item was clicked. Carries the raw
    /// `muda::MenuId` string; resolved to a `menu::MenuAction` on receipt so
    /// this event type doesn't need to depend on `muda`'s types directly.
    Menu(String),
}

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(530);
const SELECTION_SCROLL_START_DELAY: Duration = Duration::from_millis(100);
const SELECTION_SCROLL_INTERVAL: Duration = Duration::from_millis(40);
/// Maximum delay between clicks for double/triple-click detection.
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(450);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionScrollDirection {
    Up,
    Down,
}

/// Move one retained row and extend the original anchor to the edge cell.
/// Kept separate from the window/event loop so repeated ticks are testable.
#[allow(clippy::too_many_arguments)]
fn advance_drag_selection(
    grid: &Grid,
    offset: &mut usize,
    selection: &mut Selection,
    pane_id: usize,
    alternate_screen: bool,
    col: usize,
    row: usize,
    direction: SelectionScrollDirection,
) -> bool {
    let old_offset = *offset;
    *offset = match direction {
        SelectionScrollDirection::Up => offset.saturating_add(1).min(grid.scrollback_len()),
        SelectionScrollDirection::Down => offset.saturating_sub(1),
    };
    if *offset == old_offset {
        return false;
    }
    if !selection.extend_to(pane_id, alternate_screen, grid, *offset, col, row) {
        *offset = old_offset;
        return false;
    }
    true
}

#[derive(Debug, Clone, Copy)]
struct DividerDrag {
    divider_id: usize,
    /// Physical pixel position where the drag began (x for vertical, y for horizontal).
    start_px: f32,
    /// Ratio of the divider at drag-start; new ratio is computed as `start_ratio + delta/span`.
    start_ratio: f32,
    is_vertical: bool,
}

#[derive(Debug, Clone, Copy)]
enum PaneFocusDirection {
    Left,
    Right,
    Up,
    Down,
}

// ── main window state ────────────────────────────────────────────────────────

struct MainState {
    id: WindowId,
    window: Arc<Window>,
    #[cfg(target_os = "macos")]
    _native_text: crate::native_text::NativeTextInput,
    renderer: Renderer,
    render_snapshot: Option<Grid>,
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    theme: Theme,
    modifiers: ModifiersState,
    left_shift_down: bool,
    right_shift_down: bool,
    left_control_down: bool,
    right_control_down: bool,
    left_super_down: bool,
    right_super_down: bool,
    config: Config,
    keybindings: crate::keybindings::Bindings,
    mouse_pos: (f32, f32),
    is_drag_selecting: bool,
    /// Armed only while a drag is held at the top/bottom of its originating pane.
    selection_scroll_deadline: Option<(SelectionScrollDirection, Instant)>,
    selection: Option<Selection>,
    pressed_mouse_button: Option<MouseButton>,
    last_reported_mouse_cell: Option<(usize, usize)>,
    redraw_pending: bool,
    /// Extra full redraw passes requested after a window resize settles.
    post_resize_redraws: u8,
    pending_damage_rows: Option<(usize, usize)>,
    force_full_redraw: bool,
    blink_state: bool,
    next_cursor_blink: Instant,
    #[cfg(target_os = "macos")]
    display_link: Option<DisplayLinkScheduler>,
    #[cfg(target_os = "macos")]
    last_known_native_tab_count: usize,
    proxy: EventLoopProxy<VoltEvent>,
    pty_wake_pending: Arc<AtomicBool>,
    divider_drag: Option<DividerDrag>,
    /// ID of the divider the mouse is currently hovering over (for visual highlight).
    divider_hover_id: Option<usize>,
    /// AI Chat Panel sidebar state.
    workspace_panel: WorkspacePanel,
    workspace_search: crate::workspace_search::SearchPalette,
    search_mouse_capture: Option<MouseButton>,
    /// Last left-click for double/triple-click detection: (time, pane, col, row).
    last_click: Option<(Instant, usize, usize, usize)>,
    /// Consecutive clicks at the same cell: 1 = cell, 2 = word, 3 = line.
    click_count: u8,
    #[cfg(target_os = "macos")]
    /// Last title handed to the native window, so idle ticks skip `set_title`.
    last_native_title: String,
    /// Time of the last left-click on empty tab-bar chrome (not a tab, not
    /// the + button), for double-click-to-maximize detection.
    last_tab_bar_click: Option<Instant>,
    /// Active Find / Change Tab Title / Change Terminal Title overlay, if
    /// any. While `Some`, keyboard input is captured by the prompt instead
    /// of being sent to the terminal.
    active_prompt: Option<crate::prompt::TextPrompt>,
    search_dirty: bool,
    search_target: Option<(usize, usize)>,
    last_search_refresh: Instant,
    /// Debug overlay toggled by the context menu's "Toggle Terminal
    /// Inspector" — grid size, cursor position, scrollback length.
    show_inspector: bool,
    /// Open theme editor. While `Some`, `theme` is its live working copy and
    /// keyboard input goes to the editor.
    theme_editor: Option<crate::theme_editor::ThemeEditor>,
}

impl MainState {
    /// Push the editor's view to the renderer. A window too small for the
    /// panel closes the editor instead of leaving an invisible modal.
    fn sync_theme_editor(&mut self) {
        self.renderer.theme_editor = self.theme_editor.as_ref().map(|e| e.view());
        if self.theme_editor.is_some() && self.renderer.theme_editor_layout().is_none() {
            self.cancel_theme_editor();
        }
        self.begin_redraw();
    }

    /// Close the editor and put back the theme it started from (or last saved).
    fn cancel_theme_editor(&mut self) {
        if let Some(editor) = self.theme_editor.take() {
            self.theme = editor.original().clone();
        }
        self.renderer.theme_editor = None;
        self.begin_redraw();
    }

    /// Apply an editor outcome to this window. A save needs the theme
    /// registry, so it is returned to `App` as `(name, overwrite)`.
    fn apply_editor_outcome(
        &mut self,
        outcome: crate::theme_editor::EditorOutcome,
    ) -> Option<(String, Option<String>)> {
        use crate::theme_editor::EditorOutcome as O;
        match outcome {
            O::Redraw => {}
            O::ThemeChanged => {
                if let Some(editor) = &self.theme_editor {
                    self.theme = editor.working().clone();
                }
            }
            O::Cancel => {
                self.cancel_theme_editor();
                return None;
            }
            O::Save { name, overwrite } => {
                self.sync_theme_editor();
                return Some((name, overwrite));
            }
        }
        self.sync_theme_editor();
        None
    }

    /// Keyboard input while the editor is open. Every key is consumed.
    fn theme_editor_key(
        &mut self,
        key: PhysicalKey,
        text: Option<&str>,
        command: bool,
        shift: bool,
        ctrl: bool,
    ) -> Option<(String, Option<String>)> {
        if command && key == PhysicalKey::Code(KeyCode::KeyV) {
            self.paste_clipboard();
            return None;
        }
        let editor = self.theme_editor.as_mut()?;
        let naming = editor.is_naming();
        let outcome = match key {
            PhysicalKey::Code(KeyCode::KeyS) if command => editor.save(shift),
            PhysicalKey::Code(KeyCode::Backspace) if command => editor.clear(),
            _ if command || ctrl => return None,
            PhysicalKey::Code(KeyCode::Escape) => editor.escape(),
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => editor.enter(),
            PhysicalKey::Code(KeyCode::Tab) if shift => editor.prev_field(),
            PhysicalKey::Code(KeyCode::Tab) => editor.next_field(),
            PhysicalKey::Code(KeyCode::Backspace) => editor.backspace(),
            PhysicalKey::Code(KeyCode::ArrowLeft) if naming => editor.move_cursor(-1),
            PhysicalKey::Code(KeyCode::ArrowRight) if naming => editor.move_cursor(1),
            PhysicalKey::Code(KeyCode::Home) => editor.cursor_to_edge(false),
            PhysicalKey::Code(KeyCode::End) => editor.cursor_to_edge(true),
            PhysicalKey::Code(KeyCode::ArrowUp | KeyCode::ArrowLeft) if !naming => {
                editor.prev_field()
            }
            PhysicalKey::Code(KeyCode::ArrowDown | KeyCode::ArrowRight) if !naming => {
                editor.next_field()
            }
            _ => match text {
                Some(text) => editor.type_text(text),
                None => return None,
            },
        };
        self.apply_editor_outcome(outcome)
    }

    fn sync_search(&mut self) {
        self.renderer.search_palette = self.workspace_search.view();
        if self.workspace_search.visible && self.renderer.search_layout().is_none() {
            self.workspace_search.close();
            self.renderer.search_palette = None;
        }
        self.begin_redraw();
    }
    fn toggle_search(&mut self) {
        if self.theme_editor.is_some() {
            return;
        }
        if self.workspace_search.visible {
            self.workspace_search.close();
        } else {
            let pane = self.active_tab().active_pane();
            let pid = pane.pty.child_pid();
            let cwd = pid
                .and_then(crate::workspace_panel::local_process_cwd)
                .or_else(|| pane.cwd.clone());
            self.workspace_search.open(pid, cwd);
            self.workspace_panel.focused = false;
            self.workspace_panel.drag_offset = None;
            self.active_prompt = None;
            self.search_dirty = false;
            self.search_target = None;
            self.is_drag_selecting = false;
            self.selection_scroll_deadline = None;
            self.divider_drag = None;
            self.pressed_mouse_button = None;
            self.selection = None;
        }
        self.sync_search();
    }
    fn activate_search_result(&mut self) {
        use crate::workspace_search::Action;
        let action = self
            .workspace_search
            .output
            .rows
            .get(self.workspace_search.selected)
            .map(|r| r.action.clone());
        if let Some(Action::Terminal {
            text,
            alternate,
            row,
        }) = action
        {
            let valid = {
                let pane = self.active_tab().active_pane();
                pane.performer.try_lock().ok().is_some_and(|p| {
                    let g = &p.grid;
                    let total = g.scrollback_len() + g.rows;
                    p.alternate_screen_active() == alternate && row < total && {
                        let now = if row < g.scrollback_len() {
                            g.row_text(g.scrollback_row(row))
                        } else {
                            g.row_text(g.row_cells(row - g.scrollback_len()))
                        };
                        now == text
                    }
                })
            };
            if valid {
                let query = self.workspace_search.input.text().trim().to_string();
                let (matches, truncated) = crate::prompt::find_matches_bounded(
                    &query,
                    std::iter::once((row, text.as_str())),
                    100,
                );
                let mut prompt =
                    crate::prompt::TextPrompt::new(crate::prompt::PromptKind::Find, &query);
                prompt.matches = matches;
                prompt.matches_truncated = truncated;
                self.workspace_search.close();
                self.active_prompt = Some(prompt);
                self.search_dirty = false;
                self.search_target = Some((self.active_tab, self.active_tab().tree.active_id));
                self.scroll_to_current_match();
            } else {
                self.workspace_search.output.status =
                    "Output changed or busy — edit the query to refresh".into();
            }
        } else {
            self.workspace_search.preview_selected(&self.proxy);
        }
        self.sync_search();
    }
    fn sync_workspace_card(&mut self) {
        self.renderer.workspace_card = self.workspace_panel.presentation_card();
        if let Some(card) = self.renderer.workspace_card.as_mut() {
            card.floating =
                self.config.workspace.layout == volt_config::config::WorkspaceLayout::Floating;
            card.position = self.workspace_panel.position;
        }
        if self.renderer.workspace_card.is_none() {
            // Auto-hidden cards must not capture keyboard input or clipboard paste.
            self.workspace_panel.focused = false;
            self.workspace_panel.hover = None;
        }
        if let Some(l) = self.renderer.workspace_card_layout() {
            if let Some(card) = self.renderer.workspace_card.as_mut() {
                self.workspace_panel.scroll = self
                    .workspace_panel
                    .scroll
                    .min(card.rows.len().saturating_sub(l.count));
                card.scroll = self.workspace_panel.scroll;
            }
        }
    }

    fn toggle_workspace_layout(&mut self) {
        let previous_grid = self.current_grid_size();
        use volt_config::config::WorkspaceLayout;
        self.config.workspace.layout = match self.config.workspace.layout {
            WorkspaceLayout::Docked => WorkspaceLayout::Floating,
            WorkspaceLayout::Floating => WorkspaceLayout::Docked,
        };
        self.workspace_panel.drag_offset = None;
        self.sync_workspace_card();
        if self.current_grid_size() != previous_grid {
            self.resize_all_tabs_to_current_grid();
        }
        self.begin_redraw();
    }

    fn toggle_workspace_panel(&mut self) {
        let previous_grid = self.current_grid_size();
        self.workspace_panel.drag_offset = None;
        self.workspace_panel.visible = !self.workspace_panel.visible;
        self.workspace_panel.focused = self.workspace_panel.visible;
        self.workspace_panel.hover = self.workspace_panel.visible.then_some(102);
        self.sync_workspace_card();
        if self.current_grid_size() != previous_grid {
            self.resize_all_tabs_to_current_grid();
        }
        self.begin_redraw();
    }

    fn activate_workspace_action(&mut self, id: usize) {
        if id == 103 {
            self.toggle_workspace_layout();
            return;
        }
        if id == 900 || (3000..3032).contains(&id) {
            let task = if id == 900 {
                self.workspace_panel.selected_task.clone().filter(|task| {
                    self.workspace_panel
                        .snapshot
                        .as_ref()
                        .is_some_and(|s| s.project.tasks.contains(task))
                })
            } else {
                self.workspace_panel
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.git_details.worktrees.get(id - 3000))
                    .filter(|w| !w.prunable)
                    .map(|w| crate::workspace_project::ProjectTask {
                        label: format!("Worktree: {}", w.branch),
                        cwd: w.path.clone(),
                        definition: String::new(),
                        argv: std::iter::once(self.config.shell.program.clone())
                            .chain(self.config.shell.args.clone())
                            .collect(),
                    })
            };
            if let Some(task) = task {
                let mut config = self.config.clone();
                let shell = config.shell.program.clone();
                config.shell.program = "/bin/sh".into();
                config.shell.args = crate::workspace_project::task_shell_args(&task, &shell);
                let (cols, rows) = self.current_grid_size();
                if let Ok(mut tab) = TerminalTab::spawn(
                    &config,
                    cols as u16,
                    rows as u16,
                    self.proxy.clone(),
                    Arc::clone(&self.pty_wake_pending),
                ) {
                    tab.custom_title = Some(task.label);
                    self.tabs.push(tab);
                    self.active_tab = self.tabs.len() - 1;
                    self.workspace_panel.focused = false;
                    self.resize_all_tabs_to_current_grid();
                } else if let Some(snapshot) = self.workspace_panel.snapshot.as_mut() {
                    snapshot.error = Some("Could not start task terminal".into());
                }
            }
        } else if id == 6000 {
            if let Some(crate::workspace_git::PrState::Found(pr)) =
                &self.workspace_panel.pull_request
            {
                if crate::workspace_git::valid_pr_url(&pr.url) {
                    let url = pr.url.clone();
                    std::thread::spawn(move || {
                        #[cfg(target_os = "macos")]
                        let program = "open";
                        #[cfg(not(target_os = "macos"))]
                        let program = "xdg-open";
                        let _ = std::process::Command::new(program).arg(url).status();
                    });
                }
            }
        } else {
            let previous_grid = self.current_grid_size();
            self.workspace_panel.activate(id);
            self.sync_workspace_card();
            if previous_grid != self.current_grid_size() {
                self.resize_all_tabs_to_current_grid();
            }
        }
    }

    fn workspace_hit(&self) -> Option<usize> {
        self.renderer.workspace_card_layout().and_then(|l| {
            self.renderer
                .workspace_card
                .as_ref()
                .and_then(|c| l.hit(self.mouse_pos.0, self.mouse_pos.1, c))
        })
    }

    fn workspace_contains_pointer(&self) -> bool {
        self.renderer
            .workspace_card_layout()
            .is_some_and(|l| l.contains(self.mouse_pos.0, self.mouse_pos.1))
    }

    fn layout_tab_count(&self) -> usize {
        self.tabs.len().max(1)
    }

    fn show_custom_tab_bar(&self) -> bool {
        // On macOS, respect the native_tabs config option.
        #[cfg(target_os = "macos")]
        return !self.config.appearance.native_tabs;
        #[cfg(not(target_os = "macos"))]
        true
    }

    fn current_tab_bar_height(&self) -> f32 {
        self.renderer
            .top_offset_for_tab_count(self.layout_tab_count())
    }

    fn current_content_top_offset(&self) -> f32 {
        self.renderer
            .content_top_offset_for_tab_count(self.layout_tab_count())
    }

    fn current_grid_size(&self) -> (usize, usize) {
        self.renderer
            .grid_size_for_tab_count(self.layout_tab_count())
    }

    fn resize_all_tabs_to_current_grid(&mut self) {
        // Reflow can change logical row identities; a retained absolute-row
        // selection must not silently point at different text afterward.
        self.selection = None;
        self.is_drag_selecting = false;
        self.selection_scroll_deadline = None;
        self.sync_workspace_card();
        let (total_cols, total_rows) = self.current_grid_size();
        for tab in &mut self.tabs {
            let rects = tab.tree.layout(total_cols, total_rows);
            for rect in rects {
                if let Some(pane) = tab.tree.find_leaf_mut(rect.id) {
                    if let Err(err) = pane.pty.resize(rect.cols as u16, rect.rows as u16) {
                        eprintln!("volt-ui: failed to resize PTY: {err}");
                    }
                    match pane.performer.lock() {
                        Ok(mut performer) => performer.resize(rect.cols, rect.rows),
                        Err(err) => {
                            eprintln!("volt-ui: failed to lock performer for resize: {err}")
                        }
                    }
                    // Return to live output after resize — the scrollback is still
                    // intact but the user should see the current content first.
                    pane.scroll_view_offset = 0;
                }
            }
        }
    }

    fn shift_down(&self) -> bool {
        self.modifiers.shift_key() || self.left_shift_down || self.right_shift_down
    }

    fn ctrl_down(&self) -> bool {
        self.modifiers.control_key() || self.left_control_down || self.right_control_down
    }

    fn super_down(&self) -> bool {
        self.modifiers.super_key() || self.left_super_down || self.right_super_down
    }

    fn active_tab_mut(&mut self) -> &mut TerminalTab {
        &mut self.tabs[self.active_tab]
    }

    fn active_tab(&self) -> &TerminalTab {
        &self.tabs[self.active_tab]
    }

    #[cfg(target_os = "macos")]
    fn set_window_title_cached(&mut self, title: &str) {
        if self.last_native_title != title {
            self.last_native_title = title.to_string();
            self.window.set_title(title);
        }
    }

    #[cfg(target_os = "macos")]
    fn sync_native_window_title(&mut self) {
        let title = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.display_title(self.active_tab + 1))
            .unwrap_or_else(|| "~".to_string());
        self.set_window_title_cached(&title);
    }

    fn active_pane_mut(&mut self) -> &mut TerminalPane {
        self.active_tab_mut().active_pane_mut()
    }

    fn active_tab_dividers(&self) -> Vec<crate::pane_tree::DividerInfo> {
        let tab = &self.tabs[self.active_tab];
        let (total_cols, total_rows) = self.current_grid_size();
        let phys_pad = self.renderer.padding * self.renderer.scale_factor;
        let content_top = self.current_content_top_offset();
        let scale = self.renderer.scale_factor;
        let drag_id = self.divider_drag.map(|d| d.divider_id);
        let divider_opacity = self.config.appearance.divider_opacity;
        let phys_right = self.renderer.surface_width() as f32
            - phys_pad
            - self.renderer.workspace_card_reserved_width();
        let phys_bottom = self.renderer.surface_height() as f32;
        tab.tree.dividers_with_ids(
            total_cols,
            total_rows,
            self.renderer.cell_width,
            self.renderer.cell_height,
            phys_pad,
            content_top,
            phys_right,
            phys_bottom,
            scale,
            self.divider_hover_id,
            drag_id,
            divider_opacity,
        )
    }

    fn move_focus_in_direction(&mut self, direction: PaneFocusDirection) -> bool {
        let (total_cols, total_rows) = self.current_grid_size();
        let tab = self.active_tab();
        let active_id = tab.tree.active_id;
        let rects = tab.tree.layout(total_cols, total_rows);
        let Some(active) = rects.iter().find(|r| r.id == active_id).copied() else {
            return false;
        };
        let active_center_col = active.col as isize + active.cols as isize / 2;
        let active_center_row = active.row as isize + active.rows as isize / 2;
        let mut best: Option<(usize, isize, isize)> = None;

        for rect in rects.into_iter().filter(|r| r.id != active_id) {
            let center_col = rect.col as isize + rect.cols as isize / 2;
            let center_row = rect.row as isize + rect.rows as isize / 2;
            let candidate = match direction {
                PaneFocusDirection::Left => {
                    if rect.col + rect.cols > active.col {
                        None
                    } else {
                        Some((
                            active.col as isize - (rect.col + rect.cols) as isize,
                            (active_center_row - center_row).abs(),
                        ))
                    }
                }
                PaneFocusDirection::Right => {
                    if rect.col < active.col + active.cols {
                        None
                    } else {
                        Some((
                            rect.col as isize - (active.col + active.cols) as isize,
                            (active_center_row - center_row).abs(),
                        ))
                    }
                }
                PaneFocusDirection::Up => {
                    if rect.row + rect.rows > active.row {
                        None
                    } else {
                        Some((
                            active.row as isize - (rect.row + rect.rows) as isize,
                            (active_center_col - center_col).abs(),
                        ))
                    }
                }
                PaneFocusDirection::Down => {
                    if rect.row < active.row + active.rows {
                        None
                    } else {
                        Some((
                            rect.row as isize - (active.row + active.rows) as isize,
                            (active_center_col - center_col).abs(),
                        ))
                    }
                }
            };
            let Some((primary, secondary)) = candidate else {
                continue;
            };
            if best
                .map(|(_, best_primary, best_secondary)| {
                    primary < best_primary
                        || (primary == best_primary && secondary < best_secondary)
                })
                .unwrap_or(true)
            {
                best = Some((rect.id, primary, secondary));
            }
        }

        let Some((target_id, _, _)) = best else {
            return false;
        };
        self.active_tab_mut().tree.active_id = target_id;
        self.selection = None;
        true
    }

    fn adjust_font_size(&mut self, delta: f32) -> bool {
        let mut next = self.config.clone();
        let new_size = (next.font.size + delta).clamp(6.0, 72.0);
        if (new_size - next.font.size).abs() < f32::EPSILON {
            return false;
        }
        next.font.size = (new_size * 10.0).round() / 10.0;
        self.apply_config(&next);
        true
    }

    fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
            self.selection = None;
            self.is_drag_selecting = false;
            self.selection_scroll_deadline = None;
            self.divider_drag = None;
            self.divider_hover_id = None;
            self.last_reported_mouse_cell = None;
        }
    }

    fn cycle_tab_next(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        let next = (self.active_tab + 1) % self.tabs.len();
        self.switch_tab(next);
    }

    fn cycle_tab_prev(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        let prev = if self.active_tab == 0 {
            self.tabs.len() - 1
        } else {
            self.active_tab - 1
        };
        self.switch_tab(prev);
    }

    fn run_keybinding(&mut self, action: volt_config::keybindings::Action) {
        use volt_config::keybindings::Action as A;
        match action {
            A::Copy => {
                self.copy_selection();
            }
            A::Paste => self.paste_clipboard(),
            A::Find => self.open_find_prompt(),
            A::SearchWorkspace => self.toggle_search(),
            A::ToggleWorkspace => self.toggle_workspace_panel(),
            A::ToggleWorkspaceLayout => self.toggle_workspace_layout(),
            A::NewTab => {
                #[cfg(target_os = "macos")]
                if self.config.appearance.native_tabs {
                    let _ = self.proxy.send_event(VoltEvent::CreateNewWindow);
                    return;
                }
                self.new_tab();
            }
            A::NextTab => self.cycle_tab_next(),
            A::PreviousTab => self.cycle_tab_prev(),
            A::PreviousPrompt => self.jump_prompt(true),
            A::NextPrompt => self.jump_prompt(false),
            A::SplitRight | A::SplitLeft => {
                self.split_active_tab_positioned(
                    PaneSplitDirection::Vertical,
                    action == A::SplitLeft,
                );
            }
            A::SplitDown | A::SplitUp => {
                self.split_active_tab_positioned(
                    PaneSplitDirection::Horizontal,
                    action == A::SplitUp,
                );
            }
            A::FocusLeft => {
                self.move_focus_in_direction(PaneFocusDirection::Left);
            }
            A::FocusRight => {
                self.move_focus_in_direction(PaneFocusDirection::Right);
            }
            A::FocusUp => {
                self.move_focus_in_direction(PaneFocusDirection::Up);
            }
            A::FocusDown => {
                self.move_focus_in_direction(PaneFocusDirection::Down);
            }
            A::IncreaseFontSize => {
                self.adjust_font_size(1.0);
            }
            A::DecreaseFontSize => {
                self.adjust_font_size(-1.0);
            }
            A::ToggleFullscreen => {
                let next = self
                    .window
                    .fullscreen()
                    .is_none()
                    .then_some(winit::window::Fullscreen::Borderless(None));
                self.window.set_fullscreen(next);
            }
            A::OpenConfig => open_config_in_editor(),
            A::Ignore | A::Unbind => return,
            A::NewWindow | A::ReloadConfig | A::Quit | A::ClosePane | A::CustomizeTheme => {
                unreachable!("App-owned action")
            }
        }
        self.begin_redraw();
    }

    fn new_tab(&mut self) {
        let (cols, rows) = self.current_grid_size();
        let cwd = self.active_tab().active_pane().local_cwd();
        if let Ok(tab) = TerminalTab::spawn_in(
            &self.config,
            cols as u16,
            rows as u16,
            self.proxy.clone(),
            Arc::clone(&self.pty_wake_pending),
            cwd.as_deref(),
        ) {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
            self.resize_all_tabs_to_current_grid();
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(idx);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.resize_all_tabs_to_current_grid();
    }

    fn split_active_tab(&mut self, direction: PaneSplitDirection) -> bool {
        self.split_active_tab_positioned(direction, false)
    }

    /// Split the active pane. `insert_before` places the new pane to the
    /// left/above the original (Split Left/Up) instead of the default
    /// right/below (Split Right/Down).
    fn split_active_tab_positioned(
        &mut self,
        direction: PaneSplitDirection,
        insert_before: bool,
    ) -> bool {
        let (total_cols, total_rows) = self.current_grid_size();
        let active_id = self.active_tab().tree.active_id;
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        let active_rect = rects.iter().find(|r| r.id == active_id).copied();
        let Some(active_rect) = active_rect else {
            return false;
        };
        if match direction {
            PaneSplitDirection::Vertical => active_rect.cols < 3,
            PaneSplitDirection::Horizontal => active_rect.rows < 3,
        } {
            return false;
        }
        let (pane_cols, pane_rows) = match (active_rect, direction) {
            (r, PaneSplitDirection::Vertical) => ((r.cols / 2).max(1), r.rows),
            (r, PaneSplitDirection::Horizontal) => (r.cols, (r.rows / 2).max(1)),
        };
        let config = self.config.clone();
        let proxy = self.proxy.clone();
        let wake = Arc::clone(&self.pty_wake_pending);
        let cwd = self.active_tab().active_pane().local_cwd();
        let Ok(new_pane) = TerminalPane::spawn_in(
            &config,
            pane_cols as u16,
            pane_rows as u16,
            proxy,
            wake,
            cwd.as_deref(),
        ) else {
            return false;
        };
        self.active_tab_mut()
            .tree
            .split_positioned(active_id, direction, new_pane, insert_before);
        self.resize_all_tabs_to_current_grid();
        self.selection = None;
        true
    }

    /// Close the active pane, and if it was the tab's last pane, close the
    /// tab too. Returns `true` if the window itself should now close (this
    /// was the last tab in this window). Shared by the Cmd+W shortcut and
    /// the File > Close Tab menu action so both stay in sync.
    fn close_active_pane_or_tab(&mut self) -> bool {
        self.divider_drag = None;
        let active_id = self.active_tab().tree.active_id;
        let tab_idx = self.active_tab;
        let should_close_tab = self.remove_pane_from_tab(tab_idx, active_id);
        let close_window = if should_close_tab {
            if self.tabs.len() > 1 {
                self.close_tab(tab_idx);
                false
            } else {
                // Last pane in the last tab of this window.
                #[cfg(target_os = "macos")]
                {
                    true
                }
                #[cfg(not(target_os = "macos"))]
                {
                    false
                }
            }
        } else {
            false
        };
        self.begin_redraw();
        close_window
    }

    /// Remove a pane from the given tab by id. Returns `true` if the tab itself
    /// should now be closed (was the last pane).  On successful removal the
    /// surviving panes are immediately resized so TUI apps see the correct geometry.
    fn remove_pane_from_tab(&mut self, tab_idx: usize, pane_id: usize) -> bool {
        if tab_idx >= self.tabs.len() {
            return false;
        }
        match self.tabs[tab_idx].tree.remove(pane_id) {
            RemoveResult::NotFound => false,
            RemoveResult::RemovedLastLeaf => true,
            RemoveResult::Removed => {
                // Cancel any in-progress divider drag — the topology just changed.
                self.divider_drag = None;
                // Immediately resize all surviving panes so PTYs know their new geometry.
                self.resize_all_tabs_to_current_grid();
                // Invalidate any selection that pointed at the removed pane.
                if self.selection.is_some_and(|s| {
                    self.tabs
                        .get(tab_idx)
                        .is_none_or(|t| t.tree.find_leaf(s.pane_id).is_none())
                }) {
                    self.selection = None;
                }
                self.last_reported_mouse_cell = None;
                false
            }
        }
    }

    fn pane_cell_from_global_cell(&self, col: usize, row: usize) -> Option<(usize, usize, usize)> {
        let (total_cols, total_rows) = self.current_grid_size();
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        for rect in &rects {
            if col >= rect.col
                && col < rect.col + rect.cols
                && row >= rect.row
                && row < rect.row + rect.rows
            {
                return Some((rect.id, col - rect.col, row - rect.row));
            }
        }
        None
    }

    fn blit_grid(dst: &mut Grid, src: &Grid, dst_col: usize, dst_row: usize) {
        if dst_col >= dst.cols || dst_row >= dst.rows {
            return;
        }
        let rows = src.rows.min(dst.rows.saturating_sub(dst_row));
        let cols = src.cols.min(dst.cols.saturating_sub(dst_col));
        // Row-slice memcpys: this runs while holding the performer lock, so it
        // must be fast or it stalls the PTY parse thread.
        for row in 0..rows {
            dst.copy_text_from_grid(dst_row + row, dst_col, src, &src.row_cells(row)[..cols]);
        }
    }

    /// Blit `src` into `dst` while applying a scrollback view offset.
    /// The top `offset` rows of the destination pane area are filled from the
    /// scrollback buffer; the remaining rows show the beginning of the live grid.
    fn blit_grid_with_scrollback(
        dst: &mut Grid,
        src: &Grid,
        dst_col: usize,
        dst_row: usize,
        pane_rows: usize,
        offset: usize,
    ) {
        if dst_col >= dst.cols || dst_row >= dst.rows {
            return;
        }
        let cols = src.cols.min(dst.cols.saturating_sub(dst_col));
        let rows = pane_rows.min(dst.rows.saturating_sub(dst_row));

        let sb_len = src.scrollback_len();
        // Number of rows sourced from scrollback vs. the live grid.
        let sb_rows = offset.min(rows);
        let grid_rows = rows - sb_rows;
        // Index into scrollback of the first row to show.
        let sb_start = sb_len.saturating_sub(offset);

        for r in 0..sb_rows {
            let sb_idx = sb_start + r;
            if sb_idx >= sb_len {
                break;
            }
            dst.copy_text_from_grid(
                dst_row + r,
                dst_col,
                src,
                &src.scrollback_row(sb_idx)[..cols],
            );
        }
        for r in 0..grid_rows.min(src.rows) {
            dst.copy_text_from_grid(
                dst_row + sb_rows + r,
                dst_col,
                src,
                &src.row_cells(r)[..cols],
            );
        }
    }

    fn build_render_grid_for_active_tab(&mut self) -> Option<(Grid, bool)> {
        let (total_cols, total_rows) = self.current_grid_size();
        let mut out = self
            .render_snapshot
            .take()
            .unwrap_or_else(|| Grid::new(total_cols, total_rows));
        out.prepare_for_snapshot(total_cols, total_rows);
        let tab = self.active_tab();
        let active_id = tab.tree.active_id;
        let rects = tab.tree.layout(total_cols, total_rows);

        let mut active_cursor_visible = false;

        for rect in &rects {
            let Some(pane) = tab.tree.find_leaf(rect.id) else {
                continue;
            };
            // Blit directly while holding the lock — avoids cloning the grid
            // (and its scrollback buffer), which is expensive under heavy output.
            let performer = pane.performer.lock().ok()?;
            let grid = &performer.grid;
            let offset = pane.scroll_view_offset.min(grid.scrollback_len());
            if offset == 0 {
                Self::blit_grid(&mut out, grid, rect.col, rect.row);
            } else {
                Self::blit_grid_with_scrollback(
                    &mut out, grid, rect.col, rect.row, rect.rows, offset,
                );
            }
            // Only show the active cursor in the live view; when scrolled back,
            // hide it because the user is viewing history.
            if rect.id == active_id && offset == 0 {
                active_cursor_visible = performer.cursor_visible;
                out.cursor_col =
                    (rect.col + grid.cursor_col).min(rect.col + rect.cols.saturating_sub(1));
                out.cursor_row =
                    (rect.row + grid.cursor_row).min(rect.row + rect.rows.saturating_sub(1));
            }
        }

        Some((out, active_cursor_visible))
    }

    /// Apply settings other than the theme, which callers resolve through
    /// the theme registry (font-size changes must not reset it).
    fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
        self.keybindings = crate::keybindings::Bindings::compile(&config.keybindings);
        self.window
            .set_transparent(config.appearance.transparent_enabled());
        self.window.set_blur(config.appearance.blur_enabled());
        let sc = self.renderer.scale_factor;
        self.renderer.font_family = config.font.family.clone();
        self.renderer.padding = config.appearance.padding as f32;
        self.renderer.line_height = config.appearance.line_height;
        self.renderer
            .set_background_opacity(config.appearance.effective_opacity());
        self.renderer.cursor_style = config.appearance.cursor_style;
        self.renderer.update_scale(sc, config.font.size);
        self.resize_all_tabs_to_current_grid();
        if self.theme_editor.is_some() {
            self.sync_theme_editor();
        }
        self.begin_redraw();
    }

    /// Handle a left-click within the tab bar. Returns `true` if it hit a
    /// real target (a tab, its close button, or the + button); `false`
    /// means it landed on empty chrome, which the caller treats as a
    /// double-click-to-maximize candidate.
    fn handle_click(&mut self, mx: f32, my: f32) -> bool {
        if !self.show_custom_tab_bar() {
            return false;
        }
        let sc = self.renderer.scale_factor;
        let sw = self.window.inner_size().width as f32;
        let tl = TabLayout::compute(sw, self.current_tab_bar_height(), self.tabs.len(), sc);
        if let Some(i) = tl.hit_tab(mx, my, self.tabs.len()) {
            if tl.hit_close(mx, my, i) {
                self.close_tab(i);
            } else {
                self.switch_tab(i);
            }
            true
        } else if tl.hit_plus(mx, my) {
            self.new_tab();
            true
        } else {
            false
        }
    }

    fn queue_redraw(&mut self) {
        if self.redraw_pending {
            return;
        }
        self.redraw_pending = true;
        #[cfg(target_os = "macos")]
        {
            if let Some(display_link) = &self.display_link {
                display_link.request_redraw();
            }
        }
        self.window.request_redraw();
    }

    fn begin_redraw(&mut self) {
        self.mark_full_redraw();
        self.queue_redraw();
    }

    fn mark_full_redraw(&mut self) {
        self.pending_damage_rows = None;
        self.force_full_redraw = true;
    }

    fn mark_partial_redraw(&mut self, damaged_rows: Option<(usize, usize)>) {
        if self.force_full_redraw {
            return;
        }
        match damaged_rows {
            Some((start, end)) => {
                self.pending_damage_rows = Some(match self.pending_damage_rows {
                    Some((cur_start, cur_end)) => (cur_start.min(start), cur_end.max(end)),
                    None => (start, end),
                });
            }
            None => self.mark_full_redraw(),
        }
    }

    fn take_render_damage_rows(&mut self) -> Option<(usize, usize)> {
        if self.force_full_redraw {
            self.force_full_redraw = false;
            self.pending_damage_rows = None;
            None
        } else {
            self.pending_damage_rows.take()
        }
    }

    fn bump_cursor_blink(&mut self) {
        if self.config.appearance.cursor_blink {
            self.blink_state = true;
            self.next_cursor_blink = Instant::now() + CURSOR_BLINK_INTERVAL;
        }
    }

    fn effective_cursor_visible(&self, base_visible: bool) -> bool {
        if !base_visible {
            return false;
        }
        if self.config.appearance.cursor_blink {
            self.blink_state
        } else {
            true
        }
    }

    fn global_cell_from_mouse(&self, mx: f32, my: f32) -> Option<(usize, usize)> {
        let phys_pad = self.renderer.padding * self.renderer.scale_factor;
        let grid_x = mx - phys_pad;
        let grid_y = my - self.current_content_top_offset() - phys_pad;
        if grid_x < 0.0 || grid_y < 0.0 {
            return None;
        }
        let (cols, rows) = self.current_grid_size();
        let col = (grid_x / self.renderer.cell_width).floor() as usize;
        let row = (grid_y / self.renderer.cell_height).floor() as usize;
        if col >= cols || row >= rows {
            return None;
        }
        Some((col, row))
    }

    fn pane_cell_from_mouse(&self, mx: f32, my: f32) -> Option<(usize, usize, usize)> {
        let (col, row) = self.global_cell_from_mouse(mx, my)?;
        self.pane_cell_from_global_cell(col, row)
    }

    fn update_selection_end(&mut self, mx: f32, my: f32) {
        let Some((pane_id, col, row)) = self.pane_cell_from_mouse(mx, my).or_else(|| {
            self.selection_scroll_target()
                .map(|(_, pane_id, col, row)| (pane_id, col, row))
        }) else {
            return;
        };
        let Some(pane) = self.tabs[self.active_tab].tree.find_leaf(pane_id) else {
            return;
        };
        let Some((absolute_row, origin, alternate_screen)) =
            pane.performer.lock().ok().and_then(|p| {
                let grid = &p.grid;
                Some((
                    grid.viewport_absolute_row(pane.scroll_view_offset, row)?,
                    grid.scrollback_origin(),
                    p.alternate_screen_active(),
                ))
            })
        else {
            return;
        };
        if let Some(sel) = &mut self.selection {
            if sel.pane_id != pane_id
                || sel.alternate_screen != alternate_screen
                || sel.start_row < origin
            {
                return;
            }
            sel.end_col = col;
            sel.end_row = absolute_row;
            self.begin_redraw();
        }
    }

    /// The pointer may be outside the terminal's vertical bounds during a
    /// drag, so derive the edge from the selection's pane geometry rather than
    /// requiring `pane_cell_from_mouse` to return a cell.
    fn selection_scroll_target(&self) -> Option<(SelectionScrollDirection, usize, usize, usize)> {
        if !self.is_drag_selecting {
            return None;
        }
        let pane_id = self.selection?.pane_id;
        let (cols, rows) = self.current_grid_size();
        let rect = self
            .active_tab()
            .tree
            .layout(cols, rows)
            .into_iter()
            .find(|rect| rect.id == pane_id)?;
        let pad = self.renderer.padding * self.renderer.scale_factor;
        let cell_width = self.renderer.cell_width;
        let cell_height = self.renderer.cell_height;
        let left = pad + rect.col as f32 * cell_width;
        let right = left + rect.cols as f32 * cell_width;
        let top = self.current_content_top_offset() + pad + rect.row as f32 * cell_height;
        let bottom = top + rect.rows as f32 * cell_height;
        let (mx, my) = self.mouse_pos;
        if mx < left || mx >= right {
            return None;
        }
        let direction = selection_scroll_direction(my, top, bottom, cell_height)?;
        let col = ((mx - left) / cell_width).floor() as usize;
        let row = match direction {
            SelectionScrollDirection::Up => 0,
            SelectionScrollDirection::Down => rect.rows - 1,
        };
        Some((direction, pane_id, col.min(rect.cols - 1), row))
    }

    /// The event loop calls this only while idle. No recurring wake-up is
    /// scheduled outside an active drag at an edge.
    fn tick_selection_auto_scroll(&mut self) -> Option<Instant> {
        let Some((direction, pane_id, col, row)) = self.selection_scroll_target() else {
            self.selection_scroll_deadline = None;
            return None;
        };
        let now = Instant::now();
        if self
            .selection_scroll_deadline
            .is_none_or(|(armed_direction, _)| armed_direction != direction)
        {
            self.selection_scroll_deadline = Some((direction, now + SELECTION_SCROLL_START_DELAY));
        }
        let (_, deadline) = self.selection_scroll_deadline?;
        if now >= deadline {
            let mut scrolled = false;
            if let (Some(pane), Some(selection)) = (
                self.tabs[self.active_tab].tree.find_leaf_mut(pane_id),
                self.selection.as_mut(),
            ) {
                if let Ok(performer) = pane.performer.lock() {
                    scrolled = advance_drag_selection(
                        &performer.grid,
                        &mut pane.scroll_view_offset,
                        selection,
                        pane_id,
                        performer.alternate_screen_active(),
                        col,
                        row,
                        direction,
                    );
                }
            }
            if scrolled {
                self.begin_redraw();
            }
            self.selection_scroll_deadline = Some((direction, now + SELECTION_SCROLL_INTERVAL));
        }
        self.selection_scroll_deadline.map(|(_, deadline)| deadline)
    }

    /// Cell at a displayed (viewport) position, accounting for the pane's
    /// scrollback view offset — mirrors `blit_grid_with_scrollback`.
    fn displayed_cell(
        grid: &Grid,
        offset: usize,
        col: usize,
        row: usize,
    ) -> Option<&volt_core::cell::Cell> {
        if col >= grid.cols || row >= grid.rows {
            return None;
        }
        let sb_rows = offset.min(grid.rows);
        if row < sb_rows {
            let sb_len = grid.scrollback_len();
            let idx = sb_len.saturating_sub(offset) + row;
            if idx >= sb_len {
                return None;
            }
            Some(grid.scrollback_cell(idx, col))
        } else {
            Some(grid.cell(col, row - sb_rows))
        }
    }

    /// Expand a double-click into a word selection on the displayed row.
    fn word_selection(&self, pane_id: usize, col: usize, row: usize) -> Option<Selection> {
        let tab = self.tabs.get(self.active_tab)?;
        let pane = tab.tree.find_leaf(pane_id)?;
        let performer = pane.performer.lock().ok()?;
        let grid = &performer.grid;
        let offset = pane.scroll_view_offset.min(grid.scrollback_len());
        let class =
            char_select_class(grid.cell_char(Self::displayed_cell(grid, offset, col, row)?));
        let mut start = col;
        while start > 0 {
            match Self::displayed_cell(grid, offset, start - 1, row) {
                Some(cell) if char_select_class(grid.cell_char(cell)) == class => start -= 1,
                _ => break,
            }
        }
        let mut end = col;
        while end + 1 < grid.cols {
            match Self::displayed_cell(grid, offset, end + 1, row) {
                Some(cell) if char_select_class(grid.cell_char(cell)) == class => end += 1,
                _ => break,
            }
        }
        Some(Selection {
            pane_id,
            alternate_screen: performer.alternate_screen_active(),
            mode: SelectionMode::Linear,
            start_col: start,
            start_row: grid.viewport_absolute_row(offset, row)?,
            end_col: end,
            end_row: grid.viewport_absolute_row(offset, row)?,
        })
    }

    /// Expand a triple-click into a whole-row selection.
    fn line_selection(&self, pane_id: usize, row: usize) -> Option<Selection> {
        let tab = self.tabs.get(self.active_tab)?;
        let pane = tab.tree.find_leaf(pane_id)?;
        let performer = pane.performer.lock().ok()?;
        let grid = &performer.grid;
        let cols = grid.cols;
        let absolute_row = grid.viewport_absolute_row(pane.scroll_view_offset, row)?;
        Some(Selection {
            pane_id,
            alternate_screen: performer.alternate_screen_active(),
            mode: SelectionMode::Linear,
            start_col: 0,
            start_row: absolute_row,
            end_col: cols.saturating_sub(1),
            end_row: absolute_row,
        })
    }

    fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let tab = self.tabs.get(self.active_tab)?;
        let pane = tab.tree.find_leaf(sel.pane_id)?;
        let performer = pane.performer.lock().ok()?;
        if sel.alternate_screen != performer.alternate_screen_active() {
            return None;
        }
        sel.text(&performer.grid)
    }

    fn copy_selection(&mut self) -> bool {
        let Some(text) = self.selected_text() else {
            return false;
        };
        #[cfg(target_os = "macos")]
        {
            use std::io::Write;
            match std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
            {
                Ok(mut child) => {
                    if let Some(stdin) = child.stdin.as_mut() {
                        if let Err(err) = stdin.write_all(text.as_bytes()) {
                            eprintln!("volt-ui: failed to write clipboard data: {err}");
                        }
                    }
                    if let Err(err) = child.wait() {
                        eprintln!("volt-ui: failed waiting on pbcopy: {err}");
                    }
                }
                Err(err) => eprintln!("volt-ui: failed to launch pbcopy: {err}"),
            }
        }
        #[cfg(not(target_os = "macos"))]
        let _ = text;
        self.selection = None;
        true
    }

    /// Write user input to the active pane's PTY, returning the view to live
    /// output and restarting the cursor blink cycle. No-ops (after still
    /// resetting the scroll view, a harmless local UI convenience) when the
    /// pane is marked read-only via the context menu.
    fn send_pty_input(&mut self, bytes: &[u8]) {
        if self.workspace_search.visible {
            return;
        }
        self.bump_cursor_blink();
        let pane = self.active_pane_mut();
        pane.scroll_view_offset = 0;
        if pane.read_only {
            return;
        }
        if let Err(err) = pane.pty.write(bytes) {
            eprintln!("volt-ui: failed to write PTY input: {err}");
        }
    }

    fn insert_committed_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(editor) = self.theme_editor.as_mut() {
            let outcome = editor.type_text(text);
            self.apply_editor_outcome(outcome);
            return;
        }
        match committed_text_target(
            self.workspace_search.visible,
            self.active_prompt.is_some(),
            self.workspace_panel.visible
                && self.workspace_panel.focused
                && self.renderer.workspace_card_layout().is_some(),
            self.active_tab().active_pane().read_only,
        ) {
            CommittedTextTarget::Search => {
                self.workspace_search.edit(text);
                self.sync_search();
            }
            CommittedTextTarget::Prompt => {
                if let Some(prompt) = self.active_prompt.as_mut() {
                    for c in text.chars() {
                        prompt.insert_char(c);
                    }
                }
                self.on_prompt_text_changed();
            }
            CommittedTextTarget::Terminal => self.send_pty_input(text.as_bytes()),
            CommittedTextTarget::Ignore => {}
        }
    }

    fn paste_clipboard(&mut self) {
        // Clipboard access below is macOS-only, like the rest of paste.
        #[cfg(target_os = "macos")]
        if self.theme_editor.is_some() {
            match std::process::Command::new("pbpaste").output() {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout).into_owned();
                    if let Some(editor) = self.theme_editor.as_mut() {
                        let outcome = editor.paste(&text);
                        self.apply_editor_outcome(outcome);
                    }
                }
                Ok(output) => eprintln!("volt-ui: pbpaste exited with {}", output.status),
                Err(err) => eprintln!("volt-ui: failed to launch pbpaste: {err}"),
            }
            return;
        }
        #[cfg(target_os = "macos")]
        if !self.workspace_search.visible
            && self.workspace_panel.visible
            && self.workspace_panel.focused
        {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("pbpaste").output() {
                Ok(output) => {
                    if output.status.success() {
                        if self.workspace_search.visible {
                            self.workspace_search
                                .edit(&String::from_utf8_lossy(&output.stdout));
                            self.sync_search();
                            return;
                        }
                        let bracketed = self
                            .tabs
                            .get(self.active_tab)
                            .and_then(|t| t.active_pane().performer.lock().ok())
                            .map(|p| p.bracketed_paste_mode())
                            .unwrap_or(false);
                        if bracketed {
                            // Never let pasted bytes terminate bracketed-paste mode early.
                            let data = strip_bracketed_paste_end(&output.stdout);
                            let mut wrapped = Vec::with_capacity(data.len() + 12);
                            wrapped.extend_from_slice(b"\x1b[200~");
                            wrapped.extend_from_slice(&data);
                            wrapped.extend_from_slice(b"\x1b[201~");
                            self.send_pty_input(&wrapped);
                        } else {
                            self.send_pty_input(&output.stdout);
                        }
                    } else {
                        eprintln!("volt-ui: pbpaste exited with {}", output.status);
                    }
                }
                Err(err) => eprintln!("volt-ui: failed to launch pbpaste: {err}"),
            }
        }
    }

    // ── Find / rename overlay ────────────────────────────────────────────

    fn open_find_prompt(&mut self) {
        if self.theme_editor.is_some() {
            return;
        }
        self.workspace_search.close();
        self.renderer.search_palette = None;
        self.search_dirty = false;
        self.search_target = Some((self.active_tab, self.active_tab().tree.active_id));
        self.active_prompt = Some(crate::prompt::TextPrompt::new(
            crate::prompt::PromptKind::Find,
            "",
        ));
        self.begin_redraw();
    }

    #[cfg(target_os = "macos")]
    fn open_rename_prompt(&mut self, kind: crate::prompt::PromptKind) {
        if self.theme_editor.is_some() {
            return;
        }
        self.workspace_search.close();
        self.renderer.search_palette = None;
        self.search_dirty = false;
        self.search_target = None;
        let custom = match kind {
            crate::prompt::PromptKind::RenameTab => self.active_tab().custom_title.as_deref(),
            crate::prompt::PromptKind::RenameTerminal => {
                self.active_tab().active_pane().custom_title.as_deref()
            }
            crate::prompt::PromptKind::Find | crate::prompt::PromptKind::OpenLink => None,
        };
        let initial = rename_initial_text(custom);
        self.active_prompt = Some(crate::prompt::TextPrompt::new(kind, &initial));
        self.begin_redraw();
    }

    fn confirm_active_prompt(&mut self) {
        let Some(prompt) = self.active_prompt.take() else {
            return;
        };
        self.search_dirty = false;
        self.search_target = None;
        match prompt.kind {
            crate::prompt::PromptKind::OpenLink => {
                if let Err(err) = crate::links::open(&prompt.text()) {
                    self.renderer
                        .set_top_alert(Some(format!("Could not open link: {err}")));
                }
            }
            crate::prompt::PromptKind::Find => {
                // Enter/Shift+Enter cycle matches instead of confirming while
                // a Find prompt is open; reaching here just closes it.
            }
            crate::prompt::PromptKind::RenameTab => {
                let text = prompt.text();
                self.active_tab_mut().custom_title = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
            crate::prompt::PromptKind::RenameTerminal => {
                let text = prompt.text();
                self.active_pane_mut().custom_title = if text.trim().is_empty() {
                    None
                } else {
                    Some(text)
                };
            }
        }
        self.begin_redraw();
    }

    fn on_prompt_text_changed(&mut self) {
        if let Some(prompt) = self.active_prompt.as_mut() {
            prompt.clear_matches();
        }
        self.recompute_search_matches();
        self.scroll_to_current_match();
        self.begin_redraw();
    }

    /// Snapshot grid characters under the performer lock, then perform the
    /// potentially expensive scan without blocking the PTY parse thread.
    fn recompute_search_matches(&mut self) {
        let is_find = matches!(
            self.active_prompt.as_ref().map(|p| p.kind),
            Some(crate::prompt::PromptKind::Find)
        );
        if !is_find {
            self.search_dirty = false;
            return;
        }
        let query = self
            .active_prompt
            .as_ref()
            .map(|p| p.text())
            .unwrap_or_default();
        if query.is_empty() {
            if let Some(prompt) = self.active_prompt.as_mut() {
                prompt.clear_matches();
            }
            self.search_dirty = false;
            self.last_search_refresh = Instant::now();
            return;
        }
        let rows = {
            let pane = self.active_tab().active_pane();
            let Ok(performer) = pane.performer.lock() else {
                self.search_dirty = false;
                return;
            };
            let grid = &performer.grid;
            let sb_len = grid.scrollback_len();
            let mut rows: Vec<(usize, String)> = Vec::with_capacity(sb_len + grid.rows);
            for i in 0..sb_len {
                let line = grid.row_text(grid.scrollback_row(i));
                rows.push((i, line));
            }
            for r in 0..grid.rows {
                let line = grid.row_text(grid.row_cells(r));
                rows.push((sb_len + r, line));
            }
            rows
        };
        const MAX_SEARCH_MATCHES: usize = 100_000;
        let (matches, truncated) = crate::prompt::find_matches_bounded(
            &query,
            rows.iter().map(|(i, s)| (*i, s.as_str())),
            MAX_SEARCH_MATCHES,
        );
        if let Some(prompt) = self.active_prompt.as_mut() {
            prompt.replace_matches(matches, truncated);
        }
        self.search_dirty = false;
        self.search_target = Some((self.active_tab, self.active_tab().tree.active_id));
        self.last_search_refresh = Instant::now();
    }

    fn invalidate_search_if_target_changed(&mut self) {
        if matches!(
            self.active_prompt.as_ref().map(|p| p.kind),
            Some(crate::prompt::PromptKind::Find)
        ) && self.search_target != Some((self.active_tab, self.active_tab().tree.active_id))
        {
            if let Some(prompt) = self.active_prompt.as_mut() {
                prompt.clear_matches();
            }
            self.invalidate_search_after_output();
        }
    }

    fn invalidate_search_after_output(&mut self) {
        if let Some(prompt) = self.active_prompt.as_mut() {
            if prompt.kind == crate::prompt::PromptKind::Find && !prompt.is_empty() {
                // Never display a stale highlight against a changed grid.
                prompt.invalidate_matches();
                self.search_dirty = true;
            }
        }
    }

    /// Scroll the active pane so the current search match is visible,
    /// placing its row at the top of the viewport when it's in scrollback.
    fn jump_prompt(&mut self, previous: bool) {
        let pane = self.active_pane_mut();
        let offset = match pane.performer.lock() {
            Ok(p) if !p.alternate_screen_active() => p
                .grid
                .prompt_scroll_offset(pane.scroll_view_offset, previous),
            _ => return,
        };
        pane.scroll_view_offset = offset;
        self.begin_redraw();
    }

    fn scroll_to_current_match(&mut self) {
        let Some(m) = self
            .active_prompt
            .as_ref()
            .and_then(|p| p.matches.get(p.current_match).copied())
        else {
            return;
        };
        let sb_len = {
            let pane = self.active_tab().active_pane();
            match pane.performer.lock() {
                Ok(p) => p.grid.scrollback_len(),
                Err(_) => return,
            }
        };
        let pane = self.active_pane_mut();
        pane.scroll_view_offset = if m.row < sb_len {
            sb_len.saturating_sub(m.row)
        } else {
            0
        };
        self.begin_redraw();
    }

    /// Full reset (RIS) of the active pane's terminal — clears the grid and
    /// scrollback, exits alt-screen, and resets cursor/SGR/mode state.
    #[cfg(target_os = "macos")]
    fn reset_active_terminal(&mut self) {
        {
            let pane = self.active_pane_mut();
            pane.scroll_view_offset = 0;
            match pane.performer.lock() {
                Ok(mut p) => p.reset(),
                Err(err) => eprintln!("volt-ui: failed to lock performer for reset: {err}"),
            }
        }
        self.selection = None;
        self.begin_redraw();
    }

    fn selection_tuple(&self) -> Option<((usize, usize), (usize, usize))> {
        let sel = self.selection?;
        let (total_cols, total_rows) = self.current_grid_size();
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        let rect = rects.iter().find(|r| r.id == sel.pane_id)?;
        let pane = self.active_tab().tree.find_leaf(sel.pane_id)?;
        let performer = pane.performer.lock().ok()?;
        if sel.alternate_screen != performer.alternate_screen_active() {
            return None;
        }
        let ((start_col, start_row), (end_col, end_row)) =
            sel.visible_range(&performer.grid, pane.scroll_view_offset, rect.rows)?;
        Some((
            (start_col + rect.col, start_row + rect.row),
            (end_col + rect.col, end_row + rect.row),
        ))
    }

    fn selection_is_block(&self) -> bool {
        self.selection
            .map(|sel| matches!(sel.mode, SelectionMode::Block))
            .unwrap_or(false)
    }

    /// Global window-grid coordinates of the current search match, if the
    /// Find prompt is open, there's at least one match, and it's currently
    /// within the active pane's visible viewport (mirrors the offset
    /// convention `blit_grid_with_scrollback` renders with — row 0 of the
    /// viewport is scrollback row `scrollback_len - scroll_view_offset`).
    fn current_match_tuple(&self) -> Option<((usize, usize), (usize, usize))> {
        let prompt = self.active_prompt.as_ref()?;
        let m = *prompt.matches.get(prompt.current_match)?;
        let (total_cols, total_rows) = self.current_grid_size();
        let tab = self.tabs.get(self.active_tab)?;
        let pane_id = tab.tree.active_id;
        let rects = tab.tree.layout(total_cols, total_rows);
        let rect = rects.iter().find(|r| r.id == pane_id)?;
        let pane = tab.tree.find_leaf(pane_id)?;
        let performer = pane.performer.lock().ok()?;
        let sb_len = performer.grid.scrollback_len();
        let offset = pane.scroll_view_offset.min(sb_len);
        let sb_rows_shown = offset.min(rect.rows);
        let sb_start = sb_len.saturating_sub(offset);

        let viewport_row = if m.row < sb_len {
            if m.row < sb_start {
                return None; // scrolled further back than the match
            }
            let r = m.row - sb_start;
            (r < sb_rows_shown).then_some(r)?
        } else {
            let live_row = m.row - sb_len;
            let r = sb_rows_shown + live_row;
            (r < rect.rows).then_some(r)?
        };

        Some((
            (m.start_col + rect.col, viewport_row + rect.row),
            (
                m.end_col.saturating_sub(1) + rect.col,
                viewport_row + rect.row,
            ),
        ))
    }

    fn active_mouse_reporting(&self) -> Option<(MouseTrackingMode, bool)> {
        let performer = self
            .tabs
            .get(self.active_tab)?
            .active_pane()
            .performer
            .lock()
            .ok()?;
        let mode = performer.mouse_tracking_mode();
        if mode == MouseTrackingMode::Off {
            None
        } else {
            Some((mode, performer.mouse_sgr_mode()))
        }
    }

    fn active_application_cursor_keys_mode(&self) -> bool {
        self.tabs
            .get(self.active_tab)
            .and_then(|tab| tab.active_pane().performer.lock().ok())
            .map(|performer| performer.application_cursor_keys_mode())
            .unwrap_or(false)
    }

    fn mouse_modifier_bits(&self) -> u8 {
        let mut bits = 0u8;
        if self.modifiers.shift_key() {
            bits |= 4;
        }
        if self.modifiers.alt_key() {
            bits |= 8;
        }
        if self.modifiers.control_key() {
            bits |= 16;
        }
        bits
    }

    fn mouse_button_code(button: MouseButton) -> Option<u8> {
        match button {
            MouseButton::Left => Some(0),
            MouseButton::Middle => Some(1),
            MouseButton::Right => Some(2),
            _ => None,
        }
    }

    fn write_mouse_report(&mut self, cb: u8, col: usize, row: usize, release: bool, sgr: bool) {
        if self.active_pane_mut().read_only {
            return;
        }
        let x = col + 1;
        let y = row + 1;
        if sgr {
            let suffix = if release { 'm' } else { 'M' };
            let seq = format!("\x1b[<{};{};{}{}", cb, x, y, suffix);
            if let Err(err) = self.active_pane_mut().pty.write(seq.as_bytes()) {
                eprintln!("volt-ui: failed to write SGR mouse report: {err}");
            }
            return;
        }

        if x > 223 || y > 223 {
            return;
        }
        let cb_encoded = cb.saturating_add(32);
        let x_encoded = (x as u8).saturating_add(32);
        let y_encoded = (y as u8).saturating_add(32);
        let packet = [0x1b, b'[', b'M', cb_encoded, x_encoded, y_encoded];
        if let Err(err) = self.active_pane_mut().pty.write(&packet) {
            eprintln!("volt-ui: failed to write mouse report: {err}");
        }
    }

    fn report_mouse_button(
        &mut self,
        button: MouseButton,
        button_state: ElementState,
        col: usize,
        row: usize,
    ) -> bool {
        let Some((mode, sgr)) = self.active_mouse_reporting() else {
            return false;
        };
        let Some(button_code) = Self::mouse_button_code(button) else {
            return false;
        };
        let mods = self.mouse_modifier_bits();
        match button_state {
            ElementState::Pressed => {
                self.pressed_mouse_button = Some(button);
                self.write_mouse_report(button_code.saturating_add(mods), col, row, false, sgr);
            }
            ElementState::Released => {
                if self.pressed_mouse_button == Some(button) {
                    self.pressed_mouse_button = None;
                }
                if mode != MouseTrackingMode::X10 {
                    self.write_mouse_report(3u8.saturating_add(mods), col, row, true, sgr);
                }
            }
        }
        true
    }

    fn report_mouse_motion(&mut self, col: usize, row: usize) -> bool {
        let Some((mode, sgr)) = self.active_mouse_reporting() else {
            return false;
        };
        let should_report = match mode {
            MouseTrackingMode::Off | MouseTrackingMode::X10 => false,
            MouseTrackingMode::ButtonEvent => self.pressed_mouse_button.is_some(),
            MouseTrackingMode::AnyMotion => true,
        };
        if !should_report {
            return false;
        }
        let base = self
            .pressed_mouse_button
            .and_then(Self::mouse_button_code)
            .unwrap_or(3);
        let cb = 32u8
            .saturating_add(base)
            .saturating_add(self.mouse_modifier_bits());
        self.write_mouse_report(cb, col, row, false, sgr);
        true
    }

    fn report_mouse_wheel(&mut self, delta: MouseScrollDelta, col: usize, row: usize) -> bool {
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / self.renderer.cell_height.max(1.0),
        };
        if amount == 0.0 {
            return false;
        }

        // If the active pane has mouse reporting enabled, forward the wheel
        // event to the PTY (e.g. vim, less). Shift bypasses so the user can
        // always reach the local scrollback view.
        if !self.shift_down() && !self.is_drag_selecting {
            if let Some((_mode, sgr)) = self.active_mouse_reporting() {
                let steps = amount.abs().ceil().max(1.0) as usize;
                let base = if amount > 0.0 { 64u8 } else { 65u8 };
                let cb = base.saturating_add(self.mouse_modifier_bits());
                for _ in 0..steps {
                    self.write_mouse_report(cb, col, row, false, sgr);
                }
                return true;
            }
        }

        // No mouse reporting — scroll the scrollback view instead.
        let steps = amount.abs().ceil().max(1.0) as usize * 3;
        let pane = self.active_pane_mut();
        let scrollback_len = pane
            .performer
            .lock()
            .ok()
            .map(|p| p.grid.scrollback_len())
            .unwrap_or(0);
        if amount > 0.0 {
            // Scroll up: reveal older lines.
            pane.scroll_view_offset = (pane.scroll_view_offset + steps).min(scrollback_len);
        } else {
            // Scroll down: return toward live output.
            pane.scroll_view_offset = pane.scroll_view_offset.saturating_sub(steps);
        }
        true
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    config: Config,
    config_alert: Option<String>,
    windows: HashMap<WindowId, MainState>,
    proxy: Option<EventLoopProxy<VoltEvent>>,
    pty_wake_pending: Arc<AtomicBool>,
    rt: tokio::runtime::Runtime,
    /// The window that last reported `Focused(true)`. Native menu actions and
    /// new-window CWD inheritance use this window on every platform.
    focused_window: Option<WindowId>,
    #[cfg(target_os = "macos")]
    /// Must stay alive for the app's full lifetime — see the doc comment on
    /// `menu::install_app_menu`. Dropping this frees the Rust-side data
    /// every native `NSMenuItem`'s action handler reads via a raw pointer,
    /// while AppKit's menu bar keeps running against those now-dangling
    /// pointers; the first click then reads freed memory and aborts.
    app_menu: Option<crate::menu::AppMenu>,
    /// Built-in and user themes, reloaded on settings reload and after saves.
    themes: ThemeRegistry,
}

impl App {
    pub fn new(config: Config, config_alert: Option<String>) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let themes = ThemeRegistry::load();
        let config_alert = with_theme_problems(config_alert, &themes);
        Ok(Self {
            config,
            config_alert,
            themes,
            windows: HashMap::new(),
            proxy: None,
            pty_wake_pending: Arc::new(AtomicBool::new(false)),
            rt,
            focused_window: None,
            #[cfg(target_os = "macos")]
            app_menu: None,
        })
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        let event_loop = EventLoop::<VoltEvent>::with_user_event().build()?;
        self.proxy = Some(event_loop.create_proxy());
        event_loop.run_app(&mut self)?;
        Ok(())
    }

    fn create_main_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        #[allow(unused_variables)] tabbing_identifier: Option<String>,
    ) -> anyhow::Result<WindowId> {
        let Some(proxy) = self.proxy.as_ref().cloned() else {
            anyhow::bail!("missing event loop proxy");
        };

        let mut window_attrs = Window::default_attributes()
            .with_title("Volt")
            .with_inner_size(winit::dpi::PhysicalSize::new(1400u32, 900u32));

        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            let use_native = self.config.appearance.native_tabs;
            event_loop.set_allows_automatic_window_tabbing(use_native);
            if use_native {
                // Native mode: keep the system title bar visible so traffic lights and
                // tab-strip chrome are rendered by macOS as a distinct title area.
                window_attrs = window_attrs
                    .with_titlebar_transparent(false)
                    .with_tabbing_identifier("volt.terminal");
            } else {
                // Custom tab bar mode: hide native chrome, extend content to fill window.
                window_attrs = window_attrs
                    .with_titlebar_transparent(true)
                    .with_fullsize_content_view(true)
                    .with_title_hidden(true);
            }
        }
        window_attrs = window_attrs
            .with_transparent(self.config.appearance.transparent_enabled())
            .with_blur(self.config.appearance.blur_enabled());

        let window = Arc::new(event_loop.create_window(window_attrs)?);
        // Capture native picker text without changing normal keyboard/IME handling.
        #[cfg(target_os = "macos")]
        let native_text =
            crate::native_text::NativeTextInput::install(window.clone(), proxy.clone())?;
        #[cfg(target_os = "macos")]
        let native_tab_count = window.num_tabs().max(1);
        #[cfg(target_os = "macos")]
        configure_macos_tab_chrome(window.as_ref(), native_tab_count);
        let scale_factor = window.scale_factor() as f32;
        let mut renderer = self.rt.block_on(Renderer::new(
            window.clone(),
            self.config.font.size,
            scale_factor,
            &self.config.font.family,
            self.config.appearance.padding as f32,
            self.config.appearance.line_height,
            self.config.appearance.effective_opacity(),
            self.config.appearance.cursor_style,
        ))?;
        renderer.set_top_alert(self.config_alert.clone());
        if cfg!(target_os = "macos") && self.config.appearance.native_tabs {
            renderer.custom_tab_bar = false;
        }
        #[cfg(target_os = "macos")]
        let (cols, rows) = renderer.grid_size_for_tab_count(native_tab_count);
        #[cfg(not(target_os = "macos"))]
        let (cols, rows) = renderer.grid_size_for_tab_count(1);
        let cwd = self
            .target_window_id()
            .and_then(|id| self.windows.get(&id))
            .and_then(|state| state.active_tab().active_pane().local_cwd());
        let first_tab = TerminalTab::spawn_in(
            &self.config,
            cols as u16,
            rows as u16,
            proxy.clone(),
            Arc::clone(&self.pty_wake_pending),
            cwd.as_deref(),
        )?;
        let theme = self.themes.resolve(&self.config.theme);
        let id = window.id();
        #[cfg(target_os = "macos")]
        let display_link = match DisplayLinkScheduler::new(proxy.clone()) {
            Ok(dl) => Some(dl),
            Err(err) => {
                eprintln!("volt-ui: failed to start CVDisplayLink scheduler: {err}");
                None
            }
        };

        #[cfg_attr(not(target_os = "macos"), allow(unused_mut))]
        let mut state = MainState {
            id,
            window,
            #[cfg(target_os = "macos")]
            _native_text: native_text,
            renderer,
            render_snapshot: None,
            tabs: vec![first_tab],
            active_tab: 0,
            theme,
            modifiers: ModifiersState::default(),
            left_shift_down: false,
            right_shift_down: false,
            left_control_down: false,
            right_control_down: false,
            left_super_down: false,
            right_super_down: false,
            keybindings: crate::keybindings::Bindings::compile(&self.config.keybindings),
            config: self.config.clone(),
            mouse_pos: (0.0, 0.0),
            is_drag_selecting: false,
            selection_scroll_deadline: None,
            selection: None,
            pressed_mouse_button: None,
            last_reported_mouse_cell: None,
            redraw_pending: false,
            post_resize_redraws: 0,
            pending_damage_rows: None,
            force_full_redraw: true,
            blink_state: true,
            next_cursor_blink: Instant::now() + CURSOR_BLINK_INTERVAL,
            #[cfg(target_os = "macos")]
            display_link,
            #[cfg(target_os = "macos")]
            last_known_native_tab_count: native_tab_count,
            proxy,
            pty_wake_pending: Arc::clone(&self.pty_wake_pending),
            divider_drag: None,
            divider_hover_id: None,
            workspace_panel: WorkspacePanel::default(),
            workspace_search: crate::workspace_search::SearchPalette::default(),
            search_mouse_capture: None,
            last_click: None,
            click_count: 0,
            #[cfg(target_os = "macos")]
            last_native_title: String::new(),
            last_tab_bar_click: None,
            active_prompt: None,
            search_dirty: false,
            search_target: None,
            last_search_refresh: Instant::now(),
            show_inspector: false,
            theme_editor: None,
        };
        state
            .window
            .set_transparent(self.config.appearance.transparent_enabled());
        state.window.set_blur(self.config.appearance.blur_enabled());
        #[cfg(target_os = "macos")]
        state.sync_native_window_title();
        let id = state.id;
        self.windows.insert(id, state);
        Ok(id)
    }

    /// Reload settings and theme files in every window. The registry and menu
    /// are app-wide, so leaving other windows on stale themes is inconsistent.
    fn reload_config(&mut self) {
        let (new_cfg, config_alert) = Config::load_with_diagnostics();
        self.themes = ThemeRegistry::load();
        let config_alert = with_theme_problems(config_alert, &self.themes);
        self.config = new_cfg.clone();
        self.config_alert = config_alert.clone();
        let theme = self.themes.resolve(&new_cfg.theme);
        for state in self.windows.values_mut() {
            state.renderer.set_top_alert(config_alert.clone());
            state.cancel_theme_editor();
            state.theme = theme.clone();
            state.apply_config(&new_cfg);
        }
        self.refresh_theme_menu();
    }

    /// Rebuild Volt ▸ Theme from the registry, checking the configured theme.
    fn refresh_theme_menu(&mut self) {
        #[cfg(target_os = "macos")]
        if let Some(menu) = self.app_menu.as_mut() {
            let themes: Vec<_> = self
                .themes
                .entries()
                .iter()
                .map(|e| (e.id.clone(), e.name.clone(), e.is_builtin()))
                .collect();
            menu.set_themes(&themes, &self.config.theme);
        }
    }

    /// Switch every window to theme `id` and remember it in config.toml.
    /// `saved_from` is the window whose editor just saved this theme; its
    /// editor stays open. Any other open editor is closed first.
    fn select_theme(&mut self, id: &str, saved_from: Option<WindowId>) {
        let Some(theme) = self.themes.get(id).map(|e| e.theme.clone()) else {
            return;
        };
        self.config.theme = id.to_string();
        let alert = match config_path_to_edit() {
            Some(path) => volt_config::set_theme_in_config_file(&path, id)
                .err()
                .map(|err| {
                    format!(
                        "Couldn't save the theme choice to {}: {err}",
                        path.display()
                    )
                }),
            None => Some("Couldn't find the config folder to save the theme choice".into()),
        };
        for (wid, state) in self.windows.iter_mut() {
            if Some(*wid) != saved_from {
                state.cancel_theme_editor();
                state.theme = theme.clone();
            }
            state.config.theme = id.to_string();
            if alert.is_some() {
                state.renderer.set_top_alert(alert.clone());
            }
            state.begin_redraw();
        }
        self.refresh_theme_menu();
    }

    /// Open the editor on `window_id`'s current theme, or close it if open.
    fn toggle_theme_editor(&mut self, window_id: WindowId) {
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        if state.theme_editor.is_some() {
            state.cancel_theme_editor();
            return;
        }
        // Unknown ids render as the Catppuccin fallback, so edit that.
        let entry = self
            .themes
            .get(&state.config.theme)
            .or_else(|| self.themes.get("catppuccin"));
        let (name, target) = match entry {
            Some(e) => (e.name.clone(), (!e.is_builtin()).then(|| e.id.clone())),
            None => ("Theme".to_string(), None),
        };
        state.workspace_search.close();
        state.renderer.search_palette = None;
        state.active_prompt = None;
        state.search_dirty = false;
        state.search_target = None;
        state.workspace_panel.focused = false;
        state.theme_editor = Some(crate::theme_editor::ThemeEditor::new(
            &name,
            target,
            state.theme.clone(),
        ));
        state.sync_theme_editor();
        if state.theme_editor.is_none() {
            eprintln!("volt-ui: the window is too small for the theme editor");
        }
    }

    /// Write the editor's working theme and switch to it.
    fn save_edited_theme(&mut self, window_id: WindowId, name: String, overwrite: Option<String>) {
        let Some(working) = self
            .windows
            .get(&window_id)
            .and_then(|s| s.theme_editor.as_ref())
            .map(|e| e.working().clone())
        else {
            return;
        };
        let result = match volt_config::themes::themes_dir() {
            Some(dir) => {
                volt_config::themes::save_theme(&dir, &name, &working, overwrite.as_deref())
            }
            None => Err("Couldn't find the config folder".into()),
        };
        match result {
            Ok(id) => {
                self.themes = ThemeRegistry::load();
                if !self
                    .themes
                    .get(&id)
                    .is_some_and(|entry| entry.theme == working)
                {
                    if let Some(editor) = self
                        .windows
                        .get_mut(&window_id)
                        .and_then(|s| s.theme_editor.as_mut())
                    {
                        editor.save_failed(format!(
                            "Saved themes/{id}.toml, but it did not load; check the theme files"
                        ));
                    }
                    if let Some(state) = self.windows.get_mut(&window_id) {
                        state.sync_theme_editor();
                    }
                    return;
                }
                if let Some(editor) = self
                    .windows
                    .get_mut(&window_id)
                    .and_then(|s| s.theme_editor.as_mut())
                {
                    editor.saved(id.clone(), name);
                }
                self.select_theme(&id, Some(window_id));
            }
            Err(err) => {
                if let Some(editor) = self
                    .windows
                    .get_mut(&window_id)
                    .and_then(|s| s.theme_editor.as_mut())
                {
                    editor.save_failed(err);
                }
            }
        }
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.sync_theme_editor();
        }
    }

    /// Resolve which window a menu-bar/context-menu action should target:
    /// the last-focused window, falling back to an arbitrary one if that's
    /// stale (its window closed) or unset (no `Focused(true)` has fired
    /// yet, e.g. right at startup).
    fn target_window_id(&self) -> Option<WindowId> {
        self.focused_window
            .filter(|id| self.windows.contains_key(id))
            .or_else(|| self.windows.keys().next().copied())
    }

    #[cfg(target_os = "macos")]
    /// Route a native menu-bar or context-menu click to the same
    /// `MainState`/`App` methods the matching keyboard shortcut calls, so
    /// the two paths can never drift apart.
    fn handle_menu_action(
        &mut self,
        event_loop: &ActiveEventLoop,
        action: crate::menu::MenuAction,
    ) {
        use crate::menu::MenuAction as A;

        match action {
            A::NewWindow => {
                let _ = self.create_main_window(event_loop, None);
                return;
            }
            A::OpenSettings => {
                open_config_in_editor();
                return;
            }
            A::OpenThemesFolder => {
                open_themes_folder();
                return;
            }
            _ => {}
        }

        let Some(window_id) = self.target_window_id() else {
            return;
        };

        match action {
            A::ReloadSettings => {
                self.reload_config();
                return;
            }
            A::CustomizeTheme => {
                self.toggle_theme_editor(window_id);
                return;
            }
            A::CloseWindow => {
                self.windows.remove(&window_id);
                if self.windows.is_empty() {
                    event_loop.exit();
                }
                return;
            }
            _ => {}
        }

        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };

        match action {
            A::NewTab => {
                if state.config.appearance.native_tabs {
                    if let Some(proxy) = self.proxy.as_ref() {
                        let _ = proxy.send_event(VoltEvent::CreateNewWindow);
                    }
                } else {
                    state.new_tab();
                    state.begin_redraw();
                }
            }
            A::CloseTab => {
                if state.close_active_pane_or_tab() {
                    let _ = state;
                    self.windows.remove(&window_id);
                    if self.windows.is_empty() {
                        event_loop.exit();
                    }
                }
            }
            A::Copy => {
                if state.copy_selection() {
                    state.begin_redraw();
                }
            }
            A::Paste => {
                state.paste_clipboard();
                state.begin_redraw();
            }
            A::SearchWorkspace => state.toggle_search(),
            A::Find => state.open_find_prompt(),
            A::ToggleWorkspaceLayout => state.toggle_workspace_layout(),
            A::ToggleChatPanel => {
                state.toggle_workspace_panel();
            }
            A::IncreaseFontSize => {
                if state.adjust_font_size(1.0) {
                    self.config.font.size = state.config.font.size;
                }
            }
            A::DecreaseFontSize => {
                if state.adjust_font_size(-1.0) {
                    self.config.font.size = state.config.font.size;
                }
            }
            A::ToggleFullScreen => {
                let is_fullscreen = state.window.fullscreen().is_some();
                state.window.set_fullscreen(if is_fullscreen {
                    None
                } else {
                    Some(winit::window::Fullscreen::Borderless(None))
                });
            }
            A::Zoom => {
                let maximized = state.window.is_maximized();
                state.window.set_maximized(!maximized);
            }
            A::SplitRight => {
                if state.split_active_tab_positioned(PaneSplitDirection::Vertical, false) {
                    state.begin_redraw();
                }
            }
            A::SplitLeft => {
                if state.split_active_tab_positioned(PaneSplitDirection::Vertical, true) {
                    state.begin_redraw();
                }
            }
            A::SplitDown => {
                if state.split_active_tab_positioned(PaneSplitDirection::Horizontal, false) {
                    state.begin_redraw();
                }
            }
            A::SplitUp => {
                if state.split_active_tab_positioned(PaneSplitDirection::Horizontal, true) {
                    state.begin_redraw();
                }
            }
            A::ResetTerminal => state.reset_active_terminal(),
            A::ToggleInspector => {
                state.show_inspector = !state.show_inspector;
                state.begin_redraw();
            }
            A::ToggleReadOnly => {
                let pane = state.active_pane_mut();
                pane.read_only = !pane.read_only;
                state.begin_redraw();
            }
            A::ChangeTabTitle => state.open_rename_prompt(crate::prompt::PromptKind::RenameTab),
            A::ChangeTerminalTitle => {
                state.open_rename_prompt(crate::prompt::PromptKind::RenameTerminal)
            }
            A::SearchGoogle => {
                if let Some(text) = state.selected_text() {
                    let url = format!(
                        "https://www.google.com/search?q={}",
                        percent_encode_query(&text)
                    );
                    if let Err(err) = std::process::Command::new("open").arg(&url).spawn() {
                        eprintln!("volt-ui: failed to open search URL: {err}");
                    }
                }
            }
            A::NewWindow
            | A::OpenSettings
            | A::ReloadSettings
            | A::CloseWindow
            | A::CustomizeTheme
            | A::OpenThemesFolder => {
                unreachable!("handled in the early-return blocks above")
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn configure_macos_tab_chrome(_window: &Window, _native_tab_count: usize) {
    // Keep macOS tab handling fully native for stability.
    // We only use winit's tabbing identifier / native APIs.
}

/// Note skipped theme files under the config alert, without hiding it.
fn with_theme_problems(alert: Option<String>, themes: &ThemeRegistry) -> Option<String> {
    let problems = themes.problems();
    let Some(first) = problems.first() else {
        return alert;
    };
    let note = if problems.len() == 1 {
        format!("Skipped theme file {first}")
    } else {
        format!("Skipped {} theme files; first: {first}", problems.len())
    };
    for problem in problems {
        eprintln!("volt-ui: skipped theme file {problem}");
    }
    Some(match alert {
        Some(alert) => format!("{alert} · {note}"),
        None => note,
    })
}

#[cfg(target_os = "macos")]
fn open_themes_folder() {
    let Some(dir) = volt_config::themes::themes_dir() else {
        return;
    };
    if let Err(err) = std::fs::create_dir_all(&dir) {
        eprintln!("volt-ui: failed to create {}: {err}", dir.display());
        return;
    }
    if let Err(err) = std::process::Command::new("open").arg(&dir).spawn() {
        eprintln!("volt-ui: failed to open themes folder: {err}");
    }
}

fn open_config_in_editor() {
    let Some(path) = config_path_to_edit() else {
        return;
    };
    if let Err(err) = create_sample_config_if_missing(&path) {
        eprintln!("volt-ui: failed to create sample config: {err}");
        return;
    }

    #[cfg(target_os = "macos")]
    {
        if let Err(err) = std::process::Command::new("open").arg(&path).spawn() {
            eprintln!("volt-ui: failed to open config in editor: {err}");
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        if let Err(err) = std::process::Command::new("xdg-open").arg(&path).spawn() {
            eprintln!("volt-ui: failed to open config in editor: {err}");
        }
    }
}

/// Never replace an existing config, including old files without our comment
/// header. `create_new` also closes the existence-check/write race.
fn create_sample_config_if_missing(path: &std::path::Path) -> std::io::Result<()> {
    use std::io::Write;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => file.write_all(sample_config_toml().as_bytes()),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(target_os = "macos")]
fn should_show_context_menu(shift_down: bool, mouse_reporting: bool, read_only: bool) -> bool {
    shift_down || read_only || !mouse_reporting
}

#[derive(Debug, PartialEq, Eq)]
enum CommittedTextTarget {
    Search,
    Prompt,
    Terminal,
    Ignore,
}

/// Native picker/IME text follows the same modal ownership as keyboard input.
/// Read-only terminal state must not stop editing a local Find/search field.
fn committed_text_target(
    search: bool,
    prompt: bool,
    inspector_focused: bool,
    read_only: bool,
) -> CommittedTextTarget {
    if search {
        CommittedTextTarget::Search
    } else if prompt {
        CommittedTextTarget::Prompt
    } else if inspector_focused || read_only {
        CommittedTextTarget::Ignore
    } else {
        CommittedTextTarget::Terminal
    }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

impl ApplicationHandler<VoltEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }

        #[cfg(target_os = "macos")]
        set_app_icon();

        #[cfg(target_os = "macos")]
        {
            // Stored on `self` for the app's full lifetime — see the doc
            // comment on `install_app_menu`. Letting this drop after the
            // call (as an earlier version of this code did) freed the
            // Rust-side data every native menu item's click handler reads,
            // while AppKit's menu bar kept running against the now-dangling
            // pointers — confirmed via crash report as the cause of a
            // reproducible SIGABRT on the first menu click or accelerator.
            self.app_menu = Some(crate::menu::install_app_menu());
            self.refresh_theme_menu();
            // NSApp.mainMenu is process-global — install once here, not per
            // window. Menu clicks arrive on muda's own dispatch, off the
            // winit event loop, so forward them through the same
            // EventLoopProxy<VoltEvent> pattern display_link.rs already
            // uses to wake the loop from a background source.
            if let Some(proxy) = self.proxy.clone() {
                muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                    let _ = proxy.send_event(VoltEvent::Menu(event.id().0.clone()));
                }));
            }
        }

        if let Err(err) = self.create_main_window(event_loop, Some("volt".to_string())) {
            eprintln!("volt-ui: failed to create initial window: {err}");
            event_loop.exit();
        }
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: VoltEvent) {
        match event {
            VoltEvent::WorkspaceUpdated => {}
            #[cfg(target_os = "macos")]
            VoltEvent::NativeText { window_id, text } => {
                if let Some(state) = self.windows.get_mut(&window_id) {
                    state.insert_committed_text(&text);
                }
            }
            VoltEvent::PtyData => {
                self.pty_wake_pending.store(false, Ordering::Release);
                for state in self.windows.values_mut() {
                    state.queue_redraw();
                }
            }
            #[cfg(target_os = "macos")]
            VoltEvent::DisplayLinkTick => {
                for state in self.windows.values_mut() {
                    if state.redraw_pending {
                        state.window.request_redraw();
                    }
                }
            }
            #[cfg(target_os = "macos")]
            VoltEvent::CreateNewWindow => {
                let _ = self.create_main_window(_event_loop, None);
            }
            #[cfg(target_os = "macos")]
            VoltEvent::Menu(id) => {
                if let Some(theme) = crate::menu::theme_from_id(&id) {
                    self.select_theme(theme, None);
                } else if let Some(action) = crate::menu::MenuAction::from_id(&id) {
                    self.handle_menu_action(_event_loop, action);
                }
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        if matches!(event, WindowEvent::CloseRequested) {
            self.windows.remove(&window_id);
            if self.windows.is_empty() {
                event_loop.exit();
            }
            return;
        }
        let Some(state) = self.windows.get_mut(&window_id) else {
            return;
        };
        let mut close_window_after_event = false;

        match event {
            WindowEvent::Ime(Ime::Commit(text)) => {
                // Commit is final UTF-8 input, not paste or a command key.
                // Winit suppresses the composing key event to avoid duplicates.
                state.insert_committed_text(&text);
            }
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                if state.workspace_search.visible {
                    state.sync_search();
                }
                if state.theme_editor.is_some() {
                    state.sync_theme_editor();
                }
                state.resize_all_tabs_to_current_grid();
                #[cfg(target_os = "macos")]
                {
                    let native_tab_count = state.window.num_tabs().max(1);
                    state.last_known_native_tab_count = native_tab_count;
                    configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                }
                state.begin_redraw();
                state.post_resize_redraws = 1;
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state
                    .renderer
                    .update_scale(scale_factor as f32, state.config.font.size);
                state.resize_all_tabs_to_current_grid();
                if state.theme_editor.is_some() {
                    state.sync_theme_editor();
                }
                #[cfg(target_os = "macos")]
                {
                    let native_tab_count = state.window.num_tabs().max(1);
                    state.last_known_native_tab_count = native_tab_count;
                    configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                }
                state.begin_redraw();
                state.post_resize_redraws = 1;
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::Focused(false) => {
                state.workspace_panel.drag_offset = None;
                state.is_drag_selecting = false;
                state.selection_scroll_deadline = None;
                state.left_shift_down = false;
                state.right_shift_down = false;
                state.left_control_down = false;
                state.right_control_down = false;
                state.left_super_down = false;
                state.right_super_down = false;
            }
            WindowEvent::Focused(true) => {
                #[cfg(target_os = "macos")]
                {
                    let native_tab_count = state.window.num_tabs().max(1);
                    state.last_known_native_tab_count = native_tab_count;
                    configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                }
                self.focused_window = Some(window_id);
            }

            WindowEvent::CursorMoved { position, .. } => {
                let pointer_moved = state.mouse_pos != (position.x as f32, position.y as f32);
                state.mouse_pos = (position.x as f32, position.y as f32);
                if state.workspace_search.visible || state.search_mouse_capture.is_some() {
                    return;
                }
                if let Some([dx, dy]) = state.workspace_panel.drag_offset {
                    let scale = state.renderer.scale_factor;
                    state.workspace_panel.position = Some([
                        position.x as f32 / scale - dx,
                        (position.y as f32 - state.renderer.workspace_card_top_offset()) / scale
                            - dy,
                    ]);
                    state.sync_workspace_card();
                    state.begin_redraw();
                    return;
                }

                if state.divider_drag.is_none() && !state.is_drag_selecting {
                    let hit = state.workspace_hit();
                    if pointer_moved && hit != state.workspace_panel.hover {
                        state.workspace_panel.hover = hit;
                        state.sync_workspace_card();
                        state.begin_redraw();
                    }
                    if state.workspace_contains_pointer() {
                        state.window.set_cursor(if hit.is_some() {
                            winit::window::CursorIcon::Pointer
                        } else {
                            winit::window::CursorIcon::Default
                        });
                        return;
                    }
                }
                if let Some(drag) = state.divider_drag {
                    // Absolute ratio: start_ratio + total_delta / local_span.
                    // delta_px is the total movement since drag began — no per-frame reset needed.
                    let current_px = if drag.is_vertical {
                        position.x as f32
                    } else {
                        position.y as f32
                    };
                    let delta_px = current_px - drag.start_px;
                    let (total_cols, total_rows) = state.current_grid_size();
                    let active = state.active_tab;
                    state.tabs[active].tree.set_ratio_from_drag(
                        drag.divider_id,
                        drag.start_ratio,
                        delta_px,
                        state.renderer.cell_width,
                        state.renderer.cell_height,
                        total_cols,
                        total_rows,
                    );
                    state.begin_redraw();
                } else {
                    // Update cursor icon and hover state near any divider.
                    let dividers = state.active_tab_dividers();
                    let mut hovered_id: Option<usize> = None;
                    for div in &dividers {
                        let is_vertical = matches!(div.direction, PaneSplitDirection::Vertical);
                        // For vertical dividers: check x proximity AND y in [div.y, div.y+div.height]
                        // For horizontal dividers: check y proximity AND x in [div.x, div.x+div.width]
                        let (axis_coord, axis_divider, span_coord, span_start, span_end) =
                            if is_vertical {
                                (
                                    position.x as f32,
                                    div.phys.x,
                                    position.y as f32,
                                    div.phys.y,
                                    div.phys.y + div.phys.height,
                                )
                            } else {
                                (
                                    position.y as f32,
                                    div.phys.y,
                                    position.x as f32,
                                    div.phys.x,
                                    div.phys.x + div.phys.width,
                                )
                            };
                        if (axis_coord - axis_divider).abs() < 6.0
                            && span_coord >= span_start - 4.0
                            && span_coord <= span_end + 4.0
                        {
                            let icon = if is_vertical {
                                winit::window::CursorIcon::ColResize
                            } else {
                                winit::window::CursorIcon::RowResize
                            };
                            state.window.set_cursor(icon);
                            hovered_id = Some(div.id);
                            break;
                        }
                    }
                    if hovered_id.is_none() {
                        state.window.set_cursor(winit::window::CursorIcon::Default);
                    }
                    // Trigger redraw when hover state changes so divider color updates.
                    if state.divider_hover_id != hovered_id {
                        state.divider_hover_id = hovered_id;
                        state.begin_redraw();
                    }

                    let cell = state.pane_cell_from_mouse(state.mouse_pos.0, state.mouse_pos.1);
                    if let Some((pane_id, col, row)) = cell {
                        if pane_id == state.active_tab().tree.active_id {
                            if !state.shift_down()
                                && !state.is_drag_selecting
                                && state.last_reported_mouse_cell != Some((col, row))
                            {
                                let _ = state.report_mouse_motion(col, row);
                                state.last_reported_mouse_cell = Some((col, row));
                            }
                        } else {
                            state.last_reported_mouse_cell = None;
                        }
                    } else {
                        state.last_reported_mouse_cell = None;
                    }
                    if state.is_drag_selecting {
                        state.update_selection_end(state.mouse_pos.0, state.mouse_pos.1);
                    }
                }
            }

            WindowEvent::MouseInput {
                state: btn_state,
                button,
                ..
            } => {
                if btn_state == ElementState::Released && state.search_mouse_capture == Some(button)
                {
                    state.search_mouse_capture = None;
                    return;
                }
                // Finish the drag before tab-bar / card hit testing. A release
                // above the terminal must not leave edge scrolling armed.
                if button == MouseButton::Left
                    && btn_state == ElementState::Released
                    && state.is_drag_selecting
                {
                    state.is_drag_selecting = false;
                    state.selection_scroll_deadline = None;
                    return;
                }
                // Clicks on the theme editor never reach the terminal below;
                // clicks elsewhere do, so text can still be selected.
                if btn_state == ElementState::Pressed {
                    if let Some(l) = state.renderer.theme_editor_layout() {
                        use volt_renderer::theme_editor::EditorHit;
                        let naming = state.theme_editor.as_ref().is_some_and(|e| e.is_naming());
                        let hit = l.hit(state.mouse_pos.0, state.mouse_pos.1, naming);
                        if hit != EditorHit::Outside {
                            state.search_mouse_capture = Some(button);
                            let outcome = match (button, state.theme_editor.as_mut()) {
                                (MouseButton::Left, Some(editor)) => match hit {
                                    EditorHit::Field(i) => Some(editor.focus(i)),
                                    EditorHit::Close => {
                                        Some(crate::theme_editor::EditorOutcome::Cancel)
                                    }
                                    EditorHit::Revert => Some(editor.revert()),
                                    EditorHit::Save => Some(editor.save(false)),
                                    EditorHit::Body | EditorHit::Outside => None,
                                },
                                _ => None,
                            };
                            if let Some((name, overwrite)) =
                                outcome.and_then(|o| state.apply_editor_outcome(o))
                            {
                                self.save_edited_theme(window_id, name, overwrite);
                            }
                            return;
                        }
                    }
                }
                if state.workspace_search.visible {
                    if btn_state == ElementState::Pressed {
                        state.search_mouse_capture = Some(button);
                    }
                    if btn_state == ElementState::Pressed && button == MouseButton::Left {
                        use volt_renderer::search_palette::SearchHit;
                        if let Some(l) = state.renderer.search_layout() {
                            match l.hit(
                                state.mouse_pos.0,
                                state.mouse_pos.1,
                                state.workspace_search.selected,
                                state.workspace_search.output.rows.len(),
                            ) {
                                SearchHit::Close | SearchHit::Outside => {
                                    state.workspace_search.close()
                                }
                                SearchHit::Scope(i) => state
                                    .workspace_search
                                    .set_scope(crate::workspace_search::Scope::ALL[i]),
                                SearchHit::Row(i) => {
                                    state.workspace_search.selected = i;
                                    state.workspace_search.preview_selected(&state.proxy);
                                }
                                SearchHit::Body => {}
                            }
                        }
                        state.sync_search();
                    }
                    return;
                }
                if button == MouseButton::Left
                    && btn_state == ElementState::Released
                    && state.workspace_panel.drag_offset.take().is_some()
                {
                    return;
                }
                if button == MouseButton::Left
                    && btn_state == ElementState::Pressed
                    && !state.is_drag_selecting
                    && state.divider_drag.is_none()
                    && state.config.workspace.layout
                        == volt_config::config::WorkspaceLayout::Floating
                {
                    if let Some(l) = state.renderer.workspace_card_layout() {
                        let (x, y) = state.mouse_pos;
                        if l.header_drag_contains(x, y, state.workspace_panel.minimized) {
                            state.workspace_panel.drag_offset =
                                Some([(x - l.x) / l.scale, (y - l.y) / l.scale]);
                            return;
                        }
                    }
                }
                if state.workspace_contains_pointer()
                    && !state.is_drag_selecting
                    && state.divider_drag.is_none()
                {
                    if button == MouseButton::Left && btn_state == ElementState::Pressed {
                        state.workspace_panel.focused = true;
                        if let Some(id) = state.workspace_hit() {
                            state.activate_workspace_action(id);
                        }
                        state.sync_workspace_card();
                        state.begin_redraw();
                    }
                    return;
                }
                if btn_state == ElementState::Pressed {
                    state.workspace_panel.focused = false;
                    state.bump_cursor_blink();
                }
                let (mx, my) = state.mouse_pos;
                let in_tab_bar = state.show_custom_tab_bar() && my < state.current_tab_bar_height();
                if in_tab_bar {
                    if button == MouseButton::Left && btn_state == ElementState::Pressed {
                        let hit_target = state.handle_click(mx, my);
                        if !hit_target {
                            // Empty chrome — double-click here maximizes, like
                            // Finder/Safari's tab-bar convention.
                            let now = Instant::now();
                            let is_double_click = state
                                .last_tab_bar_click
                                .is_some_and(|t| now.duration_since(t) < MULTI_CLICK_INTERVAL);
                            if is_double_click {
                                let maximized = state.window.is_maximized();
                                state.window.set_maximized(!maximized);
                                state.last_tab_bar_click = None;
                            } else {
                                state.last_tab_bar_click = Some(now);
                            }
                        } else {
                            state.last_tab_bar_click = None;
                        }
                        state.begin_redraw();
                    }
                    return;
                }

                #[cfg(target_os = "macos")]
                if button == MouseButton::Right && btn_state == ElementState::Pressed {
                    if let Some((pane_id, _col, _row)) = state.pane_cell_from_mouse(mx, my) {
                        state.active_tab_mut().tree.active_id = pane_id;
                        if state.selection.is_some_and(|s| s.pane_id != pane_id) {
                            state.selection = None;
                        }
                        // Let TUIs receive ordinary right-clicks when they opted
                        // into mouse reporting. Shift-right-click always opens
                        // Volt's own menu, as Shift already bypasses reporting.
                        if should_show_context_menu(
                            state.shift_down(),
                            state.active_mouse_reporting().is_some(),
                            state.active_tab().active_pane().read_only,
                        ) {
                            // Deliberately NOT clearing `state.selection` here —
                            // Copy and Search With Google below need something to
                            // act on, so a right-click must preserve whatever was
                            // already selected instead of discarding it first.
                            let read_only = state.active_tab().active_pane().read_only;
                            let has_selection = state.selected_text().is_some();
                            let context_menu =
                                crate::menu::build_context_menu(read_only, has_selection);
                            use muda::ContextMenu;
                            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                            if let Ok(handle) = state.window.window_handle() {
                                if let RawWindowHandle::AppKit(h) = handle.as_raw() {
                                    // Passing `None` here (rather than a position we
                                    // compute ourselves) asks muda to use AppKit's
                                    // own `NSEvent.mouseLocation` directly — this
                                    // sidesteps our own physical/logical + NSView
                                    // coordinate-flip math, which was landing the
                                    // menu away from the actual click point.
                                    unsafe {
                                        context_menu.show_context_menu_for_nsview(
                                            h.ns_view.as_ptr() as *const std::ffi::c_void,
                                            None,
                                        );
                                    }
                                }
                            }
                            state.begin_redraw();
                            return;
                        }
                    }
                }

                // Divider drag: release
                if btn_state == ElementState::Released && state.divider_drag.take().is_some() {
                    state.resize_all_tabs_to_current_grid();
                    state.window.set_cursor(winit::window::CursorIcon::Default);
                    state.begin_redraw();
                    return;
                }

                // Divider drag: press
                if button == MouseButton::Left && btn_state == ElementState::Pressed {
                    let dividers = state.active_tab_dividers();
                    let mut started_drag = false;
                    for div in &dividers {
                        let is_vertical = matches!(div.direction, PaneSplitDirection::Vertical);
                        let (axis_coord, axis_div, span_coord, span_start, span_end) =
                            if is_vertical {
                                (mx, div.phys.x, my, div.phys.y, div.phys.y + div.phys.height)
                            } else {
                                (my, div.phys.y, mx, div.phys.x, div.phys.x + div.phys.width)
                            };
                        if (axis_coord - axis_div).abs() < 6.0
                            && span_coord >= span_start - 4.0
                            && span_coord <= span_end + 4.0
                        {
                            let start_ratio =
                                state.active_tab().tree.get_ratio(div.id).unwrap_or(0.5);
                            state.divider_drag = Some(DividerDrag {
                                divider_id: div.id,
                                start_px: axis_coord,
                                start_ratio,
                                is_vertical,
                            });
                            started_drag = true;
                            break;
                        }
                    }
                    if started_drag {
                        return;
                    }
                }

                // Explicit modifier-click is owned by Volt, even in a mouse-aware
                // TUI. Never forward an orphan release or start a selection drag.
                let link_modifier = if cfg!(target_os = "macos") {
                    state.super_down()
                } else {
                    state.ctrl_down()
                };
                if button == MouseButton::Left
                    && btn_state == ElementState::Pressed
                    && link_modifier
                {
                    state.search_mouse_capture = Some(button);
                    if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                        let pane = state.active_tab().tree.find_leaf(pane_id);
                        let url = pane.and_then(|pane| {
                            let p = pane.performer.try_lock().ok()?;
                            crate::links::target_at(&p.grid, pane.scroll_view_offset, col, row)
                        });
                        if let Some(target) = url {
                            if target.explicit {
                                state.search_dirty = false;
                                state.search_target = None;
                                state.active_prompt = Some(crate::prompt::TextPrompt::new(
                                    crate::prompt::PromptKind::OpenLink,
                                    &target.uri,
                                ));
                                state.begin_redraw();
                            } else if let Err(err) = crate::links::open(&target.uri) {
                                state
                                    .renderer
                                    .set_top_alert(Some(format!("Could not open link: {err}")));
                                state.begin_redraw();
                            }
                        }
                    }
                    return;
                }

                if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                    state.active_tab_mut().tree.active_id = pane_id;
                    // Shift bypasses app mouse reporting so selection still works
                    // inside TUIs (standard xterm behaviour).
                    if !state.shift_down()
                        && !state.is_drag_selecting
                        && state.report_mouse_button(button, btn_state, col, row)
                    {
                        return;
                    }
                }

                if button == MouseButton::Left {
                    if btn_state == ElementState::Pressed {
                        if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                            state.active_tab_mut().tree.active_id = pane_id;
                            // Shift-click extends the existing anchor. It is
                            // not a double-click even if it lands quickly on
                            // the same cell, and it may cross viewports.
                            let shift_click = state.shift_down();
                            if shift_click {
                                let extended = {
                                    let pane = state.tabs[state.active_tab].tree.find_leaf(pane_id);
                                    match (pane, state.selection.as_mut()) {
                                        (Some(pane), Some(sel)) => {
                                            pane.performer.lock().ok().is_some_and(|p| {
                                                sel.extend_to(
                                                    pane_id,
                                                    p.alternate_screen_active(),
                                                    &p.grid,
                                                    pane.scroll_view_offset,
                                                    col,
                                                    row,
                                                )
                                            })
                                        }
                                        _ => false,
                                    }
                                };
                                if extended {
                                    state.click_count = 1;
                                    state.last_click = None;
                                    state.is_drag_selecting = true;
                                    state.selection_scroll_deadline = None;
                                    state.begin_redraw();
                                    return;
                                }
                            }
                            let now = Instant::now();
                            let same_spot = !shift_click
                                && state.last_click.is_some_and(|(t, p, c, r)| {
                                    now.duration_since(t) < MULTI_CLICK_INTERVAL
                                        && p == pane_id
                                        && c == col
                                        && r == row
                                });
                            state.click_count = if same_spot {
                                (state.click_count % 3) + 1
                            } else {
                                1
                            };
                            state.last_click = Some((now, pane_id, col, row));
                            let expanded = match state.click_count {
                                2 => state.word_selection(pane_id, col, row),
                                3 => state.line_selection(pane_id, row),
                                _ => None,
                            };
                            state.selection = expanded.or_else(|| {
                                let pane = state.tabs[state.active_tab].tree.find_leaf(pane_id)?;
                                let p = pane.performer.lock().ok()?;
                                Selection::at_viewport(
                                    pane_id,
                                    p.alternate_screen_active(),
                                    if state.modifiers.alt_key() {
                                        SelectionMode::Block
                                    } else {
                                        SelectionMode::Linear
                                    },
                                    &p.grid,
                                    pane.scroll_view_offset,
                                    col,
                                    row,
                                )
                            });
                            // Word/line selections stay fixed; only single clicks
                            // start a drag so trackpad jitter can't collapse them.
                            state.is_drag_selecting = state.click_count == 1;
                            state.selection_scroll_deadline = None;
                            state.begin_redraw();
                        }
                    } else {
                        state.is_drag_selecting = false;
                        state.selection_scroll_deadline = None;
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
                if state.renderer.theme_editor_layout().is_some_and(|l| {
                    l.hit(state.mouse_pos.0, state.mouse_pos.1, false)
                        != volt_renderer::theme_editor::EditorHit::Outside
                }) {
                    return;
                }
                if state.workspace_search.visible {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    if dy != 0. {
                        state.workspace_search.select(if dy < 0. { 1 } else { -1 });
                        state.sync_search();
                    }
                    return;
                }
                if state.workspace_contains_pointer() {
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y,
                        MouseScrollDelta::PixelDelta(p) => p.y as f32,
                    };
                    if let Some(l) = state.renderer.workspace_card_layout() {
                        if state.workspace_panel.minimized {
                            return;
                        }
                        let count = state.workspace_panel.card().rows.len();
                        let max = count.saturating_sub(l.count);
                        state.workspace_panel.scroll = if dy < 0.0 {
                            (state.workspace_panel.scroll + 1).min(max)
                        } else {
                            state.workspace_panel.scroll.saturating_sub(1)
                        };
                        state.sync_workspace_card();
                        state.begin_redraw();
                    }
                    return;
                }
                let my = state.mouse_pos.1;
                if state.show_custom_tab_bar() && my < state.current_tab_bar_height() {
                    let direction = match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            if x.abs() > y.abs() {
                                if x > 0.0 {
                                    Some(true)
                                } else if x < 0.0 {
                                    Some(false)
                                } else {
                                    None
                                }
                            } else {
                                if y < 0.0 {
                                    Some(true)
                                } else if y > 0.0 {
                                    Some(false)
                                } else {
                                    None
                                }
                            }
                        }
                        MouseScrollDelta::PixelDelta(pos) => {
                            let x = pos.x as f32;
                            let y = pos.y as f32;
                            if x.abs() > y.abs() {
                                if x > 0.0 {
                                    Some(true)
                                } else if x < 0.0 {
                                    Some(false)
                                } else {
                                    None
                                }
                            } else {
                                if y < 0.0 {
                                    Some(true)
                                } else if y > 0.0 {
                                    Some(false)
                                } else {
                                    None
                                }
                            }
                        }
                    };
                    if let Some(cycle_forward) = direction {
                        if cycle_forward {
                            state.cycle_tab_next();
                        } else {
                            state.cycle_tab_prev();
                        }
                        state.begin_redraw();
                        return;
                    }
                }

                if let Some((pane_id, col, row)) =
                    state.pane_cell_from_mouse(state.mouse_pos.0, state.mouse_pos.1)
                {
                    state.active_tab_mut().tree.active_id = pane_id;
                    if state.report_mouse_wheel(delta, col, row) {
                        // A stationary pointer now covers a different history
                        // row; while dragging, extend to that newly shown row.
                        if state.is_drag_selecting {
                            state.update_selection_end(state.mouse_pos.0, state.mouse_pos.1);
                        }
                        state.begin_redraw();
                    }
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key,
                        logical_key,
                        state: key_state,
                        text,
                        ..
                    },
                ..
            } => {
                match physical_key {
                    PhysicalKey::Code(KeyCode::ShiftLeft) => {
                        state.left_shift_down = key_state == ElementState::Pressed;
                    }
                    PhysicalKey::Code(KeyCode::ShiftRight) => {
                        state.right_shift_down = key_state == ElementState::Pressed;
                    }
                    PhysicalKey::Code(KeyCode::ControlLeft) => {
                        state.left_control_down = key_state == ElementState::Pressed;
                    }
                    PhysicalKey::Code(KeyCode::ControlRight) => {
                        state.right_control_down = key_state == ElementState::Pressed;
                    }
                    PhysicalKey::Code(KeyCode::SuperLeft) => {
                        state.left_super_down = key_state == ElementState::Pressed;
                    }
                    PhysicalKey::Code(KeyCode::SuperRight) => {
                        state.right_super_down = key_state == ElementState::Pressed;
                    }
                    _ => {}
                }

                if key_state != ElementState::Pressed {
                    return;
                }

                let ctrl = state.ctrl_down();
                let super_key = state.super_down();
                let shift = state.shift_down();
                let alt = state.modifiers.alt_key();
                // The theme editor owns the keyboard. App-level Cmd shortcuts
                // (quit, windows, tabs, font size, reload) still work.
                if state.theme_editor.is_some() {
                    if state
                        .keybindings
                        .lookup(physical_key, ctrl, alt, shift, super_key)
                        == Some(volt_config::keybindings::Action::CustomizeTheme)
                    {
                        state.cancel_theme_editor();
                        return;
                    }
                    let app_shortcut = super_key
                        && !ctrl
                        && match physical_key {
                            PhysicalKey::Code(code) => {
                                matches!(
                                    code,
                                    KeyCode::KeyQ
                                        | KeyCode::KeyN
                                        | KeyCode::KeyW
                                        | KeyCode::KeyT
                                        | KeyCode::Comma
                                        | KeyCode::Equal
                                        | KeyCode::Minus
                                        | KeyCode::NumpadAdd
                                        | KeyCode::NumpadSubtract
                                        | KeyCode::Digit1
                                        | KeyCode::Digit2
                                        | KeyCode::Digit3
                                        | KeyCode::Digit4
                                        | KeyCode::Digit5
                                        | KeyCode::Digit6
                                        | KeyCode::Digit7
                                        | KeyCode::Digit8
                                        | KeyCode::Digit9
                                ) || (shift && code == KeyCode::KeyR)
                            }
                            _ => false,
                        };
                    if !app_shortcut {
                        if let Some((name, overwrite)) = state.theme_editor_key(
                            physical_key,
                            text.as_ref().map(|t| t.as_str()),
                            super_key,
                            shift,
                            ctrl,
                        ) {
                            self.save_edited_theme(window_id, name, overwrite);
                        }
                        return;
                    }
                }
                // Local text fields and inspector navigation own their keys.
                let custom = if crate::keybindings::terminal_owns_keys(
                    state.active_prompt.is_some() || state.theme_editor.is_some(),
                    state.workspace_search.visible,
                    state.workspace_panel.visible,
                    state.workspace_panel.focused,
                ) {
                    state
                        .keybindings
                        .lookup(physical_key, ctrl, alt, shift, super_key)
                } else {
                    None
                };
                let defaults = custom != Some(volt_config::keybindings::Action::Unbind);
                if let Some(action) =
                    custom.filter(|a| *a != volt_config::keybindings::Action::Unbind)
                {
                    use volt_config::keybindings::Action as A;
                    match action {
                        A::Quit => event_loop.exit(),
                        A::ReloadConfig => {
                            let _ = state;
                            self.reload_config();
                        }
                        A::NewWindow => {
                            let _ = state;
                            let _ = self.create_main_window(event_loop, None);
                        }
                        A::ClosePane => {
                            if state.close_active_pane_or_tab() {
                                let _ = state;
                                self.windows.remove(&window_id);
                                if self.windows.is_empty() {
                                    event_loop.exit();
                                }
                            }
                        }
                        A::IncreaseFontSize | A::DecreaseFontSize => {
                            state.run_keybinding(action);
                            self.config.font.size = state.config.font.size;
                        }
                        A::CustomizeTheme => {
                            let _ = state;
                            self.toggle_theme_editor(window_id);
                        }
                        _ => state.run_keybinding(action),
                    }
                    return;
                }
                if defaults
                    && super_key
                    && shift
                    && physical_key == PhysicalKey::Code(KeyCode::KeyP)
                {
                    state.toggle_search();
                    return;
                }
                // Prompt navigation is an application action, never text sent
                // to a TUI. Modal inputs retain ownership; overrides/unbind win.
                if defaults
                    && super_key
                    && shift
                    && !ctrl
                    && !alt
                    && crate::keybindings::terminal_owns_keys(
                        state.active_prompt.is_some(),
                        state.workspace_search.visible,
                        state.workspace_panel.visible,
                        state.workspace_panel.focused,
                    )
                {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            state.jump_prompt(true);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            state.jump_prompt(false);
                            return;
                        }
                        _ => {}
                    }
                }
                // Cmd+F opens terminal Find regardless of whether the workspace
                // search palette happens to be open. This used to be handled
                // only inside the `workspace_search.visible` branch below,
                // which meant Cmd+F silently did nothing the rest of the time
                // — the common case.
                #[cfg(target_os = "macos")]
                if defaults
                    && super_key
                    && !ctrl
                    && !alt
                    && !shift
                    && physical_key == PhysicalKey::Code(KeyCode::KeyF)
                {
                    state.open_find_prompt();
                    return;
                }
                if state.workspace_search.visible {
                    if super_key {
                        match physical_key {
                            PhysicalKey::Code(KeyCode::KeyA) => {
                                state.workspace_search.query_selected = true
                            }
                            PhysicalKey::Code(KeyCode::KeyV) => state.paste_clipboard(),
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                state.workspace_search.input = crate::prompt::TextPrompt::new(
                                    crate::prompt::PromptKind::Find,
                                    "",
                                );
                                state.workspace_search.query_selected = false;
                                state.workspace_search.changed();
                            }
                            _ => {}
                        }
                    } else {
                        match physical_key {
                            PhysicalKey::Code(KeyCode::Escape) => state.workspace_search.close(),
                            PhysicalKey::Code(KeyCode::ArrowDown) => {
                                state.workspace_search.select(1)
                            }
                            PhysicalKey::Code(KeyCode::ArrowUp) => {
                                state.workspace_search.select(-1)
                            }
                            PhysicalKey::Code(KeyCode::Tab) => {
                                let i = state.workspace_search.scope.index();
                                state.workspace_search.set_scope(
                                    crate::workspace_search::Scope::ALL
                                        [(i + if shift { 5 } else { 1 }) % 6],
                                );
                            }
                            PhysicalKey::Code(KeyCode::Enter) => state.activate_search_result(),
                            PhysicalKey::Code(KeyCode::Backspace) => {
                                state.workspace_search.delete_query(true);
                            }
                            PhysicalKey::Code(KeyCode::Delete) => {
                                state.workspace_search.delete_query(false);
                            }
                            PhysicalKey::Code(KeyCode::ArrowLeft) => {
                                if state.workspace_search.query_selected {
                                    state.workspace_search.input.cursor = 0;
                                } else {
                                    state.workspace_search.input.move_left();
                                }
                                state.workspace_search.query_selected = false;
                            }
                            PhysicalKey::Code(KeyCode::ArrowRight) => {
                                if state.workspace_search.query_selected {
                                    state.workspace_search.input.cursor =
                                        state.workspace_search.input.text().chars().count();
                                } else {
                                    state.workspace_search.input.move_right();
                                }
                                state.workspace_search.query_selected = false;
                            }
                            PhysicalKey::Code(KeyCode::Home) => {
                                state.workspace_search.query_selected = false;
                                state.workspace_search.input.cursor = 0
                            }
                            PhysicalKey::Code(KeyCode::End) => {
                                state.workspace_search.query_selected = false;
                                state.workspace_search.input.cursor =
                                    state.workspace_search.input.text().chars().count()
                            }
                            _ => {
                                if !ctrl && !alt {
                                    if let Some(text) = text.as_ref() {
                                        state.workspace_search.edit(text.as_str());
                                    }
                                }
                            }
                        }
                    }
                    state.sync_search();
                    return;
                }

                if state.workspace_panel.visible
                    && state.workspace_panel.focused
                    && state.renderer.workspace_card_layout().is_some()
                    && state.active_prompt.is_none()
                    && !super_key
                {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            state.workspace_panel.focused = false;
                            state.workspace_panel.hover = None;
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown | KeyCode::Tab | KeyCode::ArrowUp) => {
                            let card = state.workspace_panel.card();
                            let actions = card.actions();
                            if !actions.is_empty() {
                                let old = actions
                                    .iter()
                                    .position(|a| Some(*a) == state.workspace_panel.hover);
                                let back =
                                    physical_key == PhysicalKey::Code(KeyCode::ArrowUp) || shift;
                                let i = match old {
                                    Some(i) if back => (i + actions.len() - 1) % actions.len(),
                                    Some(i) => (i + 1) % actions.len(),
                                    None => 0,
                                };
                                state.workspace_panel.hover = Some(actions[i]);
                                if let Some(l) = state.renderer.workspace_card_layout() {
                                    let row = card
                                        .rows
                                        .iter()
                                        .position(|r| r.action == Some(actions[i]))
                                        .unwrap_or(0);
                                    if row < state.workspace_panel.scroll {
                                        state.workspace_panel.scroll = row;
                                    }
                                    if row >= state.workspace_panel.scroll + l.count {
                                        state.workspace_panel.scroll = row + 1 - l.count;
                                    }
                                }
                            }
                        }
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::Space) => {
                            if let Some(id) = state.workspace_panel.hover {
                                state.activate_workspace_action(id);
                            }
                        }
                        _ => {}
                    }
                    state.sync_workspace_card();
                    state.begin_redraw();
                    return;
                }

                // A Find/rename overlay is active — it captures all keyboard
                // input until confirmed or cancelled. This must fully return
                // in every branch: falling through to terminal input here
                // would silently leak keystrokes to the PTY while the user
                // believes they're typing into the overlay.
                if state.active_prompt.is_some() {
                    if state.search_dirty
                        && matches!(
                            physical_key,
                            PhysicalKey::Code(
                                KeyCode::Enter
                                    | KeyCode::NumpadEnter
                                    | KeyCode::ArrowUp
                                    | KeyCode::ArrowDown
                            )
                        )
                    {
                        state.recompute_search_matches();
                    }
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            state.active_prompt = None;
                            state.search_dirty = false;
                            state.search_target = None;
                            state.begin_redraw();
                        }
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                            match state.active_prompt.as_ref().map(|p| p.kind) {
                                Some(crate::prompt::PromptKind::Find) => {
                                    if let Some(prompt) = state.active_prompt.as_mut() {
                                        if shift {
                                            prompt.prev_match();
                                        } else {
                                            prompt.next_match();
                                        }
                                    }
                                    state.scroll_to_current_match();
                                }
                                Some(_) => state.confirm_active_prompt(),
                                None => {}
                            }
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.backspace();
                            }
                            state.on_prompt_text_changed();
                        }
                        PhysicalKey::Code(KeyCode::Delete) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.delete_forward();
                            }
                            state.on_prompt_text_changed();
                        }
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.move_left();
                            }
                            state.begin_redraw();
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.move_right();
                            }
                            state.begin_redraw();
                        }
                        PhysicalKey::Code(KeyCode::Home) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.move_home();
                            }
                            state.begin_redraw();
                        }
                        PhysicalKey::Code(KeyCode::End) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.move_end();
                            }
                            state.begin_redraw();
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.next_match();
                            }
                            state.scroll_to_current_match();
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            if let Some(prompt) = state.active_prompt.as_mut() {
                                prompt.prev_match();
                            }
                            state.scroll_to_current_match();
                        }
                        _ => {
                            if let Some(text) = text.as_ref() {
                                if let Some(prompt) = state.active_prompt.as_mut() {
                                    for c in text.chars() {
                                        prompt.insert_char(c);
                                    }
                                }
                                state.on_prompt_text_changed();
                            }
                        }
                    }
                    return;
                }

                let reload_modifier = {
                    #[cfg(target_os = "macos")]
                    {
                        super_key
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        ctrl
                    }
                };

                if defaults
                    && matches!(physical_key, PhysicalKey::Code(KeyCode::KeyR))
                    && shift
                    && reload_modifier
                {
                    let _ = state;
                    self.reload_config();
                    return;
                }

                // ── global shortcuts ─────────────────────────────────────────
                if defaults && super_key {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Tab) => {
                            if shift {
                                state.cycle_tab_prev();
                            } else {
                                state.cycle_tab_next();
                            }
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::BracketLeft) if shift => {
                            state.cycle_tab_prev();
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::BracketRight) if shift => {
                            state.cycle_tab_next();
                            state.begin_redraw();
                            return;
                        }
                        // Cmd+, → open config file in default editor
                        PhysicalKey::Code(KeyCode::Comma) => {
                            open_config_in_editor();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyD) => {
                            let direction = if shift {
                                PaneSplitDirection::Horizontal
                            } else {
                                PaneSplitDirection::Vertical
                            };
                            state.divider_drag = None;
                            if state.split_active_tab(direction) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyA) if shift => {
                            // Cmd+Shift+A: toggle the inset workspace inspector
                            state.toggle_workspace_panel();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowLeft) if alt => {
                            if state.move_focus_in_direction(PaneFocusDirection::Left) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) if alt => {
                            if state.move_focus_in_direction(PaneFocusDirection::Right) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) if alt => {
                            if state.move_focus_in_direction(PaneFocusDirection::Up) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) if alt => {
                            if state.move_focus_in_direction(PaneFocusDirection::Down) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Equal | KeyCode::NumpadAdd) => {
                            if state.adjust_font_size(1.0) {
                                self.config.font.size = state.config.font.size;
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Minus | KeyCode::NumpadSubtract) => {
                            if state.adjust_font_size(-1.0) {
                                self.config.font.size = state.config.font.size;
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            #[cfg(target_os = "macos")]
                            if state.config.appearance.native_tabs {
                                // Open a new window — macOS groups it as a native tab.
                                if let Some(proxy) = self.proxy.as_ref() {
                                    let _ = proxy.send_event(VoltEvent::CreateNewWindow);
                                }
                                return;
                            }
                            state.new_tab();
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyW) => {
                            if state.close_active_pane_or_tab() {
                                let _ = state;
                                self.windows.remove(&window_id);
                                if self.windows.is_empty() {
                                    event_loop.exit();
                                }
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            // Copy only — never fall back to SIGINT; a missed
                            // selection must not interrupt a running process.
                            if state.copy_selection() {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyV) => {
                            state.paste_clipboard();
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            event_loop.exit();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyN) => {
                            #[cfg(target_os = "macos")]
                            {
                                if let Some(proxy) = self.proxy.as_ref() {
                                    let _ = proxy.send_event(VoltEvent::CreateNewWindow);
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                state.new_tab();
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyK) => {
                            // Clear screen and scrollback, then ask the shell to
                            // repaint its prompt (form feed).
                            {
                                let pane = state.active_pane_mut();
                                pane.scroll_view_offset = 0;
                                match pane.performer.lock() {
                                    Ok(mut p) => {
                                        p.grid.clear_scrollback();
                                        p.grid.clear_screen();
                                    }
                                    Err(err) => eprintln!(
                                        "volt-ui: failed to lock performer for clear: {err}"
                                    ),
                                }
                            }
                            state.selection = None;
                            state.send_pty_input(&[0x0c]);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            // macOS line-start convention → readline beginning-of-line.
                            state.send_pty_input(&[0x01]);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            // macOS line-end convention → readline end-of-line.
                            state.send_pty_input(&[0x05]);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Backspace) => {
                            // macOS delete-to-line-start → readline unix-line-discard.
                            state.send_pty_input(&[0x15]);
                            return;
                        }
                        PhysicalKey::Code(
                            code @ (KeyCode::Digit1
                            | KeyCode::Digit2
                            | KeyCode::Digit3
                            | KeyCode::Digit4
                            | KeyCode::Digit5
                            | KeyCode::Digit6
                            | KeyCode::Digit7
                            | KeyCode::Digit8
                            | KeyCode::Digit9),
                        ) => {
                            let idx = match code {
                                KeyCode::Digit1 => 0,
                                KeyCode::Digit2 => 1,
                                KeyCode::Digit3 => 2,
                                KeyCode::Digit4 => 3,
                                KeyCode::Digit5 => 4,
                                KeyCode::Digit6 => 5,
                                KeyCode::Digit7 => 6,
                                KeyCode::Digit8 => 7,
                                _ => 8,
                            };
                            state.switch_tab(idx);
                            state.begin_redraw();
                            return;
                        }
                        _ => {}
                    }
                    // Swallow any unhandled Cmd combination — without this the
                    // text fallthrough below types the bare letter into the shell.
                    return;
                }

                if defaults && ctrl && shift {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            if state.copy_selection() {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyV) => {
                            state.paste_clipboard();
                            state.begin_redraw();
                            return;
                        }
                        _ => {}
                    }
                }

                if defaults && alt && ctrl {
                    let moved = match physical_key {
                        PhysicalKey::Code(KeyCode::ArrowLeft) => {
                            state.move_focus_in_direction(PaneFocusDirection::Left)
                        }
                        PhysicalKey::Code(KeyCode::ArrowRight) => {
                            state.move_focus_in_direction(PaneFocusDirection::Right)
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => {
                            state.move_focus_in_direction(PaneFocusDirection::Up)
                        }
                        PhysicalKey::Code(KeyCode::ArrowDown) => {
                            state.move_focus_in_direction(PaneFocusDirection::Down)
                        }
                        _ => false,
                    };
                    if moved {
                        state.begin_redraw();
                        return;
                    }
                }

                if defaults && ctrl && matches!(physical_key, PhysicalKey::Code(KeyCode::Tab)) {
                    if shift {
                        state.cycle_tab_prev();
                    } else {
                        state.cycle_tab_next();
                    }
                    state.begin_redraw();
                    return;
                }

                // ── terminal input ────────────────────────────────────────────
                let special = match physical_key {
                    PhysicalKey::Code(code) => terminal_special_sequence(
                        code,
                        shift,
                        alt,
                        ctrl,
                        state.active_application_cursor_keys_mode(),
                    ),
                    _ => None,
                };

                if let Some(bytes) = special {
                    if matches!(
                        physical_key,
                        PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter)
                    ) {
                        state.active_pane_mut().running = true;
                    }
                    state.send_pty_input(&bytes);
                    return;
                }

                if ctrl {
                    if let PhysicalKey::Code(code) = physical_key {
                        if let Some(b) = ctrl_code(code) {
                            let mut bytes = Vec::with_capacity(2);
                            if alt {
                                bytes.push(0x1b);
                            }
                            bytes.push(b);
                            state.send_pty_input(&bytes);
                            return;
                        }
                    }
                }

                if let Some(text) = text {
                    state.send_pty_input(text.as_str().as_bytes());
                    return;
                }

                if let Some(fallback_text) = logical_key.to_text() {
                    state.send_pty_input(fallback_text.as_bytes());
                }
            }

            WindowEvent::RedrawRequested => {
                state.redraw_pending = false;
                let active = state.active_tab;
                let mut needs_full_redraw = false;
                let mut pending_partial_damage: Option<(usize, usize)> = None;
                let mut active_grid_changed = false;
                // (tab_idx, pane_id)
                let mut closed_panes: Vec<(usize, usize)> = Vec::new();

                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    let mut pty_errors: Vec<String> = Vec::new();
                    tab.tree.for_each_leaf_mut(&mut |pane_id, pane| {
                        while let Ok(ev) = pane.event_rx.try_recv() {
                            match ev {
                                CoreEvent::GridUpdated { damaged_rows } => {
                                    if i == active {
                                        active_grid_changed = true;
                                        if let Some((start, end)) = damaged_rows {
                                            pending_partial_damage =
                                                Some(match pending_partial_damage {
                                                    Some((cur_start, cur_end)) => {
                                                        (cur_start.min(start), cur_end.max(end))
                                                    }
                                                    None => (start, end),
                                                });
                                        } else {
                                            needs_full_redraw = true;
                                        }
                                    }
                                }
                                CoreEvent::CwdChanged(path) => {
                                    pane.cwd = Some(path);
                                }
                                CoreEvent::TitleChanged(t) => {
                                    pane.title = t;
                                }
                                CoreEvent::CommandStarted => {
                                    pane.running = true;
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    pane.running = false;
                                }
                                CoreEvent::PtyError(msg) => {
                                    if i == active {
                                        pty_errors.push(msg);
                                    }
                                }
                                CoreEvent::PtyClosed => closed_panes.push((i, pane_id)),
                            }
                        }
                    });
                    // Process PtyErrors after the borrow on tab.tree is released.
                    for msg in pty_errors {
                        state.renderer.set_top_alert(Some(msg));
                        needs_full_redraw = true;
                    }
                }

                if active_grid_changed {
                    state.invalidate_search_after_output();
                }
                state.invalidate_search_if_target_changed();
                if state.search_dirty
                    && state.last_search_refresh.elapsed() >= Duration::from_millis(200)
                {
                    state.recompute_search_matches();
                }

                // Handle closed panes
                if !closed_panes.is_empty() {
                    // Process from highest tab index to lowest to avoid index shifting
                    closed_panes.sort_unstable();
                    closed_panes.dedup();
                    for (tab_idx, pane_id) in closed_panes.into_iter().rev() {
                        let should_close_tab = state.remove_pane_from_tab(tab_idx, pane_id);
                        if should_close_tab {
                            if state.tabs.len() <= 1 {
                                close_window_after_event = true;
                                break;
                            }
                            state.close_tab(tab_idx);
                        }
                    }
                    if !close_window_after_event {
                        needs_full_redraw = true;
                    }
                }

                if !close_window_after_event {
                    if needs_full_redraw {
                        state.mark_full_redraw();
                    } else {
                        state.mark_partial_redraw(pending_partial_damage);
                    }

                    let tab_titles: Vec<String> = state
                        .tabs
                        .iter()
                        .enumerate()
                        .map(|(i, t)| t.display_title(i + 1))
                        .collect();
                    let tab_entries: Vec<TabEntry> = tab_titles
                        .iter()
                        .enumerate()
                        .map(|(i, title)| TabEntry {
                            title,
                            active: i == state.active_tab,
                            index: i + 1,
                            busy: state.tabs[i].is_busy(),
                            pane_count: state.tabs[i].pane_count(),
                        })
                        .collect();
                    #[cfg(target_os = "macos")]
                    if let Some(active_title) = tab_titles.get(state.active_tab) {
                        state.set_window_title_cached(active_title);
                    }

                    let Some((render_grid, render_cursor_visible)) =
                        state.build_render_grid_for_active_tab()
                    else {
                        eprintln!("volt-ui: failed to snapshot panes for render");
                        return;
                    };
                    let damage_rows = state.take_render_damage_rows();
                    let dividers: Vec<volt_renderer::PaneDivider> = state
                        .active_tab_dividers()
                        .into_iter()
                        .map(|d| d.phys)
                        .collect();
                    let search_match = state.current_match_tuple();
                    let prompt_text = state.active_prompt.as_ref().map(|p| p.text());
                    let prompt_overlay =
                        state
                            .active_prompt
                            .as_ref()
                            .map(|p| volt_renderer::PromptOverlay {
                                title: p.title(),
                                read_only: p.kind == crate::prompt::PromptKind::OpenLink,
                                text: prompt_text.as_deref().unwrap_or(""),
                                cursor: p.cursor,
                                match_count: p.matches.len(),
                                matches_truncated: p.matches_truncated,
                                current_match: p.current_match,
                            });
                    let inspector_info = if state.show_inspector {
                        let pane = state.active_tab().active_pane();
                        pane.performer
                            .lock()
                            .ok()
                            .map(|p| volt_renderer::InspectorInfo {
                                cols: p.grid.cols,
                                rows: p.grid.rows,
                                cursor_col: p.grid.cursor_col,
                                cursor_row: p.grid.cursor_row,
                                scrollback_len: p.grid.scrollback_len(),
                            })
                    } else {
                        None
                    };
                    let active_read_only = state.active_tab().active_pane().read_only;
                    state.renderer.render_frame(
                        &render_grid,
                        &state.theme,
                        &tab_entries,
                        state.effective_cursor_visible(render_cursor_visible),
                        state.selection_tuple(),
                        state.selection_is_block(),
                        damage_rows,
                        &dividers,
                        search_match,
                        prompt_overlay,
                        inspector_info,
                        active_read_only,
                    );
                    state.render_snapshot = Some(render_grid);
                    if state.post_resize_redraws > 0 {
                        state.post_resize_redraws -= 1;
                        state.mark_full_redraw();
                        state.queue_redraw();
                    }
                }
            }

            _ => {}
        }

        if close_window_after_event {
            self.windows.remove(&window_id);
            if self.windows.is_empty() {
                event_loop.exit();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Drain any PTY events that arrived between redraws.
        // The EventLoopProxy already triggers user_event → request_redraw,
        // so this is just a safety drain for events that slipped through.
        if self.windows.is_empty() {
            event_loop.exit();
            return;
        }

        let mut next_blink_deadline: Option<Instant> = None;
        let mut windows_to_close = Vec::new();

        for (window_id, state) in self.windows.iter_mut() {
            let cwd = state.active_tab().active_pane().cwd.clone();
            let pid = state.active_tab().active_pane().pty.child_pid();
            if state.workspace_search.visible
                && (state.workspace_search.target != pid
                    || pid
                        .and_then(crate::workspace_panel::local_process_cwd)
                        .or_else(|| cwd.clone())
                        != state.workspace_search.cwd)
            {
                state.workspace_search.close();
                state.sync_search();
            }
            if state.workspace_search.visible {
                let performer = state.active_tab().active_pane().performer.clone();
                let query = state.workspace_search.input.text();
                let scope = state.workspace_search.scope;
                let search_cwd = state.workspace_search.cwd.clone();
                let snapshot = state.workspace_panel.search_snapshot(&search_cwd, pid);
                if state.workspace_search.tick(
                    || crate::workspace_search::SearchInput {
                        query,
                        scope,
                        cwd: search_cwd,
                        snapshot: snapshot.cloned(),
                        performer,
                    },
                    &state.proxy,
                ) {
                    state.sync_search();
                }
                if let Some(deadline) = state.workspace_search.deadline() {
                    next_blink_deadline =
                        Some(next_blink_deadline.map_or(deadline, |d| d.min(deadline)));
                }
            }
            if state.workspace_panel.tick(cwd, pid, &state.proxy) {
                let previous_grid = state.current_grid_size();
                state.sync_workspace_card();
                if previous_grid != state.current_grid_size() {
                    state.resize_all_tabs_to_current_grid();
                }
                state.begin_redraw();
            }
            if let Some(deadline) = state.workspace_panel.refresh_deadline() {
                next_blink_deadline =
                    Some(next_blink_deadline.map_or(deadline, |d| d.min(deadline)));
            }
            #[cfg(target_os = "macos")]
            {
                let native_tab_count = state.window.num_tabs().max(1);
                if native_tab_count != state.last_known_native_tab_count {
                    state.last_known_native_tab_count = native_tab_count;
                    configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                    state.resize_all_tabs_to_current_grid();
                    state.begin_redraw();
                } else if native_tab_count > 1 {
                    // AppKit may attach/move the tab accessory one tick later.
                    // Keep this idempotent sync so pinning and + visibility settle.
                    configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                }
                state.sync_native_window_title();
            }
            let active = state.active_tab;
            let show_tab_chrome = state.show_custom_tab_bar();
            let mut needs_redraw = false;
            let mut needs_full_redraw = false;
            let mut pending_partial_damage: Option<(usize, usize)> = None;
            let mut active_grid_changed = false;
            let mut closed_panes: Vec<(usize, usize)> = Vec::new();

            if !state.redraw_pending {
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    let mut pty_errors: Vec<String> = Vec::new();
                    tab.tree.for_each_leaf_mut(&mut |pane_id, pane| {
                        while let Ok(ev) = pane.event_rx.try_recv() {
                            match ev {
                                CoreEvent::GridUpdated { damaged_rows } => {
                                    if i == active {
                                        active_grid_changed = true;
                                        needs_redraw = true;
                                        if let Some((start, end)) = damaged_rows {
                                            pending_partial_damage =
                                                Some(match pending_partial_damage {
                                                    Some((cur_start, cur_end)) => {
                                                        (cur_start.min(start), cur_end.max(end))
                                                    }
                                                    None => (start, end),
                                                });
                                        } else {
                                            needs_full_redraw = true;
                                        }
                                    }
                                }
                                CoreEvent::CwdChanged(path) => {
                                    pane.cwd = Some(path);
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::TitleChanged(t) => {
                                    pane.title = t;
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::CommandStarted => {
                                    pane.running = true;
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    pane.running = false;
                                }
                                CoreEvent::PtyError(msg) => {
                                    if i == active {
                                        pty_errors.push(msg);
                                    }
                                }
                                CoreEvent::PtyClosed => closed_panes.push((i, pane_id)),
                            }
                        }
                    });
                    // Process PtyErrors after the borrow on tab.tree is released.
                    for msg in pty_errors {
                        state.renderer.set_top_alert(Some(msg));
                        needs_redraw = true;
                        needs_full_redraw = true;
                    }
                }
            }

            if active_grid_changed {
                state.invalidate_search_after_output();
            }
            state.invalidate_search_if_target_changed();
            if state.search_dirty {
                let deadline = state.last_search_refresh + Duration::from_millis(200);
                if Instant::now() >= deadline {
                    state.recompute_search_matches();
                    needs_redraw = true;
                    needs_full_redraw = true;
                } else {
                    next_blink_deadline = Some(match next_blink_deadline {
                        Some(current) => current.min(deadline),
                        None => deadline,
                    });
                }
            }

            if !closed_panes.is_empty() {
                closed_panes.sort_unstable();
                closed_panes.dedup();
                for (tab_idx, pane_id) in closed_panes.into_iter().rev() {
                    let should_close_tab = state.remove_pane_from_tab(tab_idx, pane_id);
                    if should_close_tab {
                        if state.tabs.len() <= 1 {
                            windows_to_close.push(*window_id);
                            break;
                        }
                        state.close_tab(tab_idx);
                    }
                }
                needs_redraw = true;
                needs_full_redraw = true;
            }

            if state.config.appearance.cursor_blink {
                if Instant::now() >= state.next_cursor_blink {
                    state.blink_state = !state.blink_state;
                    state.next_cursor_blink = Instant::now() + CURSOR_BLINK_INTERVAL;
                    needs_redraw = true;
                    needs_full_redraw = true;
                }
                next_blink_deadline = Some(match next_blink_deadline {
                    Some(current) => current.min(state.next_cursor_blink),
                    None => state.next_cursor_blink,
                });
            } else {
                if !state.blink_state {
                    state.blink_state = true;
                    needs_redraw = true;
                    needs_full_redraw = true;
                }
                state.next_cursor_blink = Instant::now() + CURSOR_BLINK_INTERVAL;
            }

            if let Some(deadline) = state.tick_selection_auto_scroll() {
                next_blink_deadline =
                    Some(next_blink_deadline.map_or(deadline, |d| d.min(deadline)));
            }

            if needs_redraw {
                if needs_full_redraw {
                    state.begin_redraw();
                } else {
                    state.mark_partial_redraw(pending_partial_damage);
                    state.queue_redraw();
                }
            }
        }

        if !windows_to_close.is_empty() {
            windows_to_close.sort_unstable();
            windows_to_close.dedup();
            for window_id in windows_to_close {
                self.windows.remove(&window_id);
            }
            if self.windows.is_empty() {
                event_loop.exit();
                return;
            }
        }

        if let Some(deadline) = next_blink_deadline {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}

fn selection_scroll_direction(
    y: f32,
    top: f32,
    bottom: f32,
    cell_height: f32,
) -> Option<SelectionScrollDirection> {
    let middle = (top + bottom) * 0.5;
    if y < top + cell_height && y < middle {
        Some(SelectionScrollDirection::Up)
    } else if y >= bottom - cell_height && y >= middle {
        Some(SelectionScrollDirection::Down)
    } else {
        None
    }
}

/// Character class used for double-click word selection: 0 = whitespace,
/// 1 = word characters (incl. common path/URL chars), 2 = other punctuation.
fn char_select_class(c: char) -> u8 {
    if c == ' ' {
        0
    } else if c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '~' | ':' | '@' | '+') {
        1
    } else {
        2
    }
}

/// Remove any embedded bracketed-paste terminator so pasted content cannot
/// break out of the ESC[200~ … ESC[201~ envelope.
#[cfg(target_os = "macos")]
fn strip_bracketed_paste_end(data: &[u8]) -> Vec<u8> {
    const END: &[u8] = b"\x1b[201~";
    let mut out = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        if data[i..].starts_with(END) {
            i += END.len();
        } else {
            out.push(data[i]);
            i += 1;
        }
    }
    out
}

/// Inherited OSC/cwd titles are not explicit overrides. Opening a rename
/// prompt and pressing Enter unchanged must leave that inheritance intact.
#[cfg(target_os = "macos")]
fn rename_initial_text(custom: Option<&str>) -> String {
    custom.unwrap_or_default().to_string()
}

/// Minimal RFC 3986 percent-encoding for a URL query value — encodes every
/// byte outside the unreserved set (this correctly handles multi-byte UTF-8
/// too, since each byte of a sequence gets its own `%XX`). Small and local
/// rather than pulling in a crate for one query string.
#[cfg(target_os = "macos")]
fn percent_encode_query(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push('%');
                out.push(HEX[(byte >> 4) as usize] as char);
                out.push(HEX[(byte & 15) as usize] as char);
            }
        }
    }
    out
}

fn xterm_modifier_param(shift: bool, alt: bool, ctrl: bool) -> Option<u8> {
    let mut value = 1u8;
    if shift {
        value = value.saturating_add(1);
    }
    if alt {
        value = value.saturating_add(2);
    }
    if ctrl {
        value = value.saturating_add(4);
    }
    (value > 1).then_some(value)
}

fn ss3_function_sequence(letter: char, modifier: Option<u8>) -> Vec<u8> {
    match modifier {
        Some(m) => format!("\x1b[1;{}{}", m, letter).into_bytes(),
        None => format!("\x1bO{}", letter).into_bytes(),
    }
}

fn csi_tilde_sequence(code: u16, modifier: Option<u8>) -> Vec<u8> {
    match modifier {
        Some(m) => format!("\x1b[{};{}~", code, m).into_bytes(),
        None => format!("\x1b[{}~", code).into_bytes(),
    }
}

fn terminal_special_sequence(
    code: KeyCode,
    shift: bool,
    alt: bool,
    ctrl: bool,
    application_cursor_keys_mode: bool,
) -> Option<Vec<u8>> {
    let modifier = xterm_modifier_param(shift, alt, ctrl);

    match code {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            let mut bytes = Vec::with_capacity(2);
            if alt {
                bytes.push(0x1b);
            }
            bytes.push(b'\r');
            Some(bytes)
        }
        KeyCode::Backspace | KeyCode::NumpadBackspace => {
            let mut bytes = Vec::with_capacity(2);
            if alt {
                bytes.push(0x1b);
            }
            bytes.push(0x7f);
            Some(bytes)
        }
        KeyCode::Tab => match modifier {
            Some(2) => Some(b"\x1b[Z".to_vec()),
            Some(m) => Some(format!("\x1b[1;{}Z", m).into_bytes()),
            None => Some(b"\t".to_vec()),
        },
        KeyCode::Escape => {
            let mut bytes = Vec::with_capacity(2);
            bytes.push(0x1b);
            if alt {
                bytes.push(0x1b);
            }
            Some(bytes)
        }
        KeyCode::ArrowUp => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}A", m).into_bytes())
            } else if application_cursor_keys_mode {
                Some(b"\x1bOA".to_vec())
            } else {
                Some(b"\x1b[A".to_vec())
            }
        }
        KeyCode::ArrowDown => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}B", m).into_bytes())
            } else if application_cursor_keys_mode {
                Some(b"\x1bOB".to_vec())
            } else {
                Some(b"\x1b[B".to_vec())
            }
        }
        KeyCode::ArrowRight => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}C", m).into_bytes())
            } else if application_cursor_keys_mode {
                Some(b"\x1bOC".to_vec())
            } else {
                Some(b"\x1b[C".to_vec())
            }
        }
        KeyCode::ArrowLeft => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}D", m).into_bytes())
            } else if application_cursor_keys_mode {
                Some(b"\x1bOD".to_vec())
            } else {
                Some(b"\x1b[D".to_vec())
            }
        }
        KeyCode::Home => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}H", m).into_bytes())
            } else {
                Some(b"\x1b[H".to_vec())
            }
        }
        KeyCode::End => {
            if let Some(m) = modifier {
                Some(format!("\x1b[1;{}F", m).into_bytes())
            } else {
                Some(b"\x1b[F".to_vec())
            }
        }
        KeyCode::Insert => Some(csi_tilde_sequence(2, modifier)),
        KeyCode::Delete => Some(csi_tilde_sequence(3, modifier)),
        KeyCode::PageUp => Some(csi_tilde_sequence(5, modifier)),
        KeyCode::PageDown => Some(csi_tilde_sequence(6, modifier)),

        KeyCode::F1 => Some(ss3_function_sequence('P', modifier)),
        KeyCode::F2 => Some(ss3_function_sequence('Q', modifier)),
        KeyCode::F3 => Some(ss3_function_sequence('R', modifier)),
        KeyCode::F4 => Some(ss3_function_sequence('S', modifier)),
        KeyCode::F5 => Some(csi_tilde_sequence(15, modifier)),
        KeyCode::F6 => Some(csi_tilde_sequence(17, modifier)),
        KeyCode::F7 => Some(csi_tilde_sequence(18, modifier)),
        KeyCode::F8 => Some(csi_tilde_sequence(19, modifier)),
        KeyCode::F9 => Some(csi_tilde_sequence(20, modifier)),
        KeyCode::F10 => Some(csi_tilde_sequence(21, modifier)),
        KeyCode::F11 => Some(csi_tilde_sequence(23, modifier)),
        KeyCode::F12 => Some(csi_tilde_sequence(24, modifier)),
        KeyCode::F13 => Some(csi_tilde_sequence(25, modifier)),
        KeyCode::F14 => Some(csi_tilde_sequence(26, modifier)),
        KeyCode::F15 => Some(csi_tilde_sequence(28, modifier)),
        KeyCode::F16 => Some(csi_tilde_sequence(29, modifier)),
        KeyCode::F17 => Some(csi_tilde_sequence(31, modifier)),
        KeyCode::F18 => Some(csi_tilde_sequence(32, modifier)),
        KeyCode::F19 => Some(csi_tilde_sequence(33, modifier)),
        KeyCode::F20 => Some(csi_tilde_sequence(34, modifier)),
        KeyCode::F21 => Some(csi_tilde_sequence(42, modifier)),
        KeyCode::F22 => Some(csi_tilde_sequence(43, modifier)),
        KeyCode::F23 => Some(csi_tilde_sequence(44, modifier)),
        KeyCode::F24 => Some(csi_tilde_sequence(45, modifier)),
        _ => None,
    }
}

fn ctrl_code(code: KeyCode) -> Option<u8> {
    let letter = match code {
        KeyCode::KeyA => b'A',
        KeyCode::KeyB => b'B',
        KeyCode::KeyC => b'C',
        KeyCode::KeyD => b'D',
        KeyCode::KeyE => b'E',
        KeyCode::KeyF => b'F',
        KeyCode::KeyG => b'G',
        KeyCode::KeyH => b'H',
        KeyCode::KeyI => b'I',
        KeyCode::KeyJ => b'J',
        KeyCode::KeyK => b'K',
        KeyCode::KeyL => b'L',
        KeyCode::KeyM => b'M',
        KeyCode::KeyN => b'N',
        KeyCode::KeyO => b'O',
        KeyCode::KeyP => b'P',
        KeyCode::KeyQ => b'Q',
        KeyCode::KeyR => b'R',
        KeyCode::KeyS => b'S',
        KeyCode::KeyT => b'T',
        KeyCode::KeyU => b'U',
        KeyCode::KeyV => b'V',
        KeyCode::KeyW => b'W',
        KeyCode::KeyX => b'X',
        KeyCode::KeyY => b'Y',
        KeyCode::KeyZ => b'Z',
        KeyCode::Space | KeyCode::Digit2 => return Some(0x00),
        KeyCode::Digit3 | KeyCode::BracketLeft => return Some(0x1b),
        KeyCode::Digit4 | KeyCode::Backslash => return Some(0x1c),
        KeyCode::Digit5 | KeyCode::BracketRight => return Some(0x1d),
        KeyCode::Digit6 => return Some(0x1e),
        KeyCode::Digit7 | KeyCode::Slash | KeyCode::Minus => return Some(0x1f),
        KeyCode::Digit8 => return Some(0x7f),
        _ => return None,
    };
    Some(letter - b'@')
}

// ── macOS app icon ────────────────────────────────────────────────────────────

/// Sets the Dock / app-switcher icon at runtime by loading the embedded
/// 512×512 PNG through Cocoa's NSImage API.
#[cfg(target_os = "macos")]
fn set_app_icon() {
    use objc::runtime::Object;
    use objc::{class, msg_send, sel, sel_impl};

    const ICON_PNG: &[u8] = include_bytes!("../../assets/icon-512.png");

    unsafe {
        let data: *mut Object = msg_send![
            class!(NSData),
            dataWithBytes: ICON_PNG.as_ptr() as *const std::ffi::c_void
            length: ICON_PNG.len()
        ];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if !image.is_null() {
            // Set logical size to 256×256pt for a 512px PNG.
            // This gives correct 2x Retina backing (512px / 2x = 256pt).
            // rsvg exports at 72 DPI, so without setSize the Dock would
            // render a 512px PNG at 512pt — far too large.
            #[repr(C)]
            #[derive(Clone, Copy)]
            struct NSSize {
                width: f64,
                height: f64,
            }
            let sz = NSSize {
                width: 256.0,
                height: 256.0,
            };
            let _: () = msg_send![image, setSize: sz];
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setApplicationIconImage: image];
            let _: () = msg_send![image, release];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::create_sample_config_if_missing;

    use super::{
        advance_drag_selection, selection_scroll_direction, Grid, MainState, Selection,
        SelectionMode, SelectionScrollDirection,
    };

    #[test]
    fn drag_edge_direction_uses_own_pane_not_window_middle() {
        use SelectionScrollDirection::{Down, Up};
        assert_eq!(
            selection_scroll_direction(100.0, 100.0, 200.0, 10.0),
            Some(Up)
        );
        assert_eq!(
            selection_scroll_direction(205.0, 100.0, 200.0, 10.0),
            Some(Down)
        );
        assert_eq!(selection_scroll_direction(150.0, 100.0, 200.0, 10.0), None);
        assert_eq!(
            selection_scroll_direction(104.0, 100.0, 110.0, 10.0),
            Some(Up)
        );
        assert_eq!(
            selection_scroll_direction(106.0, 100.0, 110.0, 10.0),
            Some(Down)
        );
    }

    #[test]
    fn edge_drag_repeats_until_history_boundary_and_keeps_anchor() {
        use SelectionScrollDirection::{Down, Up};
        let mut grid = Grid::new(4, 3);
        for _ in 0..6 {
            grid.scroll_up(0, 2, 1);
        }
        let mut offset = 0;
        let mut selection =
            Selection::at_viewport(1, false, SelectionMode::Linear, &grid, offset, 2, 2).unwrap();
        let anchor = (selection.start_col, selection.start_row);
        for _ in 0..6 {
            assert!(advance_drag_selection(
                &grid,
                &mut offset,
                &mut selection,
                1,
                false,
                0,
                0,
                Up,
            ));
        }
        assert_eq!(offset, 6);
        assert_eq!(selection.end_row, 0);
        assert!(!advance_drag_selection(
            &grid,
            &mut offset,
            &mut selection,
            1,
            false,
            0,
            0,
            Up,
        ));
        for _ in 0..6 {
            assert!(advance_drag_selection(
                &grid,
                &mut offset,
                &mut selection,
                1,
                false,
                3,
                2,
                Down,
            ));
        }
        assert_eq!(offset, 0);
        assert_eq!((selection.start_col, selection.start_row), anchor);
        assert_eq!((selection.end_col, selection.end_row), (3, anchor.1));
        assert!(!advance_drag_selection(
            &grid,
            &mut offset,
            &mut selection,
            1,
            false,
            3,
            2,
            Down,
        ));
    }

    #[test]
    fn native_text_respects_modal_input_and_read_only_panes() {
        use super::{committed_text_target as target, CommittedTextTarget::*};
        assert_eq!(target(false, false, false, false), Terminal);
        assert_eq!(target(false, false, false, true), Ignore);
        assert_eq!(target(false, false, true, false), Ignore);
        assert_eq!(target(false, false, true, true), Ignore);
        assert_eq!(target(true, false, false, false), Search);
        assert_eq!(target(true, true, true, true), Search);
        assert_eq!(target(false, true, false, false), Prompt);
        assert_eq!(target(false, true, false, true), Prompt);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tui_right_click_is_forwarded_unless_shift_opens_volt_menu() {
        assert!(!super::should_show_context_menu(false, true, false));
        assert!(super::should_show_context_menu(true, true, false));
        assert!(super::should_show_context_menu(false, false, false));
        assert!(super::should_show_context_menu(false, true, true));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rename_prompt_only_prefills_explicit_overrides() {
        assert_eq!(super::rename_initial_text(None), "");
        assert_eq!(super::rename_initial_text(Some("Project")), "Project");
        assert_eq!(super::rename_initial_text(Some("~")), "~");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn search_query_encoding_handles_utf8_and_reserved_bytes() {
        assert_eq!(super::percent_encode_query("日 a&"), "%E6%97%A5%20a%26");
        assert_eq!(super::percent_encode_query("a-Z_1.~"), "a-Z_1.~");
    }

    #[test]
    fn opening_settings_never_replaces_existing_markerless_config() {
        let dir = std::env::temp_dir().join(format!(
            "volt-config-preserve-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("config.toml");
        std::fs::create_dir_all(&dir).unwrap();
        let custom = "theme = \"dracula\"\n# My own settings\n";
        std::fs::write(&path, custom).unwrap();
        create_sample_config_if_missing(&path).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), custom);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn opening_settings_creates_sample_only_when_missing() {
        let dir = std::env::temp_dir().join(format!(
            "volt-config-create-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("config.toml");
        create_sample_config_if_missing(&path).unwrap();
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("# Volt Terminal"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reused_snapshot_matches_fresh_across_panes_unicode_history_and_resize() {
        let mut source = volt_core::performer::Performer::new(8, 3);
        // Create clusters directly so the UI test does not depend on VTE.
        source.grid.set_grapheme(1, 0, "👩‍💻", 2);
        source.grid.set_grapheme(4, 0, "e\u{301}", 1);
        source.grid.scroll_up(0, 2, 1);
        source.grid.set_grapheme(2, 1, "🇮🇳", 2);
        let other = Grid::new(3, 2);
        let mut reused = Grid::new(20, 6);
        for (frame, (cols, rows)) in [(20, 6), (20, 6), (12, 4), (1, 1), (20, 6)]
            .into_iter()
            .enumerate()
        {
            let mut fresh = Grid::new(cols, rows);
            reused.prepare_for_snapshot(cols, rows);
            for dst in [&mut fresh, &mut reused] {
                if frame % 2 == 0 {
                    MainState::blit_grid_with_scrollback(dst, &source.grid, 0, 0, 3, 1);
                } else {
                    MainState::blit_grid(dst, &source.grid, 1, 1);
                }
                MainState::blit_grid(dst, &other, 9, 2);
            }
            assert_eq!(fresh.dirty_rows(), reused.dirty_rows());
            for row in 0..rows {
                assert_eq!(fresh.row_cells(row), reused.row_cells(row));
                assert_eq!(
                    fresh.row_text(fresh.row_cells(row)),
                    reused.row_text(reused.row_cells(row))
                );
            }
        }
    }

    #[test]
    #[ignore = "manual CPU snapshot benchmark, not GPU throughput"]
    fn benchmark_snapshot_allocation_vs_reuse() {
        for (cols, rows) in [(91, 16), (200, 60)] {
            let source = Grid::new(cols, rows);
            let mut scratch = Grid::new(cols, rows);
            for reuse in [false, true, true, false] {
                let start = std::time::Instant::now();
                let iterations = 10_000;
                for _ in 0..iterations {
                    if reuse {
                        scratch.prepare_for_snapshot(cols, rows);
                        MainState::blit_grid(&mut scratch, &source, 0, 0);
                        std::hint::black_box(&scratch);
                    } else {
                        let mut fresh = Grid::new(cols, rows);
                        MainState::blit_grid(&mut fresh, &source, 0, 0);
                        std::hint::black_box(&fresh);
                    }
                }
                eprintln!(
                    "snapshot {cols}x{rows} reuse={reuse}: {:.3} us/frame",
                    start.elapsed().as_secs_f64() * 1e6 / iterations as f64
                );
            }
        }
    }

    /// Measures render-grid composition, not GPU present. Run explicitly
    /// with the ignored release tests to track this hot path over time.
    #[test]
    #[ignore]
    fn benchmark_render_grid_blit() {
        let src = Grid::new(200, 60);
        let mut dst = Grid::new(200, 60);
        let iterations = 2_000;
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            MainState::blit_grid(&mut dst, &src, 0, 0);
            std::hint::black_box(&dst);
        }
        let elapsed = start.elapsed();
        let cells = iterations * src.cols * src.rows;
        eprintln!(
            "render-grid blit: {cells} cells in {elapsed:?} ({:.1} M cells/s)",
            cells as f64 / elapsed.as_secs_f64() / 1_000_000.0
        );
    }
}
