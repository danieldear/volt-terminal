use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
#[cfg(target_os = "macos")]
use std::{ffi::c_void, ptr::null_mut};

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
use volt_core::performer::{MouseTrackingMode, Performer};
use volt_core::pty::Pty;
use volt_renderer::{Renderer, TabEntry};

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

#[cfg(target_os = "macos")]
type CVDisplayLinkRef = *mut c_void;
#[cfg(target_os = "macos")]
type CVOptionFlags = u64;
#[cfg(target_os = "macos")]
type CVReturn = i32;
#[cfg(target_os = "macos")]
type CVTimeStamp = c_void;
#[cfg(target_os = "macos")]
type CVDisplayLinkOutputCallback = Option<
    extern "C" fn(
        CVDisplayLinkRef,
        *const CVTimeStamp,
        *const CVTimeStamp,
        CVOptionFlags,
        *mut CVOptionFlags,
        *mut c_void,
    ) -> CVReturn,
>;

#[cfg(target_os = "macos")]
#[link(name = "CoreVideo", kind = "framework")]
unsafe extern "C" {
    fn CVDisplayLinkCreateWithActiveCGDisplays(displayLinkOut: *mut CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkSetOutputCallback(
        displayLink: CVDisplayLinkRef,
        callback: CVDisplayLinkOutputCallback,
        userInfo: *mut c_void,
    ) -> CVReturn;
    fn CVDisplayLinkStart(displayLink: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkStop(displayLink: CVDisplayLinkRef) -> CVReturn;
    fn CVDisplayLinkRelease(displayLink: CVDisplayLinkRef);
}

#[cfg(target_os = "macos")]
struct DisplayLinkContext {
    pending: Arc<AtomicBool>,
    proxy: EventLoopProxy<VoltEvent>,
}

#[cfg(target_os = "macos")]
struct DisplayLinkScheduler {
    display_link: CVDisplayLinkRef,
    pending: Arc<AtomicBool>,
    context: *mut DisplayLinkContext,
}

#[cfg(target_os = "macos")]
extern "C" fn display_link_output_callback(
    _display_link: CVDisplayLinkRef,
    _in_now: *const CVTimeStamp,
    _in_output_time: *const CVTimeStamp,
    _flags_in: CVOptionFlags,
    _flags_out: *mut CVOptionFlags,
    user_info: *mut c_void,
) -> CVReturn {
    if user_info.is_null() {
        return 0;
    }
    let ctx = unsafe { &*(user_info as *mut DisplayLinkContext) };
    if ctx.pending.swap(false, Ordering::AcqRel) {
        if let Err(err) = ctx.proxy.send_event(VoltEvent::DisplayLinkTick) {
            eprintln!("volt-ui: failed to send DisplayLinkTick event: {err}");
        }
    }
    0
}

#[cfg(target_os = "macos")]
impl DisplayLinkScheduler {
    fn new(proxy: EventLoopProxy<VoltEvent>) -> anyhow::Result<Self> {
        let mut display_link: CVDisplayLinkRef = null_mut();
        let create_result = unsafe { CVDisplayLinkCreateWithActiveCGDisplays(&mut display_link) };
        if create_result != 0 || display_link.is_null() {
            return Err(anyhow::anyhow!(
                "CVDisplayLinkCreateWithActiveCGDisplays failed with code {create_result}"
            ));
        }

        let pending = Arc::new(AtomicBool::new(false));
        let context = Box::new(DisplayLinkContext {
            pending: Arc::clone(&pending),
            proxy,
        });
        let context_ptr = Box::into_raw(context);

        let callback_result = unsafe {
            CVDisplayLinkSetOutputCallback(
                display_link,
                Some(display_link_output_callback),
                context_ptr as *mut c_void,
            )
        };
        if callback_result != 0 {
            unsafe {
                CVDisplayLinkRelease(display_link);
                drop(Box::from_raw(context_ptr));
            }
            return Err(anyhow::anyhow!(
                "CVDisplayLinkSetOutputCallback failed with code {callback_result}"
            ));
        }

        let start_result = unsafe { CVDisplayLinkStart(display_link) };
        if start_result != 0 {
            unsafe {
                CVDisplayLinkRelease(display_link);
                drop(Box::from_raw(context_ptr));
            }
            return Err(anyhow::anyhow!(
                "CVDisplayLinkStart failed with code {start_result}"
            ));
        }

        Ok(Self {
            display_link,
            pending,
            context: context_ptr,
        })
    }

    fn request_redraw(&self) {
        self.pending.store(true, Ordering::Release);
    }
}

#[cfg(target_os = "macos")]
impl Drop for DisplayLinkScheduler {
    fn drop(&mut self) {
        unsafe {
            if !self.display_link.is_null() {
                let _ = CVDisplayLinkStop(self.display_link);
                CVDisplayLinkRelease(self.display_link);
            }
            if !self.context.is_null() {
                drop(Box::from_raw(self.context));
            }
        }
    }
}

// ── per-tab state ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneSlot {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneSplitDirection {
    Vertical,
    Horizontal,
}

struct TerminalPane {
    pty: Pty,
    performer: Arc<Mutex<Performer>>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    title: String,
    cwd: Option<PathBuf>,
    running: bool,
}

struct TerminalTab {
    primary: TerminalPane,
    secondary: Option<TerminalPane>,
    split_direction: Option<PaneSplitDirection>,
    active_pane: PaneSlot,
}

#[derive(Debug, Clone, Copy)]
struct Selection {
    pane: PaneSlot,
    start_col: usize,
    start_row: usize,
    end_col: usize,
    end_row: usize,
}

impl Selection {
    fn normalized(self) -> Self {
        if (self.start_row, self.start_col) <= (self.end_row, self.end_col) {
            self
        } else {
            Self {
                pane: self.pane,
                start_col: self.end_col,
                start_row: self.end_row,
                end_col: self.start_col,
                end_row: self.start_row,
            }
        }
    }
}

impl TerminalPane {
    fn spawn(
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

    fn display_title(&self) -> String {
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
    fn spawn(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            primary: TerminalPane::spawn(config, cols, rows, proxy, wake_pending)?,
            secondary: None,
            split_direction: None,
            active_pane: PaneSlot::Primary,
        })
    }

    fn pane_count(&self) -> usize {
        if self.secondary.is_some() {
            2
        } else {
            1
        }
    }

    fn is_busy(&self) -> bool {
        self.primary.running || self.secondary.as_ref().is_some_and(|pane| pane.running)
    }

    fn active_pane(&self) -> &TerminalPane {
        if self.active_pane == PaneSlot::Secondary {
            if let Some(secondary) = self.secondary.as_ref() {
                return secondary;
            }
        }
        &self.primary
    }

    fn active_pane_mut(&mut self) -> &mut TerminalPane {
        if self.active_pane == PaneSlot::Secondary {
            if let Some(secondary) = self.secondary.as_mut() {
                return secondary;
            }
        }
        &mut self.primary
    }

    fn set_active_pane(&mut self, slot: PaneSlot) {
        if slot == PaneSlot::Secondary && self.secondary.is_none() {
            self.active_pane = PaneSlot::Primary;
            return;
        }
        self.active_pane = slot;
    }

    fn split(
        &mut self,
        direction: PaneSplitDirection,
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
        wake_pending: Arc<AtomicBool>,
    ) -> bool {
        if self.secondary.is_none() {
            let Ok(pane) = TerminalPane::spawn(config, cols, rows, proxy, wake_pending) else {
                return false;
            };
            self.secondary = Some(pane);
        }
        let changed_direction = self.split_direction != Some(direction);
        self.split_direction = Some(direction);
        self.active_pane = PaneSlot::Secondary;
        changed_direction || self.secondary.is_some()
    }

    fn close_secondary(&mut self) {
        self.secondary = None;
        self.split_direction = None;
        if self.active_pane == PaneSlot::Secondary {
            self.active_pane = PaneSlot::Primary;
        }
    }

    fn promote_secondary_to_primary(&mut self) {
        if let Some(secondary) = self.secondary.take() {
            self.primary = secondary;
            self.split_direction = None;
            self.active_pane = PaneSlot::Primary;
        }
    }

    fn display_title(&self, index: usize) -> String {
        let _ = index;
        self.active_pane().display_title()
    }
}

// ── tab bar hit testing ──────────────────────────────────────────────────────

struct TabLayout {
    sc: f32,
    left_pad: f32,
    tab_w: f32,
    tab_gap: f32,
    tab_y: f32,
    tab_h: f32,
    close_w: f32,
    plus_x: f32,
    plus_w: f32,
}

impl TabLayout {
    fn compute(sw: f32, tab_bar_h: f32, n_tabs: usize, sc: f32) -> Self {
        let left_pad = (78.0 * sc).round();
        let tab_gap = (4.0 * sc).round().max(2.0);
        let plus_w = (24.0 * sc).round().max(20.0);
        let right_pad = (10.0 * sc).round();
        let plus_x = (sw - right_pad - plus_w).max(left_pad + plus_w);
        let tab_area_w = (plus_x - left_pad - 10.0 * sc).max(80.0 * sc);
        let n = n_tabs.max(1);
        let tab_w = ((tab_area_w - tab_gap * (n.saturating_sub(1)) as f32) / n as f32)
            .min(220.0 * sc)
            .max(100.0 * sc);
        let tab_h = (tab_bar_h - 8.0 * sc).max(24.0 * sc);
        let tab_y = ((tab_bar_h - tab_h) * 0.5).round().max(2.0 * sc);
        TabLayout {
            sc,
            left_pad,
            tab_w,
            tab_gap,
            tab_y,
            tab_h,
            close_w: 16.0 * sc,
            plus_x,
            plus_w,
        }
    }

    fn tab_x(&self, i: usize) -> f32 {
        self.left_pad + i as f32 * (self.tab_w + self.tab_gap)
    }

    fn close_rect(&self, i: usize) -> (f32, f32, f32, f32) {
        let close_box_w = self.close_w + 6.0 * self.sc;
        let close_box_x = self.tab_x(i) + self.tab_w - close_box_w - 5.0 * self.sc;
        let close_box_y = self.tab_y + (1.0 * self.sc).max(1.0);
        let close_box_h = (self.tab_h - 2.0 * self.sc).max(18.0 * self.sc);
        (close_box_x, close_box_y, close_box_w, close_box_h)
    }

    fn plus_rect(&self) -> (f32, f32, f32, f32) {
        let plus_box_x = self.plus_x - 2.0 * self.sc;
        let plus_box_y = self.tab_y + (1.0 * self.sc).max(1.0);
        let plus_box_w = self.plus_w + 4.0 * self.sc;
        let plus_box_h = (self.tab_h - 2.0 * self.sc).max(18.0 * self.sc);
        (plus_box_x, plus_box_y, plus_box_w, plus_box_h)
    }

    fn hit_tab(&self, mx: f32, my: f32, n: usize) -> Option<usize> {
        if my < self.tab_y || my >= self.tab_y + self.tab_h {
            return None;
        }
        for i in 0..n {
            let tx = self.tab_x(i);
            if mx >= tx && mx < tx + self.tab_w {
                return Some(i);
            }
        }
        None
    }
    fn hit_close(&self, mx: f32, my: f32, i: usize) -> bool {
        let (x, y, w, h) = self.close_rect(i);
        mx >= x && mx < x + w && my >= y && my < y + h
    }

    fn hit_plus(&self, mx: f32, my: f32) -> bool {
        let (x, y, w, h) = self.plus_rect();
        mx >= x && mx < x + w && my >= y && my < y + h
    }
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

    fn pane_grid_sizes_for_tab(
        tab: &TerminalTab,
        total_cols: usize,
        total_rows: usize,
    ) -> ((usize, usize), Option<(usize, usize)>) {
        if tab.secondary.is_none() || tab.split_direction.is_none() {
            return ((total_cols.max(1), total_rows.max(1)), None);
        }
        let divider = 1usize;
        match tab.split_direction.unwrap_or(PaneSplitDirection::Vertical) {
            PaneSplitDirection::Vertical => {
                let usable_cols = total_cols.saturating_sub(divider).max(2);
                let left_cols = (usable_cols / 2).max(1);
                let right_cols = (usable_cols - left_cols).max(1);
                (
                    (left_cols, total_rows.max(1)),
                    Some((right_cols, total_rows.max(1))),
                )
            }
            PaneSplitDirection::Horizontal => {
                let usable_rows = total_rows.saturating_sub(divider).max(2);
                let top_rows = (usable_rows / 2).max(1);
                let bottom_rows = (usable_rows - top_rows).max(1);
                (
                    (total_cols.max(1), top_rows),
                    Some((total_cols.max(1), bottom_rows)),
                )
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
            let (total_cols, total_rows) = self.current_grid_size();
            for tab in &mut self.tabs {
                let ((primary_cols, primary_rows), secondary_size) =
                    Self::pane_grid_sizes_for_tab(tab, total_cols, total_rows);
                if let Err(err) = tab
                    .primary
                    .pty
                    .resize(primary_cols as u16, primary_rows as u16)
                {
                    eprintln!("volt-ui: failed to resize primary PTY after new tab: {err}");
                }
                match tab.primary.performer.lock() {
                    Ok(mut performer) => performer.resize(primary_cols, primary_rows),
                    Err(err) => {
                        eprintln!("volt-ui: failed to lock primary performer after new tab: {err}")
                    }
                }
                if let (Some(secondary), Some((secondary_cols, secondary_rows))) =
                    (tab.secondary.as_mut(), secondary_size)
                {
                    if let Err(err) = secondary
                        .pty
                        .resize(secondary_cols as u16, secondary_rows as u16)
                    {
                        eprintln!("volt-ui: failed to resize secondary PTY after new tab: {err}");
                    }
                    match secondary.performer.lock() {
                        Ok(mut performer) => performer.resize(secondary_cols, secondary_rows),
                        Err(err) => eprintln!(
                            "volt-ui: failed to lock secondary performer after new tab: {err}"
                        ),
                    }
                }
            }
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
        let (total_cols, total_rows) = self.current_grid_size();
        for tab in &mut self.tabs {
            let ((primary_cols, primary_rows), secondary_size) =
                Self::pane_grid_sizes_for_tab(tab, total_cols, total_rows);
            if let Err(err) = tab
                .primary
                .pty
                .resize(primary_cols as u16, primary_rows as u16)
            {
                eprintln!("volt-ui: failed to resize primary PTY after tab close: {err}");
            }
            match tab.primary.performer.lock() {
                Ok(mut performer) => performer.resize(primary_cols, primary_rows),
                Err(err) => {
                    eprintln!("volt-ui: failed to lock primary performer after tab close: {err}")
                }
            }
            if let (Some(secondary), Some((secondary_cols, secondary_rows))) =
                (tab.secondary.as_mut(), secondary_size)
            {
                if let Err(err) = secondary
                    .pty
                    .resize(secondary_cols as u16, secondary_rows as u16)
                {
                    eprintln!("volt-ui: failed to resize secondary PTY after tab close: {err}");
                }
                match secondary.performer.lock() {
                    Ok(mut performer) => performer.resize(secondary_cols, secondary_rows),
                    Err(err) => eprintln!(
                        "volt-ui: failed to lock secondary performer after tab close: {err}"
                    ),
                }
            }
        }
    }

    fn resize_all_tabs_to_current_grid(&mut self) {
        let (total_cols, total_rows) = self.current_grid_size();
        for tab in &mut self.tabs {
            let ((primary_cols, primary_rows), secondary_size) =
                Self::pane_grid_sizes_for_tab(tab, total_cols, total_rows);
            if let Err(err) = tab
                .primary
                .pty
                .resize(primary_cols as u16, primary_rows as u16)
            {
                eprintln!("volt-ui: failed to resize primary PTY: {err}");
            }
            match tab.primary.performer.lock() {
                Ok(mut performer) => performer.resize(primary_cols, primary_rows),
                Err(err) => {
                    eprintln!("volt-ui: failed to lock primary performer for resize: {err}")
                }
            }
            if let (Some(secondary), Some((secondary_cols, secondary_rows))) =
                (tab.secondary.as_mut(), secondary_size)
            {
                if let Err(err) = secondary
                    .pty
                    .resize(secondary_cols as u16, secondary_rows as u16)
                {
                    eprintln!("volt-ui: failed to resize secondary PTY: {err}");
                }
                match secondary.performer.lock() {
                    Ok(mut performer) => performer.resize(secondary_cols, secondary_rows),
                    Err(err) => {
                        eprintln!("volt-ui: failed to lock secondary performer for resize: {err}")
                    }
                }
            }
        }
    }

    fn split_active_tab(&mut self, direction: PaneSplitDirection) -> bool {
        let (total_cols, total_rows) = self.current_grid_size();
        let (primary_size, secondary_size) = {
            let tab = self.active_tab();
            Self::pane_grid_sizes_for_tab(tab, total_cols, total_rows)
        };
        let (secondary_cols, secondary_rows) = secondary_size.unwrap_or(primary_size);
        let spawned = {
            let config = self.config.clone();
            let proxy = self.proxy.clone();
            let wake_pending = Arc::clone(&self.pty_wake_pending);
            self.active_tab_mut().split(
                direction,
                &config,
                secondary_cols as u16,
                secondary_rows as u16,
                proxy,
                wake_pending,
            )
        };
        if !spawned {
            return false;
        }
        self.resize_all_tabs_to_current_grid();
        self.selection = None;
        true
    }

    fn pane_layout_for_slot(
        tab: &TerminalTab,
        total_cols: usize,
        total_rows: usize,
        slot: PaneSlot,
    ) -> Option<(usize, usize, usize, usize)> {
        let ((primary_cols, primary_rows), secondary_size) =
            Self::pane_grid_sizes_for_tab(tab, total_cols, total_rows);
        match slot {
            PaneSlot::Primary => Some((0, 0, primary_cols, primary_rows)),
            PaneSlot::Secondary => {
                let (secondary_cols, secondary_rows) = secondary_size?;
                let divider = 1usize;
                match tab.split_direction.unwrap_or(PaneSplitDirection::Vertical) {
                    PaneSplitDirection::Vertical => {
                        Some((primary_cols + divider, 0, secondary_cols, secondary_rows))
                    }
                    PaneSplitDirection::Horizontal => {
                        Some((0, primary_rows + divider, secondary_cols, secondary_rows))
                    }
                }
            }
        }
    }

    fn pane_cell_from_global_cell(
        &self,
        col: usize,
        row: usize,
    ) -> Option<(PaneSlot, usize, usize)> {
        let (total_cols, total_rows) = self.current_grid_size();
        let tab = self.active_tab();
        let primary = Self::pane_layout_for_slot(tab, total_cols, total_rows, PaneSlot::Primary)?;
        if col >= primary.0
            && col < primary.0 + primary.2
            && row >= primary.1
            && row < primary.1 + primary.3
        {
            return Some((PaneSlot::Primary, col - primary.0, row - primary.1));
        }
        let secondary =
            Self::pane_layout_for_slot(tab, total_cols, total_rows, PaneSlot::Secondary)?;
        if col >= secondary.0
            && col < secondary.0 + secondary.2
            && row >= secondary.1
            && row < secondary.1 + secondary.3
        {
            return Some((PaneSlot::Secondary, col - secondary.0, row - secondary.1));
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

        let (primary_grid, primary_cursor_visible) = {
            let performer = tab.primary.performer.lock().ok()?;
            (performer.grid.clone(), performer.cursor_visible)
        };

        let secondary_snapshot = if let Some(secondary) = tab.secondary.as_ref() {
            let performer = secondary.performer.lock().ok()?;
            Some((performer.grid.clone(), performer.cursor_visible))
        } else {
            None
        };

        let mut out = Grid::new(total_cols.max(1), total_rows.max(1));
        let primary_layout =
            Self::pane_layout_for_slot(tab, total_cols, total_rows, PaneSlot::Primary)?;
        Self::blit_grid(&mut out, &primary_grid, primary_layout.0, primary_layout.1);

        if let (Some((secondary_grid, _)), Some(secondary_layout)) = (
            secondary_snapshot.as_ref(),
            Self::pane_layout_for_slot(tab, total_cols, total_rows, PaneSlot::Secondary),
        ) {
            Self::blit_grid(
                &mut out,
                secondary_grid,
                secondary_layout.0,
                secondary_layout.1,
            );
        }

        if tab.secondary.is_some() {
            let separator_char = if tab.active_pane == PaneSlot::Secondary {
                '\u{2503}'
            } else {
                '\u{2502}'
            };
            match tab.split_direction.unwrap_or(PaneSplitDirection::Vertical) {
                PaneSplitDirection::Vertical => {
                    let separator_col = primary_layout.2.min(out.cols.saturating_sub(1));
                    for row in 0..out.rows {
                        let cell = out.cell_mut(separator_col, row);
                        cell.c = separator_char;
                    }
                }
                PaneSplitDirection::Horizontal => {
                    let separator_row = primary_layout.3.min(out.rows.saturating_sub(1));
                    let horiz = if tab.active_pane == PaneSlot::Secondary {
                        '\u{2501}'
                    } else {
                        '\u{2500}'
                    };
                    for col in 0..out.cols {
                        let cell = out.cell_mut(col, separator_row);
                        cell.c = horiz;
                    }
                }
            }
        }

        let (active_grid, active_cursor_visible) = if tab.active_pane == PaneSlot::Secondary {
            if let Some((grid, cursor_visible)) = secondary_snapshot {
                (grid, cursor_visible)
            } else {
                (primary_grid, primary_cursor_visible)
            }
        } else {
            (primary_grid, primary_cursor_visible)
        };

        if let Some((offset_col, offset_row, pane_cols, pane_rows)) =
            Self::pane_layout_for_slot(tab, total_cols, total_rows, tab.active_pane)
        {
            out.cursor_col =
                (offset_col + active_grid.cursor_col).min(offset_col + pane_cols.saturating_sub(1));
            out.cursor_row =
                (offset_row + active_grid.cursor_row).min(offset_row + pane_rows.saturating_sub(1));
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

    fn pane_cell_from_mouse(&self, mx: f32, my: f32) -> Option<(PaneSlot, usize, usize)> {
        let (col, row) = self.global_cell_from_mouse(mx, my)?;
        self.pane_cell_from_global_cell(col, row)
    }

    fn update_selection_end(&mut self, mx: f32, my: f32) {
        let Some((pane, col, row)) = self.pane_cell_from_mouse(mx, my) else {
            return;
        };
        if let Some(sel) = &mut self.selection {
            if sel.pane != pane {
                return;
            }
            sel.end_col = col;
            sel.end_row = row;
            self.begin_redraw();
        }
    }

    fn selected_text(&self) -> Option<String> {
        let sel = self.selection?.normalized();
        if sel.pane != self.active_tab().active_pane {
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
        let (offset_col, offset_row, _, _) =
            Self::pane_layout_for_slot(self.active_tab(), total_cols, total_rows, sel.pane)?;
        Some((
            (sel.start_col + offset_col, sel.start_row + offset_row),
            (sel.end_col + offset_col, sel.end_row + offset_row),
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
                let cell = state.pane_cell_from_mouse(state.mouse_pos.0, state.mouse_pos.1);
                if let Some((pane, col, row)) = cell {
                    if pane == state.active_tab().active_pane {
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

                if let Some((pane, col, row)) = state.pane_cell_from_mouse(mx, my) {
                    state.active_tab_mut().set_active_pane(pane);
                    if state.report_mouse_button(button, btn_state, col, row) {
                        return;
                    }
                }

                if button == MouseButton::Left {
                    if btn_state == ElementState::Pressed {
                        if let Some((pane, col, row)) = state.pane_cell_from_mouse(mx, my) {
                            state.active_tab_mut().set_active_pane(pane);
                            state.selection = Some(Selection {
                                pane,
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

                if let Some((pane, col, row)) =
                    state.pane_cell_from_mouse(state.mouse_pos.0, state.mouse_pos.1)
                {
                    state.active_tab_mut().set_active_pane(pane);
                    let _ = state.report_mouse_wheel(delta, col, row);
                }
            }

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key,
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
                            if state.tabs.len() > 1 {
                                let i = state.active_tab;
                                state.close_tab(i);
                                state.begin_redraw();
                            }
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
                let special: Option<&[u8]> = match physical_key {
                    PhysicalKey::Code(KeyCode::Enter) => Some(b"\r"),
                    PhysicalKey::Code(KeyCode::Backspace) => Some(b"\x7f"),
                    PhysicalKey::Code(KeyCode::Tab) => Some(b"\t"),
                    PhysicalKey::Code(KeyCode::Escape) => Some(b"\x1b"),
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        if state.active_application_cursor_keys_mode() {
                            Some(b"\x1bOA")
                        } else {
                            Some(b"\x1b[A")
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                        if state.active_application_cursor_keys_mode() {
                            Some(b"\x1bOB")
                        } else {
                            Some(b"\x1b[B")
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowRight) => {
                        if state.active_application_cursor_keys_mode() {
                            Some(b"\x1bOC")
                        } else {
                            Some(b"\x1b[C")
                        }
                    }
                    PhysicalKey::Code(KeyCode::ArrowLeft) => {
                        if state.active_application_cursor_keys_mode() {
                            Some(b"\x1bOD")
                        } else {
                            Some(b"\x1b[D")
                        }
                    }
                    PhysicalKey::Code(KeyCode::Home) => Some(b"\x1b[H"),
                    PhysicalKey::Code(KeyCode::End) => Some(b"\x1b[F"),
                    PhysicalKey::Code(KeyCode::PageUp) => Some(b"\x1b[5~"),
                    PhysicalKey::Code(KeyCode::PageDown) => Some(b"\x1b[6~"),
                    PhysicalKey::Code(KeyCode::Delete) => Some(b"\x1b[3~"),
                    _ => None,
                };

                if let Some(bytes) = special {
                    if matches!(physical_key, PhysicalKey::Code(KeyCode::Enter)) {
                        state.active_pane_mut().running = true;
                    }
                    if let Err(err) = state.active_pane_mut().pty.write(bytes) {
                        eprintln!("volt-ui: failed to write PTY input: {err}");
                    }
                    return;
                }

                if ctrl {
                    if let PhysicalKey::Code(code) = physical_key {
                        if let Some(b) = ctrl_code(code) {
                            if let Err(err) = state.active_pane_mut().pty.write(&[b]) {
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
                }
            }

            WindowEvent::RedrawRequested => {
                state.redraw_pending = false;
                let active = state.active_tab;
                let mut needs_full_redraw = false;
                let mut pending_partial_damage: Option<(usize, usize)> = None;
                let mut closed_primary = Vec::new();
                let mut closed_secondary = Vec::new();
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    while let Ok(ev) = tab.primary.event_rx.try_recv() {
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
                                tab.primary.cwd = Some(path);
                            }
                            CoreEvent::TitleChanged(t) => {
                                tab.primary.title = t;
                            }
                            CoreEvent::CommandFinished { .. } => {
                                tab.primary.running = false;
                            }
                            CoreEvent::PtyClosed => closed_primary.push(i),
                        }
                    }
                    if let Some(secondary) = tab.secondary.as_mut() {
                        while let Ok(ev) = secondary.event_rx.try_recv() {
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
                                    secondary.cwd = Some(path);
                                }
                                CoreEvent::TitleChanged(t) => {
                                    secondary.title = t;
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    secondary.running = false;
                                }
                                CoreEvent::PtyClosed => closed_secondary.push(i),
                            }
                        }
                    }
                }
                if !closed_secondary.is_empty() || !closed_primary.is_empty() {
                    closed_secondary.sort_unstable();
                    closed_secondary.dedup();
                    closed_primary.sort_unstable();
                    closed_primary.dedup();

                    for idx in closed_secondary.into_iter().rev() {
                        if closed_primary.binary_search(&idx).is_ok() {
                            continue;
                        }
                        if let Some(tab) = state.tabs.get_mut(idx) {
                            tab.close_secondary();
                        }
                    }

                    for idx in closed_primary.into_iter().rev() {
                        if idx >= state.tabs.len() {
                            continue;
                        }
                        let close_tab = {
                            let tab = &mut state.tabs[idx];
                            if tab.secondary.is_some() {
                                tab.promote_secondary_to_primary();
                                false
                            } else {
                                true
                            }
                        };
                        if !close_tab {
                            continue;
                        }
                        if state.tabs.len() <= 1 {
                            close_window_after_event = true;
                            break;
                        }
                        state.close_tab(idx);
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
                        state.window.set_title(active_title);
                    }

                    let Some((render_grid, render_cursor_visible)) =
                        state.build_render_grid_for_active_tab()
                    else {
                        eprintln!("volt-ui: failed to snapshot panes for render");
                        return;
                    };
                    let damage_rows = state.take_render_damage_rows();
                    state.renderer.render_frame(
                        &render_grid,
                        &state.theme,
                        &tab_entries,
                        state.effective_cursor_visible(render_cursor_visible),
                        state.selection_tuple(),
                        damage_rows,
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
            let mut closed_primary = Vec::new();
            let mut closed_secondary = Vec::new();

            if !state.redraw_pending {
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    while let Ok(ev) = tab.primary.event_rx.try_recv() {
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
                                tab.primary.cwd = Some(path);
                                if i == active || show_tab_chrome {
                                    needs_redraw = true;
                                    needs_full_redraw = true;
                                }
                            }
                            CoreEvent::TitleChanged(t) => {
                                tab.primary.title = t;
                                if i == active || show_tab_chrome {
                                    needs_redraw = true;
                                    needs_full_redraw = true;
                                }
                            }
                            CoreEvent::CommandFinished { .. } => {
                                tab.primary.running = false;
                            }
                            CoreEvent::PtyClosed => closed_primary.push(i),
                        }
                    }
                    if let Some(secondary) = tab.secondary.as_mut() {
                        while let Ok(ev) = secondary.event_rx.try_recv() {
                            match ev {
                                CoreEvent::GridUpdated { damaged_rows } => {
                                    if i == active {
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
                                    secondary.cwd = Some(path);
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::TitleChanged(t) => {
                                    secondary.title = t;
                                    if i == active || show_tab_chrome {
                                        needs_redraw = true;
                                        needs_full_redraw = true;
                                    }
                                }
                                CoreEvent::CommandFinished { .. } => {
                                    secondary.running = false;
                                }
                                CoreEvent::PtyClosed => closed_secondary.push(i),
                            }
                        }
                    }
                }
            }

            if !closed_secondary.is_empty() || !closed_primary.is_empty() {
                closed_secondary.sort_unstable();
                closed_secondary.dedup();
                closed_primary.sort_unstable();
                closed_primary.dedup();

                for idx in closed_secondary.into_iter().rev() {
                    if closed_primary.binary_search(&idx).is_ok() {
                        continue;
                    }
                    if let Some(tab) = state.tabs.get_mut(idx) {
                        tab.close_secondary();
                    }
                }

                for idx in closed_primary.into_iter().rev() {
                    if idx >= state.tabs.len() {
                        continue;
                    }
                    let close_tab = {
                        let tab = &mut state.tabs[idx];
                        if tab.secondary.is_some() {
                            tab.promote_secondary_to_primary();
                            false
                        } else {
                            true
                        }
                    };
                    if !close_tab {
                        continue;
                    }
                    if state.tabs.len() <= 1 {
                        windows_to_close.push(*window_id);
                        break;
                    }
                    state.close_tab(idx);
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
        _ => return None,
    };
    Some(letter - b'@')
}
