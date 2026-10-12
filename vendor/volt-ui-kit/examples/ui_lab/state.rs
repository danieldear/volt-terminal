use crate::support::RustLaunchOptions;
use std::collections::VecDeque;
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ui::prelude::*,
};

/// One registry supplies navigation, CLI routes and the test matrix.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    #[default]
    Controls,
    Layout,
    Typography,
    Effects,
    Surfaces,
    Scrolling,
    Window,
    Showcase,
    Runtime,
    Data,
}
impl Section {
    pub const ALL: [Self; 10] = [
        Self::Controls,
        Self::Layout,
        Self::Typography,
        Self::Effects,
        Self::Surfaces,
        Self::Scrolling,
        Self::Window,
        Self::Runtime,
        Self::Data,
        Self::Showcase,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Controls => "controls",
            Self::Layout => "layout",
            Self::Typography => "typography",
            Self::Effects => "effects",
            Self::Surfaces => "surfaces",
            Self::Scrolling => "scrolling",
            Self::Window => "window",
            Self::Showcase => "showcase",
            Self::Runtime => "runtime",
            Self::Data => "data",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Controls => "Controls",
            Self::Layout => "Layout & spacing",
            Self::Typography => "Text & colors",
            Self::Effects => "Effects & motion",
            Self::Surfaces => "Images, shapes & clipping",
            Self::Scrolling => "Scrolling",
            Self::Window => "Window & platform",
            Self::Showcase => "Showcase",
            Self::Runtime => "Runtime & overlays",
            Self::Data => "Data & split panes",
        }
    }
    pub fn nav_label(self) -> &'static str {
        match self {
            Self::Layout => "Layout",
            Self::Typography => "Text",
            Self::Effects => "Effects",
            Self::Surfaces => "Surfaces",
            Self::Window => "Window",
            Self::Runtime => "Runtime",
            Self::Data => "Data",
            _ => self.title(),
        }
    }
    pub fn from_slug(slug: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == slug)
    }
    pub fn next(self, reverse: bool) -> Self {
        let index = Self::ALL.iter().position(|s| *s == self).unwrap();
        Self::ALL[(index + if reverse { Self::ALL.len() - 1 } else { 1 }) % Self::ALL.len()]
    }
}
pub const ACCENTS: [(&str, [f32; 4]); 3] = [
    ("Blue", [0.20, 0.55, 0.95, 1.]),
    ("Violet", [0.58, 0.42, 0.91, 1.]),
    ("Coral", [0.93, 0.39, 0.30, 1.]),
];
pub const OPACITIES: [f32; 3] = [1., 0.65, 0.35];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ComponentPage {
    #[default]
    Overview,
    Activity,
    Details,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Density {
    Compact,
    #[default]
    Comfortable,
    Spacious,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortOrder {
    #[default]
    Recent,
    Name,
    Size,
}
pub struct App {
    pub section: Section,
    pub showcase: super::showcase::Showcase,
    pub mode: ThemeMode,
    pub accent: usize,
    pub font: String,
    pub count: usize,
    pub notifications: bool,
    pub approved: bool,
    pub component_page: ComponentPage,
    pub density: Density,
    pub bottom_tabs: bool,
    pub draft: String,
    pub notes: String,
    pub password: String,
    pub volume: f32,
    pub sort: SortOrder,
    pub sort_open: bool,
    pub dialog_open: bool,
    pub submitted: usize,
    pub enabled: bool,
    pub tasks: Vec<u64>,
    pub saved: usize,
    pub notice: bool,
    pub flow: Flow,
    pub spacing: usize,
    pub rounding: usize,
    pub back: usize,
    pub front: usize,
    pub opacity: usize,
    pub image_fit: ImageFit,
    pub image_position: ImagePosition,
    pub clip: usize,
    pub hidden_bars: bool,
    pub opened: Option<usize>,
    pub actions: usize,
    pub history: VecDeque<String>,
    pub image: Image,
    pub background: WindowBackground,
    pub chrome: WindowChrome,
    pub reduced_motion: bool,
    pub sender: Option<volt_ui_kit::native::UiSender<super::jobs::Message>>,
    pub job_cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub job_generation: u64,
    pub job_progress: usize,
    pub job_running: bool,
    pub menu_open: bool,
    pub context_open: bool,
    pub context_position: [f32; 2],
    pub popover_open: bool,
    pub list_window: std::ops::Range<usize>,
    pub list_selected: Option<usize>,
    pub split_ratio: f32,
    pub table_column: usize,
    pub table_direction: SortDirection,
    pub table_selected: Option<usize>,
    pub tree_expanded: Vec<usize>,
    pub tree_selected: Option<usize>,
}
impl Default for App {
    fn default() -> Self {
        let (w, h) = (160, 100);
        let pixels = (0..w * h)
            .flat_map(|i| {
                let (x, y) = (i % w, i / w);
                if ((x / 20) + (y / 20)) % 2 == 0 {
                    [40 + x as u8, 112 + y as u8, 207, 255]
                } else {
                    [203, 100 + (x * 3 / 4) as u8, 65 + y as u8, 255]
                }
            })
            .collect();
        Self {
            section: Section::Controls,
            showcase: super::showcase::Showcase::default(),
            mode: ThemeMode::Dark,
            accent: 0,
            font: "system".into(),
            count: 0,
            notifications: false,
            approved: false,
            component_page: ComponentPage::Overview,
            density: Density::Comfortable,
            bottom_tabs: false,
            draft: "Hello, Volt".into(),
            notes: "Editable plain text.\nTry an emoji or combining accent.".into(),
            password: "demo-only".into(),
            volume: 40.,
            sort: SortOrder::Recent,
            sort_open: false,
            dialog_open: false,
            submitted: 0,
            enabled: true,
            tasks: vec![1, 2, 3],
            saved: 0,
            notice: true,
            flow: Flow::Center,
            spacing: 1,
            rounding: 0,
            back: 0,
            front: 0,
            opacity: 0,
            image_fit: ImageFit::Cover,
            image_position: ImagePosition::Center,
            clip: 0,
            hidden_bars: false,
            opened: None,
            actions: 0,
            history: VecDeque::new(),
            image: Image::from_rgba(w, h, pixels).expect("bounded generated test asset"),
            background: WindowBackground::Opaque,
            chrome: WindowChrome::Overlay,
            reduced_motion: false,
            sender: None,
            job_cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            job_generation: 0,
            job_progress: 0,
            job_running: false,
            menu_open: false,
            context_open: false,
            context_position: [0., 0.],
            popover_open: false,
            list_window: 0..12,
            list_selected: None,
            split_ratio: 0.5,
            table_column: 0,
            table_direction: SortDirection::Ascending,
            table_selected: None,
            tree_expanded: vec![1],
            tree_selected: None,
        }
    }
}
impl App {
    pub fn record(&mut self, message: impl Into<String>) {
        self.actions += 1;
        self.history.push_front(message.into());
        self.history.truncate(20);
    }
    pub fn navigate(&mut self, section: Section) {
        self.section = section;
        self.record(format!("Opened {}", section.title()));
    }
    pub fn last_action(&self) -> &str {
        self.history
            .front()
            .map_or("Ready. Try a control or choose a section.", String::as_str)
    }
    pub fn reset_demos(&mut self) {
        let old = std::mem::take(self);
        old.job_cancel
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.sender = old.sender;
        self.job_generation = old.job_generation.wrapping_add(1);
        self.section = old.section;
        self.mode = old.mode;
        self.accent = old.accent;
        self.showcase.palette = old.showcase.palette;
        self.showcase.screen = old.showcase.screen;
        self.font = old.font;
        self.background = old.background;
        self.chrome = old.chrome;
        self.reduced_motion = old.reduced_motion;
        self.actions = old.actions;
        self.history = old.history;
        self.image = old.image;
        self.record("Reset demo values; appearance and current section kept");
    }
}
fn rgba(hex: u32) -> [f32; 4] {
    [
        ((hex >> 16) & 255) as f32 / 255.,
        ((hex >> 8) & 255) as f32 / 255.,
        (hex & 255) as f32 / 255.,
        1.,
    ]
}
pub fn theme(app: &App, system: ThemeMode) -> UiTheme {
    let light = match app.mode {
        ThemeMode::System => system == ThemeMode::Light,
        mode => mode == ThemeMode::Light,
    };
    if app.section == Section::Showcase {
        return super::showcase::theme(app, light);
    }
    let mut palette = if light {
        Palette::light()
    } else {
        Palette::dark()
    };
    palette.name = if light { "Lab light" } else { "Lab dark" }.into();
    palette.base = if light {
        [rgba(0xF2F5F9), rgba(0xFFFFFF), rgba(0xDFE7F1)]
    } else {
        [rgba(0x121B2B), rgba(0x1C2A3D), rgba(0x2B3D54)]
    };
    palette.content = rgba(if light { 0x1A2940 } else { 0xE9F0F8 });
    palette.border = rgba(if light { 0xCAD5E2 } else { 0x405B77 });
    palette.muted = rgba(if light { 0x566881 } else { 0xA6B7CB });
    UiTheme::from_palette(palette.with_accent(ACCENTS[app.accent].1))
}

/// App-specific flags are consumed here; common host flags are forwarded unchanged.
pub fn parse_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(App, RustLaunchOptions), Box<dyn std::error::Error>> {
    let mut app = App::default();
    // The app owns its selection; the host supplies live OS appearance to System.
    let mut forwarded = vec!["--theme".into(), "system".into()];
    let mut background = "opaque";
    let mut opacity = 0.65;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--section" => {
                let name = args.next().ok_or("--section requires a name")?;
                app.section = Section::from_slug(&name).ok_or(
                    "unknown section: controls, layout, typography, effects, surfaces, scrolling, window, showcase",
                )?;
            }
            "--palette" => {
                app.showcase.palette = match args.next().as_deref() {
                    Some("iris") => 0,
                    Some("tide") => 1,
                    Some("ember") => 2,
                    _ => return Err("--palette must be iris, tide or ember".into()),
                };
            }
            "--screen" => {
                let name = args
                    .next()
                    .ok_or("--screen requires welcome, workspace, studio, activity or explorer")?;
                app.showcase.screen = super::showcase::Screen::from_slug(&name)
                    .ok_or("--screen must be welcome, workspace, studio, activity or explorer")?;
                app.section = Section::Showcase;
            }
            "--theme" => {
                app.mode = match args.next().as_deref() {
                    Some("light") => ThemeMode::Light,
                    Some("dark") => ThemeMode::Dark,
                    Some("system") => ThemeMode::System,
                    _ => return Err("--theme must be light, dark or system".into()),
                };
            }
            "--accent" => {
                app.accent = match args.next().as_deref() {
                    Some("blue") => 0,
                    Some("violet") => 1,
                    Some("coral") => 2,
                    _ => return Err("--accent must be blue, violet or coral".into()),
                };
            }
            "--font" => {
                app.font = args
                    .next()
                    .ok_or("--font requires an installed family name")?;
                if app.font.trim().is_empty() || app.font.len() > 128 {
                    return Err("--font must be a nonempty family name up to 128 bytes".into());
                }
            }
            "--background" => {
                background = match args.next().as_deref() {
                    Some("opaque") => "opaque",
                    Some("transparent") => "transparent",
                    Some("blur") => "blur",
                    _ => return Err("--background must be opaque, transparent or blur".into()),
                };
            }
            "--window-opacity" => {
                opacity = args
                    .next()
                    .ok_or("--window-opacity requires a value")?
                    .parse::<f32>()?;
                if !opacity.is_finite() || !(0.0..=1.).contains(&opacity) {
                    return Err("--window-opacity must be finite and in [0,1]".into());
                }
            }
            "--chrome" => {
                app.chrome = match args.next().as_deref() {
                    Some("native") => WindowChrome::Native,
                    Some("overlay") => WindowChrome::Overlay,
                    Some("borderless") => WindowChrome::Borderless,
                    _ => return Err("--chrome must be native, overlay or borderless".into()),
                };
            }
            "--reduced-motion" => {
                app.reduced_motion = true;
                forwarded.push(arg);
            }
            "--help" => {
                println!(concat!(
                    "Volt UI Lab — one interactive test app\n",
                    "--section controls|layout|typography|effects|surfaces|scrolling|window|showcase\n",
                    "--screen welcome|workspace|studio|activity|explorer (opens Showcase)\n",
                    "--palette iris|tide|ember (Showcase)\n",
                    "--accent blue|violet|coral\n",
                    "--font FAMILY     installed family name (fallback if unavailable)\n",
                    "--background opaque|transparent|blur\n",
                    "--window-opacity NUMBER  root clear opacity, 0..1 (default 0.65)\n",
                    "Transparency/blur require native-gpu and a supporting OS/compositor.\n",
                    "Values are in memory; the app does not write preferences or use the network."
                ));
                forwarded.push(arg);
            }
            _ => forwarded.push(arg),
        }
    }
    app.background = match background {
        "transparent" => WindowBackground::Transparent { opacity },
        "blur" => WindowBackground::Blur { opacity },
        _ => WindowBackground::Opaque,
    };
    let options = RustLaunchOptions {
        chrome: app.chrome,
        background: app.background,
        args: forwarded,
    };
    Ok((app, options))
}
