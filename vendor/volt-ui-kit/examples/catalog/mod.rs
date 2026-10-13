//! Volt UI catalog: a documentation-style showcase of every component, in the
//! spirit of shadcn/ui and daisyUI docs. Each page shows live previews, the
//! Rust that produced them, variations and an API reference.
pub mod art;
pub mod desktop_services;
pub mod documents;
mod kit;
mod landing;
pub mod lifecycle;
mod pages;
mod shell;
#[cfg(test)]
mod tests;

pub use pages::{Page, PAGES};

/// The id of the current page's first example, if it has one.
pub fn first_section_id(app: &App) -> Option<&'static str> {
    pages::doc(app.page, app).sections.first().map(|s| s.id)
}
pub use shell::{view, OPACITY_MIN};
use std::{
    collections::BTreeSet,
    ops::Range,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use volt_ui_kit::{
    native::{UiSendError, UiSender},
    ui::{Image, Palette, SortDirection, ThemeMode, UiTheme},
};

/// Theme colours, shown as swatches and selectable from the header menu.
pub const THEMES: [(&str, u32, u32); 6] = [
    ("Zinc", 0x18181B, 0xFAFAFA),
    ("Blue", 0x2563EB, 0x3B82F6),
    ("Violet", 0x7C3AED, 0x8B5CF6),
    ("Rose", 0xE11D48, 0xF43F5E),
    ("Green", 0x16A34A, 0x22C55E),
    ("Orange", 0xEA580C, 0xF97316),
];

#[derive(Debug)]
pub enum Message {
    NotificationWake,
    Progress {
        generation: u64,
        step: u32,
    },
    File {
        generation: u64,
        result: Arc<Result<documents::Completion, String>>,
    },
}

pub struct App {
    pub page: Page,
    pub mode: ThemeMode,
    pub theme: usize,
    /// A fully custom theme from the Theme editor: [light, dark]. Choosing
    /// a preset accent clears it.
    pub custom_palettes: Option<[Palette; 2]>,
    pub glass: bool,
    /// Accent tint strength, independent of background opacity.
    pub tint: u8,
    pub opacity: u8,
    /// Explicit opt-in to winit compositor blur (private API on macOS).
    pub compositor_blur: bool,
    pub blur: bool,
    pub sidebar_query: String,
    /// Site search (⌘K): open flag and query.
    pub palette: volt_ui_kit::ui::CommandPaletteState,
    /// Narrow windows: the page list shown as a drawer.
    pub nav_open: bool,
    /// Sidebar width in logical pixels; drag the divider to change it.
    pub sidebar_width: f32,
    pub theme_menu_open: bool,
    /// Section ids currently showing code instead of the preview.
    pub code: BTreeSet<String>,
    pub toast: Option<(u32, String)>,
    next_toast: u32,
    // Component demo state.
    pub checked: bool,
    pub terms: bool,
    pub switch_on: bool,
    pub airplane: bool,
    pub segment: u8,
    pub navigation: u8,
    pub tab: u8,
    pub name: String,
    pub email: String,
    pub password: String,
    pub bio: String,
    pub requests: Option<UiSender<volt_ui_kit::native::ApplicationRequest>>,
    pub documents: documents::State,
    pub services: desktop_services::State,
    pub command_probe: u64,
    pub document_text: String,
    pub saved_document: String,
    pub close_prompt: Option<volt_ui_kit::native::CloseReason>,
    /// The Application lifecycle page's command palette.
    pub app_commands: volt_ui_kit::ui::CommandPaletteState,
    pub log_draft: String,
    pub log_entries: Vec<String>,
    pub volume: f32,
    pub price: f32,
    pub fruit: u8,
    pub fruit_open: bool,
    pub timezone: u8,
    pub timezone_open: bool,
    pub dialog_open: bool,
    pub delete_open: bool,
    pub menu_open: bool,
    pub actions_open: bool,
    pub context_open: bool,
    pub context_at: [f32; 2],
    pub popover_open: bool,
    pub width: f32,
    pub height: f32,
    pub sort: Option<(u8, SortDirection)>,
    pub row: Option<u32>,
    pub expanded: Vec<u16>,
    pub file: Option<u16>,
    pub window: Range<usize>,
    pub record: Option<usize>,
    pub split: f32,
    pub reveal: bool,
    /// Sign-in demo: the password is shown in plain text.
    pub show_password: bool,
    pub liked: bool,
    pub last_action: String,
    pub image: Image,
    /// Demo state for the parity component pages.
    pub kit: kit::State,
    /// The landing page's live counter demo.
    pub counter: u32,
    // Background task demo.
    pub sender: Option<UiSender<Message>>,
    pub job_running: bool,
    pub job_progress: u32,
    job_generation: u64,
    job_cancel: Arc<AtomicBool>,
}

impl App {
    pub fn new() -> Self {
        Self {
            requests: None,
            documents: documents::State::default(),
            services: desktop_services::State::default(),
            command_probe: 0,
            document_text: "A small document, owned by your Rust application.\nEdit it to test Save / Discard / Cancel.".into(),
            saved_document: "A small document, owned by your Rust application.\nEdit it to test Save / Discard / Cancel.".into(),
            close_prompt: None,
            app_commands: Default::default(),
            log_draft: String::new(),
            log_entries: (1..=12)
                .map(|i| format!("Event {i:02} · UI thread ready"))
                .collect(),
            page: Page::Introduction,
            mode: ThemeMode::Light,
            theme: 0,
            custom_palettes: None,
            glass: false,
            tint: 0,
            opacity: 72,
            compositor_blur: false,
            blur: true,
            sidebar_query: String::new(),
            palette: Default::default(),
            nav_open: false,
            sidebar_width: 248.,
            theme_menu_open: false,
            code: BTreeSet::new(),
            toast: None,
            next_toast: 0,
            checked: true,
            terms: false,
            switch_on: true,
            airplane: false,
            segment: 1,
            navigation: 0,
            tab: 0,
            name: "Ada Lovelace".into(),
            email: String::new(),
            password: String::new(),
            bio: "Mathematician. Wrote the first published algorithm intended for a machine."
                .into(),
            volume: 0.6,
            price: 40.,
            fruit: 1,
            fruit_open: false,
            timezone: 2,
            timezone_open: false,
            dialog_open: false,
            delete_open: false,
            menu_open: false,
            actions_open: false,
            context_open: false,
            context_at: [0.; 2],
            popover_open: false,
            width: 100.,
            height: 25.,
            sort: Some((0, SortDirection::Ascending)),
            row: Some(2),
            expanded: vec![1, 2],
            file: Some(4),
            window: 0..14,
            record: None,
            split: 0.4,
            reveal: true,
            show_password: false,
            liked: false,
            last_action: "Nothing yet".into(),
            image: art::landscape(480, 300),
            kit: kit::State::default(),
            counter: 0,
            sender: None,
            job_running: false,
            job_progress: 0,
            job_generation: 0,
            job_cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn go(&mut self, page: Page) {
        self.page = page;
        self.app_commands.open = false;
        self.app_commands.query.clear();
        self.palette.open = false;
        self.palette.query.clear();
        self.nav_open = false;
        // Close every open overlay that belonged to the previous page.
        self.fruit_open = false;
        self.timezone_open = false;
        self.menu_open = false;
        self.actions_open = false;
        self.context_open = false;
        self.popover_open = false;
        self.theme_menu_open = false;
        self.dialog_open = false;
        self.delete_open = false;
        self.kit.close_overlays();
    }
    pub fn append_log(&mut self) {
        let value = self.log_draft.trim();
        if value.is_empty() {
            return;
        }
        self.log_entries.push(value.to_owned());
        self.log_draft.clear();
        if self.log_entries.len() > 32 {
            self.log_entries.remove(0);
        }
    }
    pub fn notify(&mut self, message: impl Into<String>) {
        self.next_toast = self.next_toast.wrapping_add(1);
        self.toast = Some((self.next_toast, message.into()));
    }
    pub fn start_job(&mut self) {
        let Some(sender) = self.sender.clone() else {
            self.notify("Background workers run in the native window only");
            return;
        };
        self.job_cancel.store(true, Ordering::Relaxed);
        self.job_cancel = Arc::new(AtomicBool::new(false));
        self.job_generation = self.job_generation.wrapping_add(1);
        self.job_progress = 0;
        self.job_running = true;
        let (generation, cancel) = (self.job_generation, self.job_cancel.clone());
        std::thread::spawn(move || {
            for step in 1..=100 {
                std::thread::sleep(Duration::from_millis(30));
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                if !send_progress(&cancel, step, || {
                    sender.send(Message::Progress { generation, step })
                }) {
                    break;
                }
            }
        });
    }
    pub fn cancel_job(&mut self) {
        self.job_cancel.store(true, Ordering::Relaxed);
        self.job_generation = self.job_generation.wrapping_add(1);
        self.job_running = false;
        self.notify("Task cancelled");
    }
}

// Intermediate updates may coalesce, but completion must survive backpressure.
// Retry only on the worker thread; cancellation/closure bound its lifetime.
fn send_progress(
    cancel: &AtomicBool,
    step: u32,
    mut send: impl FnMut() -> Result<(), UiSendError>,
) -> bool {
    loop {
        if cancel.load(Ordering::Relaxed) {
            return false;
        }
        match send() {
            Ok(()) => return true,
            Err(UiSendError::Closed) => return false,
            Err(UiSendError::Full) if step < 100 => return true,
            Err(UiSendError::Full) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        self.job_cancel.store(true, Ordering::Relaxed);
    }
}

/// Worker messages arrive here, on the UI thread, in batches.
pub fn update(app: &mut App, message: Message) {
    desktop_services::drain(app);
    let (generation, step) = match message {
        Message::NotificationWake => return,
        Message::File { generation, result } => {
            documents::finish(app, generation, &result);
            return;
        }
        Message::Progress { generation, step } => (generation, step),
    };
    // Results from a cancelled or restarted task are ignored.
    if generation != app.job_generation || !app.job_running {
        return;
    }
    app.job_progress = step.min(100);
    if step >= 100 {
        app.job_running = false;
        app.notify("Task complete");
    }
}

fn rgba(v: u32) -> [f32; 4] {
    [
        ((v >> 16) & 255) as f32 / 255.,
        ((v >> 8) & 255) as f32 / 255.,
        (v & 255) as f32 / 255.,
        1.,
    ]
}

/// The palette in use: the Theme editor's custom palette when set, else the
/// selected preset.
pub fn effective_palette(app: &App, dark: bool) -> Palette {
    match &app.custom_palettes {
        Some(custom) => custom[usize::from(dark)].clone(),
        None => preset_palette(app, dark),
    }
}

pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let dark = match app.mode {
        ThemeMode::System => system == ThemeMode::Dark,
        mode => mode == ThemeMode::Dark,
    };
    let p = effective_palette(app, dark);
    let mut background = p.base[0];
    if app.glass {
        let tint = app.tint as f32 / 100.;
        // Neutral tint must not invert the sidebar's light/dark text contrast.
        let tint_color = if app.theme == 0 && app.custom_palettes.is_none() {
            p.base[1]
        } else {
            p.primary
        };
        for (i, component) in background.iter_mut().enumerate().take(3) {
            *component = p.base[0][i] * (1. - tint) + tint_color[i] * tint;
        }
    }
    UiTheme::from_palette(p).with_background_color(background)
}

/// Neutral zinc surfaces with a selectable primary colour.
pub fn preset_palette(app: &App, dark: bool) -> Palette {
    let mut p = if dark {
        Palette::dark()
    } else {
        Palette::light()
    };
    p.name = THEMES[app.theme].0.into();
    if dark {
        p.base = [rgba(0x09090B), rgba(0x18181B), rgba(0x27272A)];
        p.content = rgba(0xFAFAFA);
        p.muted = rgba(0xA1A1AA);
        p.border = rgba(0x27272A);
        p.neutral = rgba(0xFAFAFA);
        p.on_neutral = rgba(0x18181B);
        p.error = rgba(0xDC2626);
        p.on_error = rgba(0xFAFAFA);
    } else {
        p.base = [rgba(0xFFFFFF), rgba(0xF4F4F5), rgba(0xE4E4E7)];
        p.content = rgba(0x09090B);
        p.muted = rgba(0x71717A);
        p.border = rgba(0xE4E4E7);
        p.neutral = rgba(0x18181B);
        p.on_neutral = rgba(0xFAFAFA);
        p.error = rgba(0xDC2626);
        p.on_error = rgba(0xFFFFFF);
    }
    p.success = rgba(if dark { 0x22C55E } else { 0x16A34A });
    p.on_success = rgba(0xFFFFFF);
    p.warning = rgba(if dark { 0xF59E0B } else { 0xD97706 });
    p.on_warning = rgba(0x18181B);
    p.info = rgba(if dark { 0x3B82F6 } else { 0x2563EB });
    p.on_info = rgba(0xFFFFFF);
    let (_, light_primary, dark_primary) = THEMES[app.theme];
    if app.theme == 0 {
        // Zinc: the primary colour is the text colour itself.
        let mut p = p;
        p.primary = p.content;
        p.on_primary = p.base[0];
        p.accent = p.content;
        p.on_accent = p.base[0];
        p
    } else {
        p.with_accent(rgba(if dark { dark_primary } else { light_primary }))
    }
}

pub fn window_background(app: &App) -> volt_ui_kit::native::WindowBackground {
    use volt_ui_kit::native::WindowBackground;
    let opacity = app.opacity as f32 / 100.;
    if !app.glass {
        WindowBackground::Opaque
    } else if !app.blur {
        WindowBackground::Transparent { opacity }
    } else if app.compositor_blur {
        WindowBackground::CompositorBlur { opacity }
    } else {
        WindowBackground::Blur { opacity }
    }
}

/// Development restarts (`cargo volt dev`): keep the page and appearance.
pub fn dev_save(app: &App) -> String {
    format!(
        "page={}\nmode={}\ntheme={}\nsidebar={}\nglass={}\n",
        app.page.slug(),
        match app.mode {
            ThemeMode::Dark => "dark",
            ThemeMode::Light => "light",
            ThemeMode::System => "system",
        },
        app.theme,
        app.sidebar_width,
        app.glass
    )
}
pub fn dev_load(app: &mut App, saved: &str) {
    for line in saved.lines() {
        match line.split_once('=') {
            Some(("page", slug)) => {
                if let Some(page) = PAGES.into_iter().find(|p| p.slug() == slug) {
                    app.go(page);
                }
            }
            Some(("mode", "dark")) => app.mode = ThemeMode::Dark,
            Some(("mode", "light")) => app.mode = ThemeMode::Light,
            Some(("mode", "system")) => app.mode = ThemeMode::System,
            Some(("theme", v)) => {
                if let Ok(i) = v.parse::<usize>() {
                    app.theme = i.min(THEMES.len() - 1);
                }
            }
            Some(("sidebar", v)) => {
                if let Ok(w) = v.parse::<f32>() {
                    app.sidebar_width = w.clamp(200., 420.);
                }
            }
            Some(("glass", v)) => app.glass = v == "true",
            _ => {}
        }
    }
}
