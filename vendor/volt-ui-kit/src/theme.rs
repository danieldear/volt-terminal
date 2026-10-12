//! Semantic UI palettes independent of Volt's terminal/ANSI theme contract.
use crate::{Colors, Theme};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorToken {
    Base100,
    Base200,
    Base300,
    BaseContent,
    Primary,
    PrimaryContent,
    Secondary,
    SecondaryContent,
    Accent,
    AccentContent,
    Neutral,
    NeutralContent,
    Info,
    InfoContent,
    Success,
    SuccessContent,
    Warning,
    WarningContent,
    Error,
    ErrorContent,
    Border,
    Muted,
}
impl ColorToken {
    pub fn name(self) -> &'static str {
        match self {
            Self::Base100 => "base-100",
            Self::Base200 => "base-200",
            Self::Base300 => "base-300",
            Self::BaseContent => "base-content",
            Self::Primary => "primary",
            Self::PrimaryContent => "primary-content",
            Self::Secondary => "secondary",
            Self::SecondaryContent => "secondary-content",
            Self::Accent => "accent",
            Self::AccentContent => "accent-content",
            Self::Neutral => "neutral",
            Self::NeutralContent => "neutral-content",
            Self::Info => "info",
            Self::InfoContent => "info-content",
            Self::Success => "success",
            Self::SuccessContent => "success-content",
            Self::Warning => "warning",
            Self::WarningContent => "warning-content",
            Self::Error => "error",
            Self::ErrorContent => "error-content",
            Self::Border => "border",
            Self::Muted => "muted",
        }
    }
    /// Parse `@name`, optionally with an opacity suffix `@name/NN` (0..=100).
    pub fn parse(value: &str) -> Option<Self> {
        Self::parse_with_alpha(value).map(|(token, _)| token)
    }
    /// Parse a token and its opacity (1.0 when no `/NN` suffix is given).
    pub fn parse_with_alpha(value: &str) -> Option<(Self, f32)> {
        let value = value.strip_prefix('@')?;
        let (name, alpha) = match value.split_once('/') {
            Some((name, percent)) => {
                let percent: u8 = percent.parse().ok().filter(|p| *p <= 100)?;
                (name, f32::from(percent) / 100.)
            }
            None => (value, 1.),
        };
        let token = [
            Self::Base100,
            Self::Base200,
            Self::Base300,
            Self::BaseContent,
            Self::Primary,
            Self::PrimaryContent,
            Self::Secondary,
            Self::SecondaryContent,
            Self::Accent,
            Self::AccentContent,
            Self::Neutral,
            Self::NeutralContent,
            Self::Info,
            Self::InfoContent,
            Self::Success,
            Self::SuccessContent,
            Self::Warning,
            Self::WarningContent,
            Self::Error,
            Self::ErrorContent,
            Self::Border,
            Self::Muted,
        ]
        .into_iter()
        .find(|v| v.name() == name)?;
        Some((token, alpha))
    }
    /// This role at a percentage opacity, still resolved from the active
    /// theme: `ColorToken::Primary.alpha(15)` is a primary tint.
    pub fn alpha(self, percent: u8) -> String {
        format!("@{}/{}", self.name(), percent.min(100))
    }
}
impl From<ColorToken> for String {
    fn from(value: ColorToken) -> Self {
        format!("@{}", value.name())
    }
}
/// Each semantic role has a paired foreground. Hosts can supply a named palette.
#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub name: String,
    pub base: [[f32; 4]; 3],
    pub content: [f32; 4],
    pub primary: [f32; 4],
    pub on_primary: [f32; 4],
    pub secondary: [f32; 4],
    pub on_secondary: [f32; 4],
    pub accent: [f32; 4],
    pub on_accent: [f32; 4],
    pub neutral: [f32; 4],
    pub on_neutral: [f32; 4],
    pub info: [f32; 4],
    pub on_info: [f32; 4],
    pub success: [f32; 4],
    pub on_success: [f32; 4],
    pub warning: [f32; 4],
    pub on_warning: [f32; 4],
    pub error: [f32; 4],
    pub on_error: [f32; 4],
    pub border: [f32; 4],
    pub muted: [f32; 4],
}
pub(crate) fn contrast(c: [f32; 4]) -> [f32; 4] {
    let linear = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let luminance = 0.2126 * linear(c[0]) + 0.7152 * linear(c[1]) + 0.0722 * linear(c[2]);
    if luminance > 0.179 {
        [0., 0., 0., 1.]
    } else {
        [1., 1., 1., 1.]
    }
}
impl Palette {
    pub fn dark() -> Self {
        UiTheme::from_legacy(&Theme::dark()).palette
    }
    pub fn light() -> Self {
        UiTheme::from_legacy(&Theme::light()).palette
    }
    pub fn with_accent(mut self, c: [f32; 4]) -> Self {
        self.accent = c;
        self.on_accent = contrast(c);
        self.primary = c;
        self.on_primary = contrast(c);
        self
    }
    pub fn is_valid(&self) -> bool {
        self.name.len() <= 128
            && self.all_colors().into_iter().all(|c| {
                c.into_iter()
                    .all(|v| v.is_finite() && (0. ..=1.).contains(&v))
            })
    }
    fn all_colors(&self) -> Vec<[f32; 4]> {
        let mut out = self.base.to_vec();
        out.extend([
            self.content,
            self.primary,
            self.on_primary,
            self.secondary,
            self.on_secondary,
            self.accent,
            self.on_accent,
            self.neutral,
            self.on_neutral,
            self.info,
            self.on_info,
            self.success,
            self.on_success,
            self.warning,
            self.on_warning,
            self.error,
            self.on_error,
            self.border,
            self.muted,
        ]);
        out
    }
    pub fn color(&self, t: ColorToken) -> [f32; 4] {
        match t {
            ColorToken::Base100 => self.base[0],
            ColorToken::Base200 => self.base[1],
            ColorToken::Base300 => self.base[2],
            ColorToken::BaseContent => self.content,
            ColorToken::Primary => self.primary,
            ColorToken::PrimaryContent => self.on_primary,
            ColorToken::Secondary => self.secondary,
            ColorToken::SecondaryContent => self.on_secondary,
            ColorToken::Accent => self.accent,
            ColorToken::AccentContent => self.on_accent,
            ColorToken::Neutral => self.neutral,
            ColorToken::NeutralContent => self.on_neutral,
            ColorToken::Info => self.info,
            ColorToken::InfoContent => self.on_info,
            ColorToken::Success => self.success,
            ColorToken::SuccessContent => self.on_success,
            ColorToken::Warning => self.warning,
            ColorToken::WarningContent => self.on_warning,
            ColorToken::Error => self.error,
            ColorToken::ErrorContent => self.on_error,
            ColorToken::Border => self.border,
            ColorToken::Muted => self.muted,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct UiTheme {
    pub palette: Palette,
    legacy: Option<Theme>,
    legacy_palette: Option<Palette>,
    background: Option<[f32; 4]>,
}
impl UiTheme {
    pub fn dark() -> Self {
        Self::from_palette(Palette::dark())
    }
    pub fn light() -> Self {
        Self::from_palette(Palette::light())
    }
    pub fn from_palette(palette: Palette) -> Self {
        Self {
            palette,
            legacy: None,
            legacy_palette: None,
            background: None,
        }
    }
    /// Preserves exact legacy default drawing, including its primary foreground.
    pub fn from_legacy(theme: &Theme) -> Self {
        let c = Colors::from_theme(theme);
        let info = [0.22, 0.60, 0.95, 1.];
        let success = [0.18, 0.70, 0.42, 1.];
        let warning = [0.95, 0.68, 0.15, 1.];
        let error = [0.90, 0.25, 0.30, 1.];
        let palette = Palette {
            name: "Volt".into(),
            base: [theme.background, c.panel, c.selected],
            content: theme.foreground,
            primary: c.accent,
            // Contrast with the primary fill itself, like `on_accent`; the old
            // window-background choice made primary labels nearly unreadable.
            on_primary: contrast(c.accent),
            secondary: info,
            on_secondary: contrast(info),
            accent: c.accent,
            on_accent: contrast(c.accent),
            neutral: c.border,
            on_neutral: c.text,
            info,
            on_info: contrast(info),
            success,
            on_success: contrast(success),
            warning,
            on_warning: contrast(warning),
            error,
            on_error: contrast(error),
            border: c.border,
            muted: c.muted,
        };
        Self {
            legacy: Some(theme.clone()),
            legacy_palette: Some(palette.clone()),
            background: None,
            palette,
        }
    }
    pub fn is_legacy(&self) -> bool {
        self.legacy.is_some() && self.legacy_palette.as_ref() == Some(&self.palette)
    }
    pub fn with_accent(mut self, color: [f32; 4]) -> Self {
        self.palette = self.palette.with_accent(color);
        self.legacy = None;
        self
    }
    /// Root window clear color, independent of semantic component surfaces.
    /// Native hosts validate its finite 0..=1 components before rendering.
    pub fn with_background_color(mut self, color: [f32; 4]) -> Self {
        self.background = Some(color);
        self
    }
    pub fn legacy_theme(&self) -> Theme {
        if self.is_legacy() {
            let mut theme = self.legacy.as_ref().unwrap().clone();
            if let Some(background) = self.background {
                theme.background = background;
            }
            return theme;
        };
        let mut t = Theme::dark();
        t.background = self.background.unwrap_or(self.palette.base[0]);
        t.foreground = self.palette.content;
        t.ansi[3] = self.palette.primary;
        t.ansi[11] = self.palette.accent;
        t
    }
    pub fn colors(&self) -> Colors {
        if self.is_legacy() {
            return Colors::from_theme(self.legacy.as_ref().unwrap());
        };
        let mut c = Colors::from_theme(&self.legacy_theme());
        c.panel = self.palette.base[1];
        c.selected = self.palette.base[2];
        c.text = self.palette.content;
        c.muted = self.palette.muted;
        c.border = self.palette.border;
        c.accent = self.palette.primary;
        c.accent_bright = self.palette.accent;
        c
    }
    pub fn resolve(&self, value: &str) -> Option<[f32; 4]> {
        if let Some((token, alpha)) = ColorToken::parse_with_alpha(value) {
            let mut color = self.palette.color(token);
            color[3] *= alpha;
            return Some(color);
        };
        let hex = value.strip_prefix('#')?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        };
        let mut out = [1.; 4];
        for (i, c) in out.iter_mut().enumerate().take(hex.len() / 2) {
            *c = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()? as f32 / 255.
        }
        Some(out)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn root_background_override_never_changes_semantic_surfaces() {
        let ui = UiTheme::light();
        let updated = ui.clone().with_background_color([0.2, 0.3, 0.4, 1.]);
        assert_eq!(updated.palette, ui.palette);
        assert_eq!(updated.resolve("@base-100"), ui.resolve("@base-100"));
        assert_eq!(updated.legacy_theme().background, [0.2, 0.3, 0.4, 1.]);
        let legacy = Theme::dark();
        let updated = UiTheme::from_legacy(&legacy).with_background_color([0.2, 0.3, 0.4, 1.]);
        let mut expected = legacy.clone();
        expected.background = [0.2, 0.3, 0.4, 1.];
        assert_eq!(updated.legacy_theme(), expected);
    }
    #[test]
    fn legacy_colors_are_exact_but_mutating_palette_uses_semantic_recipe() {
        let legacy = Theme::dark();
        let mut ui = UiTheme::from_legacy(&legacy);
        assert_eq!(ui.colors(), Colors::from_theme(&legacy));
        assert_eq!(ui.legacy_theme(), legacy);
        ui.palette.base[0] = [0.4, 0.5, 0.6, 1.];
        assert!(!ui.is_legacy());
        assert_eq!(ui.legacy_theme().background, ui.palette.base[0]);
    }
    #[test]
    fn accent_updates_on_colors_and_unknown_tokens_are_rejected() {
        let ui = UiTheme::light().with_accent([0., 0., 0., 1.]);
        assert_eq!(ui.palette.on_primary, [1.; 4]);
        assert_eq!(ui.resolve("@accent"), Some([0., 0., 0., 1.]));
        assert_eq!(ui.resolve("@missing"), None);
        assert_eq!(ui.resolve("@accent/25"), Some([0., 0., 0., 0.25]));
        assert_eq!(ColorToken::Accent.alpha(25), "@accent/25");
        for bad in ["@accent/101", "@accent/x", "@accent/", "@missing/10"] {
            assert_eq!(ui.resolve(bad), None, "{bad}");
        }
        assert_eq!(
            UiTheme::dark().with_accent([1.; 4]).palette.on_primary,
            [0., 0., 0., 1.]
        );
        let mut p = Palette::dark();
        p.error[0] = f32::NAN;
        assert!(!p.is_valid());
    }
}
