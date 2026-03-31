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
use volt_renderer::{Renderer, SettingsFocus, SettingsPageData, TabEntry};

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
        let close_w = 16.0 * sc;
        TabLayout { left_pad, tab_w, tab_bar_h, close_w }
    }
    fn tab_x(&self, i: usize) -> f32 { self.left_pad + i as f32 * self.tab_w }
    fn hit_tab(&self, mx: f32, my: f32, n_tabs: usize) -> Option<usize> {
        if my >= self.tab_bar_h { return None; }
        for i in 0..n_tabs {
            let tx = self.tab_x(i);
            if mx >= tx && mx < tx + self.tab_w { return Some(i); }
        }
        None
    }
    fn hit_close(&self, mx: f32, my: f32, i: usize) -> bool {
        if my >= self.tab_bar_h { return false; }
        let tx = self.tab_x(i);
        let cx = tx + self.tab_w - self.close_w - 2.0;
        mx >= cx && mx < tx + self.tab_w
    }
    fn hit_plus(&self, mx: f32, my: f32, n_tabs: usize, sc: f32) -> bool {
        if my >= self.tab_bar_h { return false; }
        let plus_x = self.tab_x(n_tabs);
        mx >= plus_x && mx < plus_x + 26.0 * sc
    }
}

// ── settings window state ────────────────────────────────────────────────────

struct SettingsPage {
    theme_idx: usize,
    padding: u16,
    line_height: f32,
    font_size: f32,
    font_family: String,
    font_scroll: usize,
    font_filter: String,
    focused: SettingsFocus,
}

impl SettingsPage {
    fn from_config(config: &Config, font_families: &[String]) -> Self {
        let theme_idx = Theme::names().iter().position(|&n| n == config.theme).unwrap_or(0);
        let family = config.font.family.clone();
        let fam_idx = font_families.iter().position(|f| *f == family).unwrap_or(0);
        let scroll = fam_idx.saturating_sub(2);
        Self {
            theme_idx,
            padding: config.appearance.padding,
            line_height: config.appearance.line_height,
            font_size: config.font.size,
            font_family: family,
            font_scroll: scroll,
            font_filter: String::new(),
            focused: SettingsFocus::Theme,
        }
    }

    fn cycle_focus(&mut self) {
        self.focused = match self.focused {
            SettingsFocus::Theme      => SettingsFocus::FontSize,
            SettingsFocus::FontSize   => SettingsFocus::Padding,
            SettingsFocus::Padding    => SettingsFocus::LineHeight,
            SettingsFocus::LineHeight => SettingsFocus::FontFilter,
            SettingsFocus::FontFilter => SettingsFocus::FontList,
            SettingsFocus::FontList   => SettingsFocus::Theme,
        };
    }

    #[allow(dead_code)]
    fn cycle_focus_back(&mut self) {
        self.focused = match self.focused {
            SettingsFocus::Theme      => SettingsFocus::FontList,
            SettingsFocus::FontSize   => SettingsFocus::Theme,
            SettingsFocus::Padding    => SettingsFocus::FontSize,
            SettingsFocus::LineHeight => SettingsFocus::Padding,
            SettingsFocus::FontFilter => SettingsFocus::LineHeight,
            SettingsFocus::FontList   => SettingsFocus::FontFilter,
        };
    }

    fn nav_up(&mut self, families: &[String]) {
        match self.focused {
            SettingsFocus::Theme => {
                if self.theme_idx > 0 { self.theme_idx -= 1; }
            }
            SettingsFocus::FontSize => {
                self.font_size = (self.font_size + 1.0).min(48.0);
            }
            SettingsFocus::Padding => {
                self.padding = self.padding.saturating_add(1).min(40);
            }
            SettingsFocus::LineHeight => {
                self.line_height = ((self.line_height + 0.1) * 10.0).round() / 10.0;
                self.line_height = self.line_height.min(3.0);
            }
            SettingsFocus::FontFilter | SettingsFocus::FontList => {
                let filtered = self.filtered_families(families);
                if let Some(pos) = filtered.iter().position(|f| f.as_str() == self.font_family) {
                    if pos > 0 {
                        self.font_family = filtered[pos - 1].to_string();
                        if pos - 1 < self.font_scroll { self.font_scroll = pos - 1; }
                    }
                }
                self.focused = SettingsFocus::FontList;
            }
        }
    }

    fn nav_down(&mut self, families: &[String]) {
        match self.focused {
            SettingsFocus::Theme => {
                if self.theme_idx + 1 < Theme::names().len() { self.theme_idx += 1; }
            }
            SettingsFocus::FontSize => {
                self.font_size = (self.font_size - 1.0).max(6.0);
            }
            SettingsFocus::Padding => {
                self.padding = self.padding.saturating_sub(1);
            }
            SettingsFocus::LineHeight => {
                self.line_height = ((self.line_height - 0.1) * 10.0).round() / 10.0;
                self.line_height = self.line_height.max(0.8);
            }
            SettingsFocus::FontFilter | SettingsFocus::FontList => {
                let filtered = self.filtered_families(families);
                if let Some(pos) = filtered.iter().position(|f| f.as_str() == self.font_family) {
                    if pos + 1 < filtered.len() {
                        self.font_family = filtered[pos + 1].to_string();
                        // scroll down if needed
                        if pos + 1 >= self.font_scroll + 6 { self.font_scroll += 1; }
                    }
                } else if !filtered.is_empty() {
                    self.font_family = filtered[0].to_string();
                    self.font_scroll = 0;
                }
                self.focused = SettingsFocus::FontList;
            }
        }
    }

    fn type_char(&mut self, ch: char, families: &[String]) {
        if self.focused == SettingsFocus::FontFilter {
            self.font_filter.push(ch);
            // reset scroll + auto-select first filtered result
            self.font_scroll = 0;
            let filtered = self.filtered_families(families);
            if let Some(first) = filtered.first() {
                self.font_family = first.to_string();
            }
        }
    }

    fn backspace(&mut self, families: &[String]) {
        if self.focused == SettingsFocus::FontFilter {
            self.font_filter.pop();
            self.font_scroll = 0;
            let filtered = self.filtered_families(families);
            if let Some(first) = filtered.first() {
                self.font_family = first.to_string();
            }
        }
    }

    fn filtered_families<'a>(&self, families: &'a [String]) -> Vec<&'a String> {
        let lower = self.font_filter.to_lowercase();
        families.iter()
            .filter(|f| lower.is_empty() || f.to_lowercase().contains(&lower))
            .collect()
    }

    fn to_page_data<'a>(&'a self) -> (usize, u16, f32, f32, &'a str, usize, &'a str) {
        (self.theme_idx, self.padding, self.line_height, self.font_size,
         &self.font_family, self.font_scroll, &self.font_filter)
    }
}

struct SettingsWin {
    id: WindowId,
    window: Arc<Window>,
    renderer: Renderer,
    page: SettingsPage,
    font_families: Vec<String>,
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
}

impl MainState {
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
            tab.performer.lock().unwrap().grid.resize(cols, rows);
        }
        self.window.request_redraw();
    }

    fn surface_size(&self) -> (f32, f32) {
        let s = self.window.inner_size();
        (s.width as f32, s.height as f32)
    }

    fn handle_click(&mut self, mx: f32, my: f32) {
        let sc = self.renderer.scale_factor;
        let (sw, _sh) = self.surface_size();
        let tbh = self.renderer.tab_bar_height;
        let tl = TabLayout::compute(sw, tbh, self.tabs.len(), sc);
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
    settings: Option<SettingsWin>,
    rt: tokio::runtime::Runtime,
}

impl App {
    pub fn new(config: Config) -> Self {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        Self { config, main: None, settings: None, rt }
    }

    pub fn run(mut self) {
        let event_loop = EventLoop::new().unwrap();
        event_loop.run_app(&mut self).unwrap();
    }

    fn open_settings_window(&mut self, event_loop: &ActiveEventLoop) {
        if self.settings.is_some() {
            if let Some(sw) = &self.settings {
                let _ = sw.window.focus_window();
            }
            return;
        }
        let Some(main) = &self.main else { return };

        let attrs = Window::default_attributes()
            .with_title("Volt — Settings")
            .with_inner_size(winit::dpi::LogicalSize::new(560u32, 510u32))
            .with_resizable(false);

        let window = Arc::new(event_loop.create_window(attrs).unwrap());
        let sc = window.scale_factor() as f32;
        let renderer = self.rt.block_on(
            Renderer::new(window.clone(), 13.0, sc, "monospace", 0.0, 1.4)
        );
        let font_families = renderer.list_monospace_families();
        let page = SettingsPage::from_config(&main.config, &font_families);
        let id = window.id();
        self.settings = Some(SettingsWin { id, window, renderer, page, font_families });
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.main.is_some() { return; }

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

        let first_tab = TerminalTab::spawn(&self.config, cols as u16, rows as u16)
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
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        // ── settings window events ────────────────────────────────────────────
        if let Some(sw) = &self.settings {
            if window_id == sw.id {
                return self.handle_settings_event(event_loop, event);
            }
        }

        // ── main window events ────────────────────────────────────────────────
        let Some(state) = &mut self.main else { return };
        if window_id != state.id { return; }

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

                if super_key {
                    match physical_key {
                        PhysicalKey::Code(KeyCode::Comma) => {
                            self.open_settings_window(event_loop);
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyT) => {
                            self.main.as_mut().unwrap().new_tab();
                            self.main.as_ref().unwrap().window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::KeyW) => {
                            let state = self.main.as_mut().unwrap();
                            let i = state.active_tab;
                            state.close_tab(i);
                            state.window.request_redraw();
                            return;
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => { self.main.as_mut().unwrap().switch_tab(0); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit2) => { self.main.as_mut().unwrap().switch_tab(1); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit3) => { self.main.as_mut().unwrap().switch_tab(2); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit4) => { self.main.as_mut().unwrap().switch_tab(3); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit5) => { self.main.as_mut().unwrap().switch_tab(4); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit6) => { self.main.as_mut().unwrap().switch_tab(5); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit7) => { self.main.as_mut().unwrap().switch_tab(6); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit8) => { self.main.as_mut().unwrap().switch_tab(7); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        PhysicalKey::Code(KeyCode::Digit9) => { self.main.as_mut().unwrap().switch_tab(8); self.main.as_ref().unwrap().window.request_redraw(); return; }
                        _ => {}
                    }
                }

                let state = self.main.as_mut().unwrap();

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
                let state = self.main.as_mut().unwrap();
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
                state.renderer.render_frame(&performer.grid, &state.theme, &tab_entries);
            }

            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(state) = &mut self.main {
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
}

// ── settings window event handling ───────────────────────────────────────────

impl App {
    fn handle_settings_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        let sw = match self.settings.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => {
                self.settings = None;
                return;
            }

            WindowEvent::Resized(size) => {
                sw.renderer.resize(size.width, size.height);
                sw.window.request_redraw();
            }

            WindowEvent::KeyboardInput {
                event: KeyEvent { physical_key, state: key_state, text, .. },
                ..
            } => {
                if key_state != ElementState::Pressed { return; }
                match physical_key {
                    PhysicalKey::Code(KeyCode::Escape) => {
                        self.settings = None;
                        return;
                    }
                    PhysicalKey::Code(KeyCode::Enter) => {
                        self.apply_settings_from_window(event_loop);
                        return;
                    }
                    PhysicalKey::Code(KeyCode::Tab) => {
                        sw.page.cycle_focus();
                    }
                    PhysicalKey::Code(KeyCode::ArrowUp) => {
                        let fams = sw.font_families.clone();
                        sw.page.nav_up(&fams);
                    }
                    PhysicalKey::Code(KeyCode::ArrowDown) => {
                        let fams = sw.font_families.clone();
                        sw.page.nav_down(&fams);
                    }
                    PhysicalKey::Code(KeyCode::Backspace) => {
                        let fams = sw.font_families.clone();
                        sw.page.backspace(&fams);
                    }
                    _ => {
                        if let Some(t) = text {
                            let fams = sw.font_families.clone();
                            for ch in t.chars() {
                                sw.page.type_char(ch, &fams);
                            }
                        }
                    }
                }
                sw.window.request_redraw();
            }

            WindowEvent::RedrawRequested => {
                let sw = self.settings.as_mut().unwrap();
                let (ti, pad, lh, fs, fam, scroll, filter) = sw.page.to_page_data();
                let data = SettingsPageData {
                    theme_idx: ti,
                    theme_names: Theme::names(),
                    padding: pad,
                    line_height: lh,
                    font_size: fs,
                    font_family: fam,
                    font_families: &sw.font_families,
                    font_scroll: scroll,
                    font_filter: filter,
                    focused: sw.page.focused,
                };
                sw.renderer.render_settings_full(&data);
            }

            _ => {}
        }
    }

    fn apply_settings_from_window(&mut self, _event_loop: &ActiveEventLoop) {
        let sw = match self.settings.take() {
            Some(s) => s,
            None => return,
        };

        let theme_name = Theme::names().get(sw.page.theme_idx).copied().unwrap_or("catppuccin");
        let mut new_config = self.config.clone();
        new_config.theme = theme_name.to_string();
        new_config.appearance.padding = sw.page.padding;
        new_config.appearance.line_height = sw.page.line_height;
        new_config.font.size = sw.page.font_size;
        new_config.font.family = sw.page.font_family.clone();
        new_config.save();
        self.config = new_config.clone();

        if let Some(main) = &mut self.main {
            main.apply_config(&new_config);
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
