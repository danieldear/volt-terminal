use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use volt_config::{Config, Theme};
use volt_core::events::CoreEvent;
use volt_core::performer::Performer;
use volt_core::pty::Pty;
use volt_renderer::{Renderer, SettingsOverlay, TabEntry};

// ── per-tab state ────────────────────────────────────────────────────────────

struct TerminalTab {
    pty: Pty,
    performer: Arc<Mutex<Performer>>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    title: String,
    cwd: Option<PathBuf>,
}

impl TerminalTab {
    fn spawn(config: &Config, cols: u16, rows: u16) -> anyhow::Result<Self> {
        let (pty, performer, event_rx) =
            Pty::spawn(&config.shell.program, &config.shell.args, cols, rows)?;
        Ok(Self { pty, performer, event_rx, title: "~".to_string(), cwd: None })
    }

    fn display_title(&self, index: usize) -> String {
        let raw = self.cwd.as_deref()
            .map(|p| {
                if let Some(home) = dirs::home_dir() {
                    if let Ok(rel) = p.strip_prefix(&home) {
                        return format!("~/{}", rel.display());
                    }
                }
                p.to_string_lossy().to_string()
            })
            .unwrap_or_else(|| self.title.clone());

        let short = if raw.len() > 26 {
            format!("...{}", &raw[raw.len() - 26..])
        } else {
            raw
        };
        format!("{} \u{2318}{}", short, index)
    }
}

// ── settings state ───────────────────────────────────────────────────────────

struct SettingsState {
    visible: bool,
    font_size: f32,
    font_family: String,
    theme_idx: usize,
    focused_field: u8, // 0=theme 1=font_family 2=font_size
}

impl SettingsState {
    fn from_config(config: &Config) -> Self {
        let theme_idx = Theme::names().iter().position(|&n| n == config.theme).unwrap_or(0);
        Self {
            visible: false,
            font_size: config.font.size,
            font_family: config.font.family.clone(),
            theme_idx,
            focused_field: 0,
        }
    }

    fn cycle_next(&mut self) { self.focused_field = (self.focused_field + 1) % 3; }
    fn cycle_prev(&mut self) { self.focused_field = (self.focused_field + 2) % 3; }

    fn apply_up(&mut self) {
        match self.focused_field {
            0 => { if self.theme_idx > 0 { self.theme_idx -= 1; } }
            2 => { self.font_size = (self.font_size + 1.0).min(48.0); }
            _ => {}
        }
    }

    fn apply_down(&mut self) {
        match self.focused_field {
            0 => { if self.theme_idx + 1 < Theme::names().len() { self.theme_idx += 1; } }
            2 => { self.font_size = (self.font_size - 1.0).max(6.0); }
            _ => {}
        }
    }

    fn type_char(&mut self, ch: char) {
        if self.focused_field == 1 && (ch.is_ascii_graphic() || ch == ' ') {
            self.font_family.push(ch);
        }
    }

    fn backspace(&mut self) {
        if self.focused_field == 1 { self.font_family.pop(); }
    }

    fn current_theme_name(&self) -> &'static str {
        Theme::names().get(self.theme_idx).copied().unwrap_or("catppuccin")
    }
}

// ── layout helpers (shared between rendering and hit-testing) ────────────────

/// Compute the layout constants for the tab bar hit regions.
struct TabLayout {
    left_pad: f32,
    tab_w: f32,
    tab_bar_h: f32,
    close_w: f32,
}

impl TabLayout {
    fn compute(sw: f32, tab_bar_h: f32, n_tabs: usize, sc: f32) -> Self {
        let left_pad = (78.0 * sc).round();
        let n = n_tabs.max(1);
        let tab_w = ((sw - left_pad - 32.0 * sc) / n as f32).min(220.0 * sc).max(80.0 * sc);
        let close_w = 16.0 * sc;
        TabLayout { left_pad, tab_w, tab_bar_h, close_w }
    }

    fn tab_x(&self, i: usize) -> f32 { self.left_pad + i as f32 * self.tab_w }

    /// Returns Some(tab_index) if (mx,my) hits a tab, None otherwise.
    fn hit_tab(&self, mx: f32, my: f32, n_tabs: usize) -> Option<usize> {
        if my >= self.tab_bar_h { return None; }
        for i in 0..n_tabs {
            let tx = self.tab_x(i);
            if mx >= tx && mx < tx + self.tab_w { return Some(i); }
        }
        None
    }

    /// Returns true if (mx,my) is on the close × of tab i.
    fn hit_close(&self, mx: f32, my: f32, i: usize) -> bool {
        if my >= self.tab_bar_h { return false; }
        let tx = self.tab_x(i);
        let cx = tx + self.tab_w - self.close_w - 2.0;
        mx >= cx && mx < tx + self.tab_w
    }

    /// Returns true if (mx,my) hits the + button.
    fn hit_plus(&self, mx: f32, my: f32, n_tabs: usize, sc: f32) -> bool {
        if my >= self.tab_bar_h { return false; }
        let plus_x = self.tab_x(n_tabs);
        mx >= plus_x && mx < plus_x + 26.0 * sc
    }
}

/// Compute settings panel layout constants.
struct SettingsLayout {
    px: f32, py: f32, pw: f32, ph: f32,
    field_x: f32,
    row1_y: f32, row2_y: f32, row3_y: f32,
    field_h: f32, size_field_w: f32,
    sc: f32,
}

impl SettingsLayout {
    fn compute(sw: f32, sh: f32, sc: f32) -> Self {
        let pw = (480.0 * sc).min(sw - 40.0 * sc);
        let ph = (360.0 * sc).min(sh - 40.0 * sc);
        let px = ((sw - pw) / 2.0).round();
        let py = ((sh - ph) / 2.0).round();
        let pad = 20.0 * sc;
        let hdr_font = 16.0 * sc;
        let row_h = 34.0 * sc;
        let field_h = row_h * 0.8;
        let size_field_w = 80.0 * sc;

        let sec_y = py + pad + hdr_font * 1.6 + 8.0 * sc;
        let row1_y = sec_y + 10.0 * sc * 1.4 + 6.0 * sc;
        let sec2_y = row1_y + row_h + 16.0 * sc;
        let row2_y = sec2_y + 10.0 * sc * 1.4 + 6.0 * sc;
        let row3_y = row2_y + row_h;
        let field_x = px + pad + 90.0 * sc;

        SettingsLayout { px, py, pw, ph, field_x, row1_y, row2_y, row3_y, field_h, size_field_w, sc }
    }

    fn hit_theme_field(&self, mx: f32, my: f32) -> bool {
        let fw = self.pw - 20.0 * self.sc - 90.0 * self.sc - 20.0 * self.sc;
        mx >= self.field_x && mx < self.field_x + fw &&
        my >= self.row1_y - 2.0 * self.sc && my < self.row1_y - 2.0 * self.sc + self.field_h
    }

    fn hit_family_field(&self, mx: f32, my: f32) -> bool {
        let fw = self.pw - 20.0 * self.sc - 90.0 * self.sc - 20.0 * self.sc;
        mx >= self.field_x && mx < self.field_x + fw &&
        my >= self.row2_y - 2.0 * self.sc && my < self.row2_y - 2.0 * self.sc + self.field_h
    }

    fn hit_size_field(&self, mx: f32, my: f32) -> bool {
        mx >= self.field_x && mx < self.field_x + self.size_field_w &&
        my >= self.row3_y - 2.0 * self.sc && my < self.row3_y - 2.0 * self.sc + self.field_h
    }

    fn hit_minus_btn(&self, mx: f32, my: f32) -> bool {
        let bx = self.field_x + self.size_field_w + 8.0 * self.sc;
        mx >= bx && mx < bx + 26.0 * self.sc &&
        my >= self.row3_y - 2.0 * self.sc && my < self.row3_y - 2.0 * self.sc + self.field_h
    }

    fn hit_plus_btn(&self, mx: f32, my: f32) -> bool {
        let bx = self.field_x + self.size_field_w + 8.0 * self.sc + 30.0 * self.sc;
        mx >= bx && mx < bx + 26.0 * self.sc &&
        my >= self.row3_y - 2.0 * self.sc && my < self.row3_y - 2.0 * self.sc + self.field_h
    }

    fn hit_outside(&self, mx: f32, my: f32) -> bool {
        mx < self.px || mx >= self.px + self.pw || my < self.py || my >= self.py + self.ph
    }
}

// ── window state ─────────────────────────────────────────────────────────────

struct WindowState {
    window: Arc<Window>,
    renderer: Renderer,
    tabs: Vec<TerminalTab>,
    active_tab: usize,
    theme: Theme,
    modifiers: ModifiersState,
    settings: SettingsState,
    config: Config,
    mouse_pos: (f32, f32),
}

impl WindowState {
    fn active(&mut self) -> &mut TerminalTab { &mut self.tabs[self.active_tab] }

    fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() { self.active_tab = idx; }
    }

    fn new_tab(&mut self) {
        let (cols, rows) = self.renderer.grid_size();
        if let Ok(tab) = TerminalTab::spawn(&self.config, cols as u16, rows as u16) {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn close_tab(&mut self, idx: usize) {
        if self.tabs.len() <= 1 { return; }
        self.tabs.remove(idx);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn apply_settings(&mut self) {
        let name = self.settings.current_theme_name();
        self.theme = Theme::by_name(name);
        self.config.theme = name.to_string();
        self.config.font.size = self.settings.font_size;
        self.config.font.family = self.settings.font_family.clone();
        self.config.save();
        self.renderer.font_family = self.settings.font_family.clone();
        self.renderer.update_scale(self.renderer.scale_factor, self.settings.font_size);
        let (cols, rows) = self.renderer.grid_size();
        for tab in &mut self.tabs {
            let _ = tab.pty.resize(cols as u16, rows as u16);
            tab.performer.lock().unwrap().grid.resize(cols, rows);
        }
    }

    fn surface_size(&self) -> (f32, f32) {
        let s = self.window.inner_size();
        (s.width as f32, s.height as f32)
    }

    fn handle_click(&mut self, mx: f32, my: f32) {
        let sc = self.renderer.scale_factor;
        let (sw, sh) = self.surface_size();
        let tbh = self.renderer.tab_bar_height;

        if self.settings.visible {
            let lay = SettingsLayout::compute(sw, sh, sc);
            if lay.hit_outside(mx, my) {
                self.settings.visible = false;
            } else if lay.hit_theme_field(mx, my) {
                self.settings.focused_field = 0;
            } else if lay.hit_family_field(mx, my) {
                self.settings.focused_field = 1;
            } else if lay.hit_size_field(mx, my) {
                self.settings.focused_field = 2;
            } else if lay.hit_minus_btn(mx, my) {
                self.settings.font_size = (self.settings.font_size - 1.0).max(6.0);
                self.settings.focused_field = 2;
            } else if lay.hit_plus_btn(mx, my) {
                self.settings.font_size = (self.settings.font_size + 1.0).min(48.0);
                self.settings.focused_field = 2;
            }
            return;
        }

        let tl = TabLayout::compute(sw, tbh, self.tabs.len(), sc);
        if let Some(i) = tl.hit_tab(mx, my, self.tabs.len()) {
            if tl.hit_close(mx, my, i) {
                self.close_tab(i);
            } else {
                self.switch_tab(i);
            }
        } else if tl.hit_plus(mx, my, self.tabs.len(), sc) {
            self.new_tab();
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    config: Config,
    state: Option<WindowState>,
    rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(config: Config) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        Self { config, state: None, rt }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.run_app(&mut self).unwrap();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() { return; }

        let mut window_attrs = Window::default_attributes()
            .with_title("Volt")
            .with_inner_size(winit::dpi::PhysicalSize::new(1400u32, 900u32));

        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowAttributesExtMacOS;
            window_attrs = window_attrs
                .with_title_hidden(true)
                .with_titlebar_transparent(true)
                .with_fullsize_content_view(true);
        }

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        let scale_factor = window.scale_factor() as f32;
        let renderer = self.rt.block_on(Renderer::new(window.clone(), self.config.font.size, scale_factor, &self.config.font.family));
        let (cols, rows) = renderer.grid_size();

        let first_tab = TerminalTab::spawn(&self.config, cols as u16, rows as u16)
            .expect("failed to spawn PTY");

        let theme = Theme::by_name(&self.config.theme);
        let settings = SettingsState::from_config(&self.config);

        self.state = Some(WindowState {
            window,
            renderer,
            tabs: vec![first_tab],
            active_tab: 0,
            theme,
            modifiers: ModifiersState::default(),
            settings,
            config: self.config.clone(),
            mouse_pos: (0.0, 0.0),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let Some(state) = &mut self.state else { return };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                let (cols, rows) = state.renderer.grid_size();
                for tab in &mut state.tabs {
                    let _ = tab.pty.resize(cols as u16, rows as u16);
                    tab.performer.lock().unwrap().grid.resize(cols, rows);
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.renderer.update_scale(scale_factor as f32, state.config.font.size);
                let (cols, rows) = state.renderer.grid_size();
                for tab in &mut state.tabs {
                    let _ = tab.pty.resize(cols as u16, rows as u16);
                    tab.performer.lock().unwrap().grid.resize(cols, rows);
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::CursorMoved { position, .. } => {
                state.mouse_pos = (position.x as f32, position.y as f32);
            }

            WindowEvent::MouseInput { state: btn_state, button, .. } => {
                if btn_state == ElementState::Pressed && button == MouseButton::Left {
                    let (mx, my) = state.mouse_pos;
                    state.handle_click(mx, my);
                    state.window.request_redraw();
                }
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state: key_state, text, .. },
                ..
            } => {
                if key_state != ElementState::Pressed { return; }

                let ctrl = state.modifiers.control_key();
                let super_key = state.modifiers.super_key();

                // ── settings input ───────────────────────────────────────────
                if state.settings.visible {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            state.settings.visible = false;
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            if state.modifiers.shift_key() { state.settings.cycle_prev(); }
                            else { state.settings.cycle_next(); }
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp)   => state.settings.apply_up(),
                        PhysicalKey::Code(KeyCode::ArrowDown) => state.settings.apply_down(),
                        PhysicalKey::Code(KeyCode::Backspace) => state.settings.backspace(),
                        PhysicalKey::Code(KeyCode::Enter) => {
                            state.apply_settings();
                            state.settings.visible = false;
                        }
                        _ => {
                            if let Some(text) = text {
                                for ch in text.chars() { state.settings.type_char(ch); }
                            }
                        }
                    }
                    state.window.request_redraw();
                    return;
                }

                // ── global shortcuts ─────────────────────────────────────────
                if super_key {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Comma) => {
                            state.settings.visible = !state.settings.visible;
                            state.window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => { state.new_tab(); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::KeyW) => {
                            let i = state.active_tab;
                            state.close_tab(i);
                            state.window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => { state.switch_tab(0); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit2) => { state.switch_tab(1); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit3) => { state.switch_tab(2); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit4) => { state.switch_tab(3); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit5) => { state.switch_tab(4); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit6) => { state.switch_tab(5); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit7) => { state.switch_tab(6); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit8) => { state.switch_tab(7); state.window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit9) => { state.switch_tab(8); state.window.request_redraw(); return; }
                        _ => {}
                    }
                }

                // ── terminal input ────────────────────────────────────────────
                let special: Option<&[u8]> = match physical_key {
                    PhysicalKey::Code(KeyCode::Enter)      => Some(b"\r"),
                    PhysicalKey::Code(KeyCode::Backspace)  => Some(b"\x7f"),
                    PhysicalKey::Code(KeyCode::Tab)        => Some(b"\t"),
                    PhysicalKey::Code(KeyCode::Escape)     => Some(b"\x1b"),
                    PhysicalKey::Code(KeyCode::ArrowUp)    => Some(b"\x1b[A"),
                    PhysicalKey::Code(KeyCode::ArrowDown)  => Some(b"\x1b[B"),
                    PhysicalKey::Code(KeyCode::ArrowRight) => Some(b"\x1b[C"),
                    PhysicalKey::Code(KeyCode::ArrowLeft)  => Some(b"\x1b[D"),
                    PhysicalKey::Code(KeyCode::Home)       => Some(b"\x1b[H"),
                    PhysicalKey::Code(KeyCode::End)        => Some(b"\x1b[F"),
                    PhysicalKey::Code(KeyCode::PageUp)     => Some(b"\x1b[5~"),
                    PhysicalKey::Code(KeyCode::PageDown)   => Some(b"\x1b[6~"),
                    PhysicalKey::Code(KeyCode::Delete)     => Some(b"\x1b[3~"),
                    _ => None,
                };

                if let Some(bytes) = special {
                    let _ = state.active().pty.write(bytes);
                    return;
                }

                if ctrl {
                    if let PhysicalKey::Code(code) = physical_key {
                        if let Some(b) = ctrl_code(code) {
                            let _ = state.active().pty.write(&[b]);
                            return;
                        }
                    }
                }

                if let Some(text) = text {
                    let _ = state.active().pty.write(text.as_str().as_bytes());
                }
            }

            WindowEvent::RedrawRequested => {
                let active = state.active_tab;
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    while let Ok(ev) = tab.event_rx.try_recv() {
                        match ev {
                            CoreEvent::GridUpdated => {
                                if i == active { state.window.request_redraw(); }
                            }
                            CoreEvent::CwdChanged(path) => { tab.cwd = Some(path); }
                            CoreEvent::TitleChanged(t)  => { tab.title = t; }
                            CoreEvent::CommandFinished { .. } => {}
                        }
                    }
                }

                let tab_titles: Vec<String> = state.tabs.iter().enumerate()
                    .map(|(i, t)| t.display_title(i + 1))
                    .collect();
                let tab_entries: Vec<TabEntry> = tab_titles.iter().enumerate()
                    .map(|(i, title)| TabEntry { title, active: i == state.active_tab, index: i + 1 })
                    .collect();

                let settings_overlay = if state.settings.visible {
                    Some(SettingsOverlay {
                        font_size: state.settings.font_size,
                        font_family: &state.settings.font_family,
                        theme_names: Theme::names(),
                        theme_idx: state.settings.theme_idx,
                        focused_field: state.settings.focused_field,
                    })
                } else {
                    None
                };

                let performer = state.tabs[state.active_tab].performer.lock().unwrap();
                state.renderer.render_frame(
                    &performer.grid,
                    &state.theme,
                    &tab_entries,
                    settings_overlay.as_ref(),
                );
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let Some(state) = &mut self.state else { return };
        let active = state.active_tab;
        let mut needs_redraw = false;
        for (i, tab) in state.tabs.iter_mut().enumerate() {
            while let Ok(ev) = tab.event_rx.try_recv() {
                match ev {
                    CoreEvent::GridUpdated => { if i == active { needs_redraw = true; } }
                    CoreEvent::CwdChanged(path) => { tab.cwd = Some(path); }
                    CoreEvent::TitleChanged(t)  => { tab.title = t; }
                    CoreEvent::CommandFinished { .. } => {}
                }
            }
        }
        if needs_redraw { state.window.request_redraw(); }
    }
}

fn ctrl_code(code: KeyCode) -> Option<u8> {
    let letter = match code {
        KeyCode::KeyA => b'A', KeyCode::KeyB => b'B', KeyCode::KeyC => b'C',
        KeyCode::KeyD => b'D', KeyCode::KeyE => b'E', KeyCode::KeyF => b'F',
        KeyCode::KeyG => b'G', KeyCode::KeyH => b'H', KeyCode::KeyI => b'I',
        KeyCode::KeyJ => b'J', KeyCode::KeyK => b'K', KeyCode::KeyL => b'L',
        KeyCode::KeyM => b'M', KeyCode::KeyN => b'N', KeyCode::KeyO => b'O',
        KeyCode::KeyP => b'P', KeyCode::KeyQ => b'Q', KeyCode::KeyR => b'R',
        KeyCode::KeyS => b'S', KeyCode::KeyT => b'T', KeyCode::KeyU => b'U',
        KeyCode::KeyV => b'V', KeyCode::KeyW => b'W', KeyCode::KeyX => b'X',
        KeyCode::KeyY => b'Y', KeyCode::KeyZ => b'Z',
        _ => return None,
    };
    Some(letter - b'@')
}
