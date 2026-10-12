use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Welcome,
    Workspace,
    Studio,
    Activity,
    Explorer,
}
impl Screen {
    pub const ALL: [Self; 5] = [
        Self::Welcome,
        Self::Workspace,
        Self::Studio,
        Self::Activity,
        Self::Explorer,
    ];
    pub fn slug(self) -> &'static str {
        match self {
            Self::Welcome => "welcome",
            Self::Workspace => "workspace",
            Self::Studio => "studio",
            Self::Activity => "activity",
            Self::Explorer => "explorer",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Welcome => "Discover",
            Self::Workspace => "Workspace",
            Self::Studio => "Components",
            Self::Activity => "Activity",
            Self::Explorer => "Explorer",
        }
    }
    pub fn from_slug(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.slug() == name)
    }
}
/// Local demo data, deliberately not a pretend service or persisted project.
pub struct Showcase {
    pub screen: Screen,
    pub palette: usize,
    pub week: bool,
    pub completed: [bool; 4],
    pub day: usize,
    pub playing: bool,
    pub track: usize,
    pub saved: bool,
    pub notice: bool,
    pub radius: usize,
    pub clipping: bool,
    pub hidden_scrollbars: bool,
    pub counter: usize,
    pub annual: bool,
    pub subscribed: bool,
}
impl Default for Showcase {
    fn default() -> Self {
        Self {
            screen: Screen::Welcome,
            palette: 0,
            week: true,
            completed: [true, false, false, false],
            day: 2,
            playing: false,
            track: 0,
            saved: false,
            notice: false,
            radius: 1,
            clipping: true,
            hidden_scrollbars: true,
            counter: 0,
            annual: true,
            subscribed: false,
        }
    }
}
pub const RADII: [Rounding; 4] = [
    Rounding::Radius(2.),
    Rounding::Radius(12.),
    Rounding::Radius(24.),
    Rounding::Full,
];
pub fn radius_label(rounding: Rounding) -> String {
    match rounding {
        Rounding::Radius(px) => format!("{px:.0}px"),
        Rounding::Full => "Full".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_radius_preset_is_symbolic_and_has_no_pixel_label() {
        assert_eq!(RADII[3], Rounding::Full);
        assert_eq!(radius_label(RADII[3]), "Full");
        for (preset, label) in RADII[..3].iter().zip(["2px", "12px", "24px"]) {
            assert_eq!(radius_label(*preset), label);
        }
    }
}
pub const PALETTES: [&str; 3] = ["Iris", "Tide", "Ember"];
pub fn theme(app: &super::App, light: bool) -> UiTheme {
    fn rgba(hex: u32) -> [f32; 4] {
        [
            ((hex >> 16) & 255) as f32 / 255.,
            ((hex >> 8) & 255) as f32 / 255.,
            (hex & 255) as f32 / 255.,
            1.,
        ]
    }
    let mut p = if light {
        Palette::light()
    } else {
        Palette::dark()
    };
    p.name = if light {
        "Atelier paper"
    } else {
        "Atelier midnight"
    }
    .into();
    p.base = if light {
        [rgba(0xF5F3EE), rgba(0xFFFFFF), rgba(0xEAE7E1)]
    } else {
        [rgba(0x17181F), rgba(0x21232D), rgba(0x2D303C)]
    };
    p.content = rgba(if light { 0x242634 } else { 0xF4F2ED });
    p.muted = rgba(if light { 0x656471 } else { 0xB5B3C2 });
    p.border = rgba(if light { 0xD6D2DC } else { 0x3B3D4E });
    p = p.with_accent(rgba(match (app.showcase.palette, light) {
        (1, true) => 0x20675B,
        (1, false) => 0x9EE0C5,
        (2, true) => 0xA04B32,
        (2, false) => 0xF5B895,
        (_, true) => 0x6952AF,
        (_, false) => 0xBEB0F4,
    }));
    p.secondary = rgba(if light { 0xA04B32 } else { 0xF5B895 });
    p.on_secondary = rgba(if light { 0xFFFFFF } else { 0x2D221B });
    p.success = rgba(if light { 0x20675B } else { 0x9EE0C5 });
    p.on_success = rgba(if light { 0xFFFFFF } else { 0x132D25 });
    p.warning = rgba(if light { 0x805F16 } else { 0xE9CC85 });
    p.on_warning = rgba(if light { 0xFFFFFF } else { 0x302711 });
    p.error = rgba(if light { 0xA9324C } else { 0xF0A3B4 });
    p.on_error = rgba(if light { 0xFFFFFF } else { 0x35151F });
    p.info = p.primary;
    p.on_info = p.on_primary;
    p.neutral = p.base[2];
    p.on_neutral = p.content;
    UiTheme::from_palette(p)
}
