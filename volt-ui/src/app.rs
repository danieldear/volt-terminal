use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

use volt_config::{sample_config_toml, Config, Theme};
use volt_core::events::CoreEvent;
use volt_core::performer::Performer;
use volt_core::pty::Pty;
use volt_renderer::{Renderer, TabEntry};

// ── user events (used to wake the event loop from background threads) ────────

#[derive(Debug, Clone)]
pub enum VoltEvent {
    /// PTY reader thread produced output — request a redraw.
    PtyData,
}

// ── per-tab state ────────────────────────────────────────────────────────────

struct TerminalTab {
    pty: Pty,
    performer: Arc<Mutex<Performer>>,
    event_rx: tokio::sync::mpsc::UnboundedReceiver<CoreEvent>,
    title: String,
    cwd: Option<PathBuf>,
}

impl TerminalTab {
    fn spawn(
        config: &Config,
        cols: u16,
        rows: u16,
        proxy: EventLoopProxy<VoltEvent>,
    ) -> anyhow::Result<Self> {
        let (pty, performer, event_rx) = Pty::spawn(
            &config.shell.program,
            &config.shell.args,
            cols,
            rows,
            move || { proxy.send_event(VoltEvent::PtyData).ok(); },
        )?;
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
        let short = if raw.len() > 26 { format!("...{}", &raw[raw.len() - 26..]) } else { raw };
        format!("{} \u{2318}{}", short, index)
    }
}

// ── tab bar hit testing ──────────────────────────────────────────────────────

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
        TabLayout { left_pad, tab_w, tab_bar_h, close_w: 16.0 * sc }
    }
    fn tab_x(&self, i: usize) -> f32 { self.left_pad + i as f32 * self.tab_w }
    fn hit_tab(&self, mx: f32, my: f32, n: usize) -> Option<usize> {
        if my >= self.tab_bar_h { return None; }
        for i in 0..n {
            let tx = self.tab_x(i);
            if mx >= tx && mx < tx + self.tab_w { return Some(i); }
        }
        None
    }
    fn hit_close(&self, mx: f32, my: f32, i: usize) -> bool {
        if my >= self.tab_bar_h { return false; }
        let cx = self.tab_x(i) + self.tab_w - self.close_w - 2.0;
        mx >= cx && mx < self.tab_x(i) + self.tab_w
    }
    fn hit_plus(&self, mx: f32, my: f32, n: usize, sc: f32) -> bool {
        if my >= self.tab_bar_h { return false; }
        let plus_x = self.tab_x(n);
        mx >= plus_x && mx < plus_x + 26.0 * sc
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
    config: Config,
    mouse_pos: (f32, f32),
    proxy: EventLoopProxy<VoltEvent>,
}

impl MainState {
    fn active(&mut self) -> &mut TerminalTab { &mut self.tabs[self.active_tab] }

    fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() { self.active_tab = idx; }
    }

    fn new_tab(&mut self) {
        let (cols, rows) = self.renderer.grid_size();
        if let Ok(tab) = TerminalTab::spawn(&self.config, cols as u16, rows as u16, self.proxy.clone()) {
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

    fn apply_config(&mut self, config: &Config) {
        self.theme = Theme::by_name(&config.theme);
        self.config = config.clone();
        let sc = self.renderer.scale_factor;
        self.renderer.font_family = config.font.family.clone();
        self.renderer.padding = config.appearance.padding as f32;
        self.renderer.line_height = config.appearance.line_height;
        self.renderer.update_scale(sc, config.font.size);
        let (cols, rows) = self.renderer.grid_size();
        for tab in &mut self.tabs {
            let _ = tab.pty.resize(cols as u16, rows as u16);
            tab.performer.lock().unwrap().resize(cols, rows);
        }
        self.window.request_redraw();
    }

    fn handle_click(&mut self, mx: f32, my: f32) {
        let sc = self.renderer.scale_factor;
        let sw = self.window.inner_size().width as f32;
        let tl = TabLayout::compute(sw, self.renderer.tab_bar_height, self.tabs.len(), sc);
        if let Some(i) = tl.hit_tab(mx, my, self.tabs.len()) {
            if tl.hit_close(mx, my, i) { self.close_tab(i); }
            else { self.switch_tab(i); }
        } else if tl.hit_plus(mx, my, self.tabs.len(), sc) {
            self.new_tab();
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    config: Config,
    main: Option<MainState>,
    proxy: Option<EventLoopProxy<VoltEvent>>,
    rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(config: Config) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        Self { config, main: None, proxy: None, rt }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::<VoltEvent>::with_user_event().build().unwrap();
        self.proxy = Some(event_loop.create_proxy());
        event_loop.run_app(&mut self).unwrap();
    }
}

// ── config helpers ────────────────────────────────────────────────────────────

fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("volt").join("config.toml"))
}

fn open_config_in_editor() {
    let Some(path) = config_path() else { return };

    if !path.exists() {
        // First run: write the fully-documented sample config
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&path, sample_config_toml());
    } else if let Ok(existing) = std::fs::read_to_string(&path) {
        // Old config (created before documentation was added) — replace it
        if !existing.contains("# Volt Terminal") {
            let _ = std::fs::write(&path, sample_config_toml());
        }
    }

    #[cfg(target_os = "macos")]
    { std::process::Command::new("open").arg(&path).spawn().ok(); }
    #[cfg(not(target_os = "macos"))]
    { std::process::Command::new("xdg-open").arg(&path).spawn().ok(); }
}

// ── ApplicationHandler ────────────────────────────────────────────────────────

impl ApplicationHandler<VoltEvent> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main.is_some() { return; }

        let proxy = self.proxy.as_ref().unwrap().clone();

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
        let renderer = self.rt.block_on(Renderer::new(
            window.clone(),
            self.config.font.size,
            scale_factor,
            &self.config.font.family,
            self.config.appearance.padding as f32,
            self.config.appearance.line_height,
        ));
        let (cols, rows) = renderer.grid_size();

        let first_tab = TerminalTab::spawn(&self.config, cols as u16, rows as u16, proxy.clone())
            .expect("failed to spawn PTY");
        let theme = Theme::by_name(&self.config.theme);
        let id = window.id();

        self.main = Some(MainState {
            id,
            window,
            renderer,
            tabs: vec![first_tab],
            active_tab: 0,
            theme,
            modifiers: ModifiersState::default(),
            config: self.config.clone(),
            mouse_pos: (0.0, 0.0),
            proxy,
        });
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: VoltEvent) {
        match event {
            VoltEvent::PtyData => {
                if let Some(state) = &self.main {
                    state.window.request_redraw();
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
        let Some(state) = &mut self.main else { return };
        if window_id != state.id { return; }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::Resized(size) => {
                state.renderer.resize(size.width, size.height);
                let (cols, rows) = state.renderer.grid_size();
                for tab in &mut state.tabs {
                    let _ = tab.pty.resize(cols as u16, rows as u16);
                    tab.performer.lock().unwrap().resize(cols, rows);
                }
            }

            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                state.renderer.update_scale(scale_factor as f32, state.config.font.size);
                let (cols, rows) = state.renderer.grid_size();
                for tab in &mut state.tabs {
                    let _ = tab.pty.resize(cols as u16, rows as u16);
                    tab.performer.lock().unwrap().resize(cols, rows);
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

                let ctrl      = state.modifiers.control_key();
                let super_key = state.modifiers.super_key();
                let shift     = state.modifiers.shift_key();

                // ── global shortcuts ─────────────────────────────────────────
                if super_key {
                    match physical_key {
                        // Cmd+, → open config file in default editor
                        PhysicalKey::Code(KeyCode::Comma) => {
                            open_config_in_editor();
                            return;
                        }
                        // Cmd+Shift+R → reload config from file
                        PhysicalKey::Code(KeyCode::KeyR) if shift => {
                            let new_cfg = Config::load();
                            self.config = new_cfg.clone();
                            self.main.as_mut().unwrap().apply_config(&new_cfg);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            state.new_tab();
                            state.window.request_redraw();
                            return;
                        }
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
                            CoreEvent::GridUpdated => { if i == active { state.window.request_redraw(); } }
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

                let performer = state.tabs[state.active_tab].performer.lock().unwrap();
                state.renderer.render_frame(
                    &performer.grid,
                    &state.theme,
                    &tab_entries,
                    performer.cursor_visible,
                );
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        // Drain any PTY events that arrived between redraws.
        // The EventLoopProxy already triggers user_event → request_redraw,
        // so this is just a safety drain for events that slipped through.
        let Some(state) = &mut self.main else { return };
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
