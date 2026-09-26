use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
#[cfg(target_os = "macos")]
use winit::platform::macos::{ActiveEventLoopExtMacOS, WindowExtMacOS};
use winit::window::{Window, WindowId};

use volt_config::{config_path_to_edit, sample_config_toml, Config, Theme};
use volt_core::events::CoreEvent;
use volt_core::grid::Grid;
use volt_core::performer::MouseTrackingMode;

use volt_renderer::{Renderer, TabEntry};

use crate::chat_panel::ChatPanel;
#[cfg(target_os = "macos")]
use crate::display_link::DisplayLinkScheduler;
use crate::pane_tree::RemoveResult;
use crate::tab::{PaneSplitDirection, Selection, SelectionMode, TerminalPane, TerminalTab};
use crate::tab_layout::TabLayout;

// ── user events (used to wake the event loop from background threads) ────────

#[derive(Debug, Clone)]
pub enum VoltEvent {
    /// PTY reader thread produced output — request a redraw.
    PtyData,
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
/// Maximum delay between clicks for double/triple-click detection.
const MULTI_CLICK_INTERVAL: Duration = Duration::from_millis(450);

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
    renderer: Renderer,
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
    mouse_pos: (f32, f32),
    is_drag_selecting: bool,
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
    chat_panel: ChatPanel,
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
}

impl MainState {
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
        let phys_right = self.renderer.surface_width() as f32 - phys_pad;
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

    fn new_tab(&mut self) {
        let (cols, rows) = self.current_grid_size();
        if let Ok(tab) = TerminalTab::spawn(
            &self.config,
            cols as u16,
            rows as u16,
            self.proxy.clone(),
            Arc::clone(&self.pty_wake_pending),
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
        let Ok(new_pane) =
            TerminalPane::spawn(&config, pane_cols as u16, pane_rows as u16, proxy, wake)
        else {
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
            dst.copy_from_grid(dst_row + row, dst_col, src, &src.row_cells(row)[..cols]);
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
            dst.copy_from_grid(
                dst_row + r,
                dst_col,
                src,
                &src.scrollback_row(sb_idx)[..cols],
            );
        }
        for r in 0..grid_rows.min(src.rows) {
            dst.copy_from_grid(
                dst_row + sb_rows + r,
                dst_col,
                src,
                &src.row_cells(r)[..cols],
            );
        }
    }

    fn build_render_grid_for_active_tab(&self) -> Option<(Grid, bool)> {
        let (total_cols, total_rows) = self.current_grid_size();
        let tab = self.active_tab();
        let active_id = tab.tree.active_id;
        let rects = tab.tree.layout(total_cols, total_rows);

        let mut out = Grid::new(total_cols.max(1), total_rows.max(1));
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

    fn apply_config(&mut self, config: &Config) {
        self.theme = Theme::by_name(&config.theme);
        self.config = config.clone();
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
        let Some((pane_id, col, row)) = self.pane_cell_from_mouse(mx, my) else {
            return;
        };
        if let Some(sel) = &mut self.selection {
            if sel.pane_id != pane_id {
                return;
            }
            sel.end_col = col;
            sel.end_row = row;
            self.begin_redraw();
        }
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
            mode: SelectionMode::Linear,
            start_col: start,
            start_row: row,
            end_col: end,
            end_row: row,
        })
    }

    /// Expand a triple-click into a whole-row selection.
    fn line_selection(&self, pane_id: usize, row: usize) -> Option<Selection> {
        let tab = self.tabs.get(self.active_tab)?;
        let pane = tab.tree.find_leaf(pane_id)?;
        let performer = pane.performer.lock().ok()?;
        let cols = performer.grid.cols;
        Some(Selection {
            pane_id,
            mode: SelectionMode::Linear,
            start_col: 0,
            start_row: row,
            end_col: cols.saturating_sub(1),
            end_row: row,
        })
    }

    fn selected_text(&self) -> Option<String> {
        let sel = self.selection?.normalized();
        let tab = self.tabs.get(self.active_tab)?;
        let pane = tab.tree.find_leaf(sel.pane_id)?;
        let performer = pane.performer.lock().ok()?;
        let grid = &performer.grid;
        if sel.start_row >= grid.rows || sel.end_row >= grid.rows {
            return None;
        }
        let offset = pane.scroll_view_offset.min(grid.scrollback_len());
        let mut out = String::new();
        for row in sel.start_row..=sel.end_row {
            let (start_col, end_col) = match sel.mode {
                SelectionMode::Block => (
                    sel.start_col.min(grid.cols.saturating_sub(1)),
                    sel.end_col.min(grid.cols.saturating_sub(1)),
                ),
                SelectionMode::Linear => {
                    let start_col = if row == sel.start_row {
                        sel.start_col
                    } else {
                        0
                    };
                    let end_col = if row == sel.end_row {
                        sel.end_col.min(grid.cols.saturating_sub(1))
                    } else {
                        grid.cols.saturating_sub(1)
                    };
                    (start_col, end_col)
                }
            };
            if start_col > end_col || start_col >= grid.cols {
                continue;
            }
            let mut line = String::new();
            for col in start_col..=end_col {
                if let Some(cell) = Self::displayed_cell(grid, offset, col, row) {
                    grid.push_cell_text(&mut line, cell);
                } else {
                    line.push(' ');
                }
            }
            if matches!(sel.mode, SelectionMode::Block) {
                out.push_str(&line);
            } else {
                out.push_str(line.trim_end_matches(' '));
            }
            if row != sel.end_row {
                out.push('\n');
            }
        }
        Some(out)
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

    fn paste_clipboard(&mut self) {
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("pbpaste").output() {
                Ok(output) => {
                    if output.status.success() {
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

    #[cfg(target_os = "macos")]
    fn open_find_prompt(&mut self) {
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
        self.search_dirty = false;
        self.search_target = None;
        let custom = match kind {
            crate::prompt::PromptKind::RenameTab => self.active_tab().custom_title.as_deref(),
            crate::prompt::PromptKind::RenameTerminal => {
                self.active_tab().active_pane().custom_title.as_deref()
            }
            crate::prompt::PromptKind::Find => None,
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
                prompt.matches.clear();
                prompt.matches_truncated = false;
                prompt.current_match = 0;
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
            prompt.matches = matches;
            prompt.matches_truncated = truncated;
            prompt.current_match = 0;
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
            self.invalidate_search_after_output();
        }
    }

    fn invalidate_search_after_output(&mut self) {
        if let Some(prompt) = self.active_prompt.as_mut() {
            if prompt.kind == crate::prompt::PromptKind::Find && !prompt.is_empty() {
                // Never display a stale highlight against a changed grid.
                prompt.matches.clear();
                prompt.matches_truncated = false;
                prompt.current_match = 0;
                self.search_dirty = true;
            }
        }
    }

    /// Scroll the active pane so the current search match is visible,
    /// placing its row at the top of the viewport when it's in scrollback.
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
        let sel = self.selection?.normalized();
        let (total_cols, total_rows) = self.current_grid_size();
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        let rect = rects.iter().find(|r| r.id == sel.pane_id)?;
        Some((
            (sel.start_col + rect.col, sel.start_row + rect.row),
            (sel.end_col + rect.col, sel.end_row + rect.row),
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
        if !self.shift_down() {
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
        let old_offset = pane.scroll_view_offset;
        if amount > 0.0 {
            // Scroll up: reveal older lines.
            pane.scroll_view_offset = (pane.scroll_view_offset + steps).min(scrollback_len);
        } else {
            // Scroll down: return toward live output.
            pane.scroll_view_offset = pane.scroll_view_offset.saturating_sub(steps);
        }
        let new_offset = pane.scroll_view_offset;
        if new_offset != old_offset && self.selection.is_some() {
            // The view shifted under the selection; its coordinates no longer
            // match what is displayed, so drop it.
            self.selection = None;
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
    #[cfg(target_os = "macos")]
    /// The window that last reported `Focused(true)` — menu-bar actions
    /// (which are process-global, not per-window) target this window.
    focused_window: Option<WindowId>,
    #[cfg(target_os = "macos")]
    /// Must stay alive for the app's full lifetime — see the doc comment on
    /// `menu::install_app_menu`. Dropping this frees the Rust-side data
    /// every native `NSMenuItem`'s action handler reads via a raw pointer,
    /// while AppKit's menu bar keeps running against those now-dangling
    /// pointers; the first click then reads freed memory and aborts.
    app_menu: Option<muda::Menu>,
}

impl App {
    pub fn new(config: Config, config_alert: Option<String>) -> anyhow::Result<Self> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        Ok(Self {
            config,
            config_alert,
            windows: HashMap::new(),
            proxy: None,
            pty_wake_pending: Arc::new(AtomicBool::new(false)),
            rt,
            #[cfg(target_os = "macos")]
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
        let first_tab = TerminalTab::spawn(
            &self.config,
            cols as u16,
            rows as u16,
            proxy.clone(),
            Arc::clone(&self.pty_wake_pending),
        )?;
        let theme = Theme::by_name(&self.config.theme);
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
            renderer,
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
            config: self.config.clone(),
            mouse_pos: (0.0, 0.0),
            is_drag_selecting: false,
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
            chat_panel: ChatPanel::new(),
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

    /// Reload `config.toml` and apply it to `window_id`'s state. Shared by
    /// the Cmd+Shift+R shortcut and the "Reload Settings" menu action.
    fn reload_config(&mut self, window_id: WindowId) {
        let (new_cfg, config_alert) = Config::load_with_diagnostics();
        self.config = new_cfg.clone();
        self.config_alert = config_alert.clone();
        if let Some(state) = self.windows.get_mut(&window_id) {
            state.renderer.set_top_alert(config_alert);
            state.apply_config(&new_cfg);
        }
    }

    #[cfg(target_os = "macos")]
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
            _ => {}
        }

        let Some(window_id) = self.target_window_id() else {
            return;
        };

        match action {
            A::ReloadSettings => {
                self.reload_config(window_id);
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
            A::Find => state.open_find_prompt(),
            A::ToggleChatPanel => {
                state.chat_panel.toggle();
                state.begin_redraw();
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
            A::NewWindow | A::OpenSettings | A::ReloadSettings | A::CloseWindow => {
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
                if let Some(action) = crate::menu::MenuAction::from_id(&id) {
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
            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
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
                state.left_shift_down = false;
                state.right_shift_down = false;
                state.left_control_down = false;
                state.right_control_down = false;
                state.left_super_down = false;
                state.right_super_down = false;
            }
            #[cfg(target_os = "macos")]
            WindowEvent::Focused(true) => {
                let native_tab_count = state.window.num_tabs().max(1);
                state.last_known_native_tab_count = native_tab_count;
                configure_macos_tab_chrome(state.window.as_ref(), native_tab_count);
                self.focused_window = Some(window_id);
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (position.x as f32, position.y as f32);

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
                if btn_state == ElementState::Pressed {
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

                if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                    state.active_tab_mut().tree.active_id = pane_id;
                    // Shift bypasses app mouse reporting so selection still works
                    // inside TUIs (standard xterm behaviour).
                    if !state.shift_down() && state.report_mouse_button(button, btn_state, col, row)
                    {
                        return;
                    }
                }

                if button == MouseButton::Left {
                    if btn_state == ElementState::Pressed {
                        if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                            state.active_tab_mut().tree.active_id = pane_id;
                            let now = Instant::now();
                            let same_spot = state.last_click.is_some_and(|(t, p, c, r)| {
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
                            state.selection = Some(expanded.unwrap_or(Selection {
                                pane_id,
                                mode: if state.modifiers.alt_key() {
                                    SelectionMode::Block
                                } else {
                                    SelectionMode::Linear
                                },
                                start_col: col,
                                start_row: row,
                                end_col: col,
                                end_row: row,
                            }));
                            // Word/line selections stay fixed; only single clicks
                            // start a drag so trackpad jitter can't collapse them.
                            state.is_drag_selecting = state.click_count == 1;
                            state.begin_redraw();
                        }
                    } else {
                        state.is_drag_selecting = false;
                    }
                }
            }

            WindowEvent::MouseWheel { delta, .. } => {
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

                if matches!(physical_key, PhysicalKey::Code(KeyCode::KeyR))
                    && shift
                    && reload_modifier
                {
                    let _ = state;
                    self.reload_config(window_id);
                    return;
                }

                // ── global shortcuts ─────────────────────────────────────────
                if super_key {
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
                            // Cmd+Shift+A: toggle AI Chat Panel
                            state.chat_panel.toggle();
                            state.begin_redraw();
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

                if ctrl && shift {
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

                if alt && ctrl {
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

                if ctrl && matches!(physical_key, PhysicalKey::Code(KeyCode::Tab)) {
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

    use super::{Grid, MainState};

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
