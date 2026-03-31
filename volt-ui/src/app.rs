use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
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
        Ok(Self {
            pty,
            performer,
            event_rx,
            title: "~".to_string(),
            cwd: None,
        })
    }

    fn display_title(&self, index: usize) -> String {
        let short = self.cwd.as_deref()
            .and_then(|p| {
                let s = p.to_string_lossy();
                let home = dirs::home_dir();
                if let Some(h) = home {
                    if let Ok(rel) = p.strip_prefix(&h) {
                        return Some(format!("~/{}", rel.display()));
                    }
                }
                Some(s.to_string())
            })
            .unwrap_or_else(|| self.title.clone());

        // Truncate to last ~28 chars
        let truncated = if short.len() > 28 {
            format!("...{}", &short[short.len() - 28..])
        } else {
            short
        };
        format!("{} \u{2318}{}", truncated, index)  // ⌘N
    }
}

// ── settings state ───────────────────────────────────────────────────────────

struct SettingsState {
    visible: bool,
    font_size: f32,
    font_family: String,
    theme_idx: usize,
    focused_field: u8,   // 0=theme 1=font_family 2=font_size
}

impl SettingsState {
    fn from_config(config: &Config) -> Self {
        let theme_idx = Theme::names()
            .iter()
            .position(|&n| n == config.theme)
            .unwrap_or(0);
        Self {
            visible: false,
            font_size: config.font.size,
            font_family: config.font.family.clone(),
            theme_idx,
            focused_field: 0,
        }
    }

    fn cycle_next(&mut self) {
        self.focused_field = (self.focused_field + 1) % 3;
    }

    fn cycle_prev(&mut self) {
        self.focused_field = (self.focused_field + 2) % 3;
    }

    fn apply_up(&mut self) {
        match self.focused_field {
            0 => {
                if self.theme_idx > 0 { self.theme_idx -= 1; }
            }
            2 => { self.font_size = (self.font_size + 1.0).min(48.0); }
            _ => {}
        }
    }

    fn apply_down(&mut self) {
        match self.focused_field {
            0 => {
                if self.theme_idx + 1 < Theme::names().len() { self.theme_idx += 1; }
            }
            2 => { self.font_size = (self.font_size - 1.0).max(6.0); }
            _ => {}
        }
    }

    fn type_char(&mut self, ch: char) {
        if self.focused_field == 1 {
            self.font_family.push(ch);
        }
    }

    fn backspace(&mut self) {
        if self.focused_field == 1 {
            self.font_family.pop();
        }
    }

    fn current_theme_name(&self) -> &'static str {
        Theme::names().get(self.theme_idx).copied().unwrap_or("catppuccin")
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
}

impl WindowState {
    fn active(&mut self) -> &mut TerminalTab {
        &mut self.tabs[self.active_tab]
    }

    fn switch_tab(&mut self, idx: usize) {
        if idx < self.tabs.len() {
            self.active_tab = idx;
        }
    }

    fn new_tab(&mut self) {
        let (cols, rows) = self.renderer.grid_size();
        if let Ok(tab) = TerminalTab::spawn(&self.config, cols as u16, rows as u16) {
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
        }
    }

    fn close_tab(&mut self) {
        if self.tabs.len() <= 1 { return; }
        self.tabs.remove(self.active_tab);
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
        // Update renderer cell dims (font size may have changed)
        self.renderer.update_scale(self.renderer.scale_factor, self.settings.font_size);
        // Resize all PTYs to new grid size
        let (cols, rows) = self.renderer.grid_size();
        for tab in &mut self.tabs {
            let _ = tab.pty.resize(cols as u16, rows as u16);
            tab.performer.lock().unwrap().grid.resize(cols, rows);
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

        // macOS: hide title bar, extend content to full window
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
        let font_size = self.config.font.size;

        let renderer = self.rt.block_on(Renderer::new(window.clone(), font_size, scale_factor));
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
                let font_size = state.config.font.size;
                state.renderer.update_scale(scale_factor as f32, font_size);
                let (cols, rows) = state.renderer.grid_size();
                for tab in &mut state.tabs {
                    let _ = tab.pty.resize(cols as u16, rows as u16);
                    tab.performer.lock().unwrap().grid.resize(cols, rows);
                }
            }

            WindowEvent::ModifiersChanged(mods) => {
                state.modifiers = mods.state();
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state: key_state, text, .. },
                ..
            } => {
                if key_state != ElementState::Pressed { return; }

                let ctrl  = state.modifiers.control_key();
                let super_key = state.modifiers.super_key();

                // ── settings open → handle settings input ────────────────────
                if state.settings.visible {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Escape) => {
                            state.settings.visible = false;
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            if state.modifiers.shift_key() {
                                state.settings.cycle_prev();
                            } else {
                                state.settings.cycle_next();
                            }
                        }
                        PhysicalKey::Code(KeyCode::ArrowUp) => state.settings.apply_up(),
                        PhysicalKey::Code(KeyCode::ArrowDown) => state.settings.apply_down(),
                        PhysicalKey::Code(KeyCode::Backspace) => state.settings.backspace(),
                        PhysicalKey::Code(KeyCode::Enter) => {
                            state.apply_settings();
                            state.settings.visible = false;
                        }
                        _ => {
                            if let Some(text) = text {
                                for ch in text.chars() {
                                    if ch.is_ascii_graphic() || ch == ' ' {
                                        state.settings.type_char(ch);
                                    }
                                }
                            }
                        }
                    }
                    state.window.request_redraw();
                    return;
                }

                // ── global shortcuts (Cmd/Super) ─────────────────────────────
                if super_key {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Comma) => {
                            state.settings.visible = true;
                            state.window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            state.new_tab();
                            state.window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyW) => {
                            state.close_tab();
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
                    PhysicalKey::Code(KeyCode::Enter)     => Some(b"\r"),
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
                // Drain events for ALL tabs, update state
                let active = state.active_tab;
                for (i, tab) in state.tabs.iter_mut().enumerate() {
                    while let Ok(ev) = tab.event_rx.try_recv() {
                        match ev {
                            CoreEvent::GridUpdated => {
                                if i == active {
                                    state.window.request_redraw();
                                }
                            }
                            CoreEvent::CwdChanged(path) => { tab.cwd = Some(path); }
                            CoreEvent::TitleChanged(t)  => { tab.title = t; }
                            CoreEvent::CommandFinished { .. } => {}
                        }
                    }
                }

                // Build tab entries
                let tab_titles: Vec<String> = state.tabs
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
                    })
                    .collect();

                // Settings overlay data
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
                    CoreEvent::GridUpdated => {
                        if i == active { needs_redraw = true; }
                    }
                    CoreEvent::CwdChanged(path) => { tab.cwd = Some(path); }
                    CoreEvent::TitleChanged(t)  => { tab.title = t; }
                    CoreEvent::CommandFinished { .. } => {}
                }
            }
        }
        if needs_redraw {
            state.window.request_redraw();
        }
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
