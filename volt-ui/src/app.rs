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

use volt_config::{config_path, sample_config_toml, Config, Theme};
use volt_core::events::CoreEvent;
use volt_core::grid::Grid;
use volt_core::performer::MouseTrackingMode;

use volt_renderer::{Renderer, TabEntry};

#[cfg(target_os = "macos")]
use crate::display_link::DisplayLinkScheduler;
use crate::tab::{PaneSplitDirection, Selection, TerminalPane, TerminalTab};
use crate::tab_layout::TabLayout;

// ── user events (used to wake the event loop from background threads) ────────

#[derive(Debug, Clone)]
pub enum VoltEvent {
    /// PTY reader thread produced output — request a redraw.
    PtyData,
    #[cfg(target_os = "macos")]
    /// Display link tick — redraw on display cadence when needed.
    DisplayLinkTick,
}

const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(530);

#[derive(Debug, Clone, Copy)]
struct DividerDrag {
    divider_id: usize,
    start_px: f32,
    is_vertical: bool,
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
}

impl MainState {
    fn layout_tab_count(&self) -> usize {
        self.tabs.len().max(1)
    }

    fn show_custom_tab_bar(&self) -> bool {
        self.layout_tab_count() > 1
    }

    fn current_tab_bar_height(&self) -> f32 {
        self.renderer.top_offset_for_tab_count(self.layout_tab_count())
    }

    fn current_content_top_offset(&self) -> f32 {
        self.renderer
            .content_top_offset_for_tab_count(self.layout_tab_count())
    }

    fn current_grid_size(&self) -> (usize, usize) {
        self.renderer.grid_size_for_tab_count(self.layout_tab_count())
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
                        Err(err) => eprintln!("volt-ui: failed to lock performer for resize: {err}"),
                    }
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
    fn sync_native_window_title(&self) {
        let title = self
            .tabs
            .get(self.active_tab)
            .map(|t| t.display_title(self.active_tab + 1))
            .unwrap_or_else(|| "~".to_string());
        self.window.set_title(&title);
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
        tab.tree.dividers_with_ids(
            total_cols, total_rows,
            self.renderer.cell_width, self.renderer.cell_height,
            phys_pad, content_top, scale,
        )
    }

    fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
            self.selection = None;
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
        let (total_cols, total_rows) = self.current_grid_size();
        let active_id = self.active_tab().tree.active_id;
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        let active_rect = rects.iter().find(|r| r.id == active_id).copied();
        let (pane_cols, pane_rows) = match (active_rect, direction) {
            (Some(r), PaneSplitDirection::Vertical) => ((r.cols / 2).max(1), r.rows),
            (Some(r), PaneSplitDirection::Horizontal) => (r.cols, (r.rows / 2).max(1)),
            (None, _) => (total_cols.max(1), total_rows.max(1)),
        };
        let config = self.config.clone();
        let proxy = self.proxy.clone();
        let wake = Arc::clone(&self.pty_wake_pending);
        let Ok(new_pane) = TerminalPane::spawn(&config, pane_cols as u16, pane_rows as u16, proxy, wake) else {
            return false;
        };
        self.active_tab_mut().tree.split(active_id, direction, new_pane);
        self.resize_all_tabs_to_current_grid();
        self.selection = None;
        true
    }

    fn pane_cell_from_global_cell(
        &self,
        col: usize,
        row: usize,
    ) -> Option<(usize, usize, usize)> {
        let (total_cols, total_rows) = self.current_grid_size();
        let rects = self.active_tab().tree.layout(total_cols, total_rows);
        for rect in &rects {
            if col >= rect.col && col < rect.col + rect.cols
                && row >= rect.row && row < rect.row + rect.rows
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
        for row in 0..rows {
            for col in 0..cols {
                *dst.cell_mut(dst_col + col, dst_row + row) = *src.cell(col, row);
            }
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
            let Some(pane) = tab.tree.find_leaf(rect.id) else { continue; };
            let (grid, cursor_visible) = {
                let performer = pane.performer.lock().ok()?;
                (performer.grid.clone(), performer.cursor_visible)
            };
            Self::blit_grid(&mut out, &grid, rect.col, rect.row);
            if rect.id == active_id {
                active_cursor_visible = cursor_visible;
                out.cursor_col = (rect.col + grid.cursor_col).min(rect.col + rect.cols.saturating_sub(1));
                out.cursor_row = (rect.row + grid.cursor_row).min(rect.row + rect.rows.saturating_sub(1));
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

    fn handle_click(&mut self, mx: f32, my: f32) {
        if !self.show_custom_tab_bar() {
            return;
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
        } else if tl.hit_plus(mx, my) {
            self.new_tab();
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

    fn selected_text(&self) -> Option<String> {
        let sel = self.selection?.normalized();
        if sel.pane_id != self.active_tab().tree.active_id {
            return None;
        }
        let performer = self
            .tabs
            .get(self.active_tab)?
            .active_pane()
            .performer
            .lock()
            .ok()?;
        let grid = &performer.grid;
        if sel.start_row >= grid.rows || sel.end_row >= grid.rows {
            return None;
        }
        let mut out = String::new();
        for row in sel.start_row..=sel.end_row {
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
            if start_col > end_col || start_col >= grid.cols {
                continue;
            }
            let mut line = String::new();
            for col in start_col..=end_col {
                line.push(grid.cell(col, row).c);
            }
            out.push_str(line.trim_end_matches(' '));
            if row != sel.end_row {
                out.push('\n');
            }
        }
        Some(out)
    }

    fn copy_selection(&self) {
        let Some(text) = self.selected_text() else {
            return;
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
    }

    fn paste_clipboard(&mut self) {
        #[cfg(target_os = "macos")]
        {
            match std::process::Command::new("pbpaste").output() {
                Ok(output) => {
                    if output.status.success() {
                        if let Err(err) = self.active_pane_mut().pty.write(&output.stdout) {
                            eprintln!("volt-ui: failed to write pasted clipboard to PTY: {err}");
                        }
                    } else {
                        eprintln!("volt-ui: pbpaste exited with {}", output.status);
                    }
                }
                Err(err) => eprintln!("volt-ui: failed to launch pbpaste: {err}"),
            }
        }
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
        let Some((_mode, sgr)) = self.active_mouse_reporting() else {
            return false;
        };
        let amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => y,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / self.renderer.cell_height.max(1.0),
        };
        if amount == 0.0 {
            return false;
        }
        let steps = amount.abs().ceil().max(1.0) as usize;
        let base = if amount > 0.0 { 64u8 } else { 65u8 };
        let cb = base.saturating_add(self.mouse_modifier_bits());
        for _ in 0..steps {
            self.write_mouse_report(cb, col, row, false, sgr);
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
            event_loop.set_allows_automatic_window_tabbing(false);
            window_attrs = window_attrs
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true)
                .with_title_hidden(true);
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

        let state = MainState {
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
        };
        state
            .window
            .set_transparent(self.config.appearance.transparent_enabled());
        state
            .window
            .set_blur(self.config.appearance.blur_enabled());
        #[cfg(target_os = "macos")]
        state.sync_native_window_title();
        let id = state.id;
        self.windows.insert(id, state);
        Ok(id)
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn configure_macos_tab_chrome(_window: &Window, _native_tab_count: usize) {
    // Keep macOS tab handling fully native for stability.
    // We only use winit's tabbing identifier / native APIs.
}

fn open_config_in_editor() {
    let Some(path) = config_path() else { return };

    if !path.exists() {
        // First run: write the fully-documented sample config
        if let Some(dir) = path.parent() {
            if let Err(err) = std::fs::create_dir_all(dir) {
                eprintln!("volt-ui: failed to create config directory: {err}");
                return;
            }
        }
        if let Err(err) = std::fs::write(&path, sample_config_toml()) {
            eprintln!("volt-ui: failed to write sample config: {err}");
            return;
        }
    } else if let Ok(existing) = std::fs::read_to_string(&path) {
        // Old config (created before documentation was added) — replace it
        if !existing.contains("# Volt Terminal") {
            if let Err(err) = std::fs::write(&path, sample_config_toml()) {
                eprintln!("volt-ui: failed to refresh sample config: {err}");
                return;
            }
        }
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

// ── ApplicationHandler ────────────────────────────────────────────────────────

impl ApplicationHandler<VoltEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.windows.is_empty() {
            return;
        }

        #[cfg(target_os = "macos")]
        set_app_icon();

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
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (position.x as f32, position.y as f32);

                if let Some(drag) = state.divider_drag {
                    let delta = if drag.is_vertical {
                        position.x as f32 - drag.start_px
                    } else {
                        position.y as f32 - drag.start_px
                    };
                    let (total_cols, total_rows) = state.current_grid_size();
                    let active = state.active_tab;
                    state.tabs[active].tree.adjust_ratio(
                        drag.divider_id, delta,
                        state.renderer.cell_width, state.renderer.cell_height,
                        total_cols, total_rows,
                    );
                    // reset start_px so delta is incremental on next move
                    state.divider_drag = Some(DividerDrag {
                        start_px: if drag.is_vertical { position.x as f32 } else { position.y as f32 },
                        ..drag
                    });
                    state.begin_redraw();
                } else {
                    // Update cursor icon near any divider
                    let dividers = state.active_tab_dividers();
                    let mut hovering = false;
                    for div in &dividers {
                        let is_vertical = matches!(div.direction, PaneSplitDirection::Vertical);
                        // For vertical dividers: check x proximity AND y in [div.y, div.y+div.height]
                        // For horizontal dividers: check y proximity AND x in [div.x, div.x+div.width]
                        let (axis_coord, axis_divider, span_coord, span_start, span_end) = if is_vertical {
                            (position.x as f32, div.phys.x, position.y as f32,
                             div.phys.y, div.phys.y + div.phys.height)
                        } else {
                            (position.y as f32, div.phys.y, position.x as f32,
                             div.phys.x, div.phys.x + div.phys.width)
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
                            hovering = true;
                            break;
                        }
                    }
                    if !hovering {
                        state.window.set_cursor(winit::window::CursorIcon::Default);
                    }

                    let cell = state.pane_cell_from_mouse(state.mouse_pos.0, state.mouse_pos.1);
                    if let Some((pane_id, col, row)) = cell {
                        if pane_id == state.active_tab().tree.active_id {
                            if state.last_reported_mouse_cell != Some((col, row)) {
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
                        state.handle_click(mx, my);
                        state.begin_redraw();
                    }
                    return;
                }

                // Divider drag: release
                if btn_state == ElementState::Released {
                    if state.divider_drag.take().is_some() {
                        state.resize_all_tabs_to_current_grid();
                        state.window.set_cursor(winit::window::CursorIcon::Default);
                        state.begin_redraw();
                        return;
                    }
                }

                // Divider drag: press
                if button == MouseButton::Left && btn_state == ElementState::Pressed {
                    let dividers = state.active_tab_dividers();
                    let mut started_drag = false;
                    for div in &dividers {
                        let is_vertical = matches!(div.direction, PaneSplitDirection::Vertical);
                        let (axis_coord, axis_div, span_coord, span_start, span_end) = if is_vertical {
                            (mx, div.phys.x, my, div.phys.y, div.phys.y + div.phys.height)
                        } else {
                            (my, div.phys.y, mx, div.phys.x, div.phys.x + div.phys.width)
                        };
                        if (axis_coord - axis_div).abs() < 6.0
                            && span_coord >= span_start - 4.0
                            && span_coord <= span_end + 4.0
                        {
                            state.divider_drag = Some(DividerDrag {
                                divider_id: div.id,
                                start_px: axis_coord,
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
                    if state.report_mouse_button(button, btn_state, col, row) {
                        return;
                    }
                }

                if button == MouseButton::Left {
                    if btn_state == ElementState::Pressed {
                        if let Some((pane_id, col, row)) = state.pane_cell_from_mouse(mx, my) {
                            state.active_tab_mut().tree.active_id = pane_id;
                            state.selection = Some(Selection {
                                pane_id,
                                start_col: col,
                                start_row: row,
                                end_col: col,
                                end_row: row,
                            });
                            state.is_drag_selecting = true;
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
                    let _ = state.report_mouse_wheel(delta, col, row);
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
                state.bump_cursor_blink();

                let ctrl = state.ctrl_down();
                let super_key = state.super_down();
                let shift = state.shift_down();
                let alt = state.modifiers.alt_key();

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
                    let (new_cfg, config_alert) = Config::load_with_diagnostics();
                    self.config = new_cfg.clone();
                    self.config_alert = config_alert.clone();
                    state.renderer.set_top_alert(config_alert);
                    state.apply_config(&new_cfg);
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
                            if state.split_active_tab(direction) {
                                state.begin_redraw();
                            }
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            state.new_tab();
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyW) => {
                            state.divider_drag = None;
                            state.selection = None;
                            state.last_reported_mouse_cell = None;
                            let active_id = state.active_tab().tree.active_id;
                            if !state.active_tab_mut().tree.remove(active_id) {
                                // last pane — close the whole tab
                                if state.tabs.len() > 1 {
                                    let i = state.active_tab;
                                    state.close_tab(i);
                                }
                            }
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            state.copy_selection();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyV) => {
                            state.paste_clipboard();
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            state.switch_tab(0);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            state.switch_tab(1);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            state.switch_tab(2);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit4) => {
                            state.switch_tab(3);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit5) => {
                            state.switch_tab(4);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit6) => {
                            state.switch_tab(5);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit7) => {
                            state.switch_tab(6);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit8) => {
                            state.switch_tab(7);
                            state.begin_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit9) => {
                            state.switch_tab(8);
                            state.begin_redraw();
                            return;
                        }
                        _ => {}
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
                    if let Err(err) = state.active_pane_mut().pty.write(&bytes) {
                        eprintln!("volt-ui: failed to write PTY input: {err}");
                    }
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
                            if let Err(err) = state.active_pane_mut().pty.write(&bytes) {
                                eprintln!("volt-ui: failed to write PTY control input: {err}");
                            }
                            return;
                        }
                    }
                }

                if let Some(text) = text {
                    if let Err(err) = state.active_pane_mut().pty.write(text.as_str().as_bytes()) {
                        eprintln!("volt-ui: failed to write PTY text input: {err}");
                    }
                    return;
                }

                if let Some(fallback_text) = logical_key.to_text() {
                    if let Err(err) = state.active_pane_mut().pty.write(fallback_text.as_bytes()) {
                        eprintln!("volt-ui: failed to write PTY fallback input: {err}");
                    }
                }
            }

            WindowEvent::RedrawRequested => {
                state.redraw_pending = false;
                let active = state.active_tab;
                let mut needs_full_redraw = false;
                let mut pending_partial_damage: Option<(usize, usize)> = None;
                // (tab_idx, pane_id)
                let mut closed_panes: Vec<(usize, usize)> = Vec::new();

                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    let leaf_ids = tab.tree.leaf_ids();
                    for pane_id in leaf_ids {
                        loop {
                            let Some(ev) = tab.tree.find_leaf_mut(pane_id)
                                .and_then(|p| p.event_rx.try_recv().ok()) else { break; };
                            match ev {
                                CoreEvent::GridUpdated { damaged_rows } => {
                                    if i == active {
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
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.cwd = Some(path);
                                    }
                                }
                                CoreEvent::TitleChanged(t) => {
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.title = t;
                                    }
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.running = false;
                                    }
                                }
                                CoreEvent::PtyError(msg) => {
                                    if i == active {
                                        state.renderer.set_top_alert(Some(msg));
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::PtyClosed => closed_panes.push((i, pane_id)),
                            }
                        }
                    }
                }

                // Handle closed panes
                if !closed_panes.is_empty() {
                    // Process from highest tab index to lowest to avoid index shifting
                    closed_panes.sort_unstable();
                    closed_panes.dedup();
                    for (tab_idx, pane_id) in closed_panes.into_iter().rev() {
                        if tab_idx >= state.tabs.len() { continue; }
                        let still_has_panes = state.tabs[tab_idx].tree.remove(pane_id);
                        if !still_has_panes {
                            if state.tabs.len() <= 1 {
                                close_window_after_event = true;
                                break;
                            }
                            state.close_tab(tab_idx);
                        }
                    }
                    if !close_window_after_event {
                        // Repair UI state after structural change
                        if state.selection.map_or(false, |s| {
                            state.active_tab().tree.find_leaf(s.pane_id).is_none()
                        }) {
                            state.selection = None;
                        }
                        state.last_reported_mouse_cell = None;
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
                        state.window.set_title(active_title);
                    }

                    let Some((render_grid, render_cursor_visible)) =
                        state.build_render_grid_for_active_tab()
                    else {
                        eprintln!("volt-ui: failed to snapshot panes for render");
                        return;
                    };
                    let damage_rows = state.take_render_damage_rows();
                    let dividers: Vec<volt_renderer::PaneDivider> =
                        state.active_tab_dividers().into_iter().map(|d| d.phys).collect();
                    state.renderer.render_frame(
                        &render_grid,
                        &state.theme,
                        &tab_entries,
                        state.effective_cursor_visible(render_cursor_visible),
                        state.selection_tuple(),
                        damage_rows,
                        &dividers,
                    );
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
            let mut closed_panes: Vec<(usize, usize)> = Vec::new();

            if !state.redraw_pending {
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    let leaf_ids = tab.tree.leaf_ids();
                    for pane_id in leaf_ids {
                        loop {
                            let Some(ev) = tab.tree.find_leaf_mut(pane_id)
                                .and_then(|p| p.event_rx.try_recv().ok()) else { break; };
                            match ev {
                                CoreEvent::GridUpdated { damaged_rows } => {
                                    if i == active {
                                        needs_redraw = true;
                                        if let Some((start, end)) = damaged_rows {
                                            pending_partial_damage = Some(match pending_partial_damage {
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
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.cwd = Some(path);
                                    }
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::TitleChanged(t) => {
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.title = t;
                                    }
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    if let Some(p) = tab.tree.find_leaf_mut(pane_id) {
                                        p.running = false;
                                    }
                                }
                                CoreEvent::PtyError(msg) => {
                                    if i == active {
                                        state.renderer.set_top_alert(Some(msg));
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::PtyClosed => closed_panes.push((i, pane_id)),
                            }
                        }
                    }
                }
            }

            if !closed_panes.is_empty() {
                closed_panes.sort_unstable();
                closed_panes.dedup();
                for (tab_idx, pane_id) in closed_panes.into_iter().rev() {
                    if tab_idx >= state.tabs.len() { continue; }
                    let still_has_panes = state.tabs[tab_idx].tree.remove(pane_id);
                    if !still_has_panes {
                        if state.tabs.len() <= 1 {
                            windows_to_close.push(*window_id);
                            break;
                        }
                        state.close_tab(tab_idx);
                    }
                }
                // Repair UI state after structural change
                if state.selection.map_or(false, |s| {
                    state.active_tab().tree.find_leaf(s.pane_id).is_none()
                }) {
                    state.selection = None;
                }
                state.last_reported_mouse_cell = None;
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
            struct NSSize { width: f64, height: f64 }
            let sz = NSSize { width: 256.0, height: 256.0 };
            let _: () = msg_send![image, setSize: sz];
            let app: *mut Object =
                msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setApplicationIconImage: image];
            let _: () = msg_send![image, release];
        }
    }
}
