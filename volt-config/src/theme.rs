use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse `#rrggbb`. The leading `#` is optional and case is ignored;
    /// anything else (shorthand `#rgb`, alpha, names) is rejected.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim();
        let hex = s.strip_prefix('#').unwrap_or(s);
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let v = u32::from_str_radix(hex, 16).ok()?;
        Some(Self::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
    pub fn to_f32(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            1.0,
        ]
    }

    pub fn to_f32_alpha(self, alpha: f32) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            alpha.clamp(0.0, 1.0),
        ]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::rgb(0, 0, 0)
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Color::from_hex(&raw).ok_or_else(|| {
            serde::de::Error::custom(format!("expected a \"#rrggbb\" color, got {raw:?}"))
        })
    }
}

/// Built-in themes as `(config id, display name)`. These ids are reserved:
/// a theme file in the themes folder can never shadow one of them.
pub const BUILTIN_THEMES: [(&str, &str); 5] = [
    ("catppuccin", "Catppuccin Mocha"),
    ("tokyo-night", "Tokyo Night Storm"),
    ("gruvbox", "Gruvbox Dark Hard"),
    ("nord", "Nord"),
    ("dracula", "Dracula"),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub cursor: Color,
    pub cursor_text: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub ansi: [Color; 16],
}

impl Theme {
    pub fn names() -> &'static [&'static str] {
        &["catppuccin", "tokyo-night", "gruvbox", "nord", "dracula"]
    }

    /// A built-in theme by its config id, or `None` for any other id.
    pub fn builtin(id: &str) -> Option<Self> {
        Some(match id {
            "catppuccin" => Self::dark(),
            "tokyo-night" => Self::tokyo_night(),
            "gruvbox" => Self::gruvbox(),
            "nord" => Self::nord(),
            "dracula" => Self::dracula(),
            _ => return None,
        })
    }

    /// A built-in theme by id, falling back to Catppuccin for unknown ids.
    /// User theme files are resolved by `themes::ThemeRegistry`, not here.
    pub fn by_name(name: &str) -> Self {
        Self::builtin(name).unwrap_or_else(Self::dark)
    }

    /// Catppuccin Mocha
    pub fn dark() -> Self {
        Self {
            background: Color::rgb(24, 24, 37),
            foreground: Color::rgb(202, 211, 245),
            cursor: Color::rgb(202, 211, 245),
            cursor_text: Color::rgb(24, 24, 37),
            selection_bg: Color::rgb(69, 90, 120),
            selection_fg: Color::rgb(202, 211, 245),
            ansi: [
                Color::rgb(30, 30, 46),
                Color::rgb(243, 139, 168),
                Color::rgb(166, 227, 161),
                Color::rgb(249, 226, 175),
                Color::rgb(137, 180, 250),
                Color::rgb(203, 166, 247),
                Color::rgb(137, 220, 235),
                Color::rgb(166, 173, 200),
                Color::rgb(88, 91, 112),
                Color::rgb(243, 139, 168),
                Color::rgb(166, 227, 161),
                Color::rgb(249, 226, 175),
                Color::rgb(137, 180, 250),
                Color::rgb(203, 166, 247),
                Color::rgb(137, 220, 235),
                Color::rgb(186, 194, 222),
            ],
        }
    }

    /// Tokyo Night Storm
    pub fn tokyo_night() -> Self {
        Self {
            background: Color::rgb(36, 40, 59),
            foreground: Color::rgb(192, 202, 245),
            cursor: Color::rgb(187, 154, 247),
            cursor_text: Color::rgb(36, 40, 59),
            selection_bg: Color::rgb(78, 98, 144),
            selection_fg: Color::rgb(192, 202, 245),
            ansi: [
                Color::rgb(32, 32, 44),
                Color::rgb(247, 118, 142),
                Color::rgb(158, 206, 106),
                Color::rgb(224, 175, 104),
                Color::rgb(122, 162, 247),
                Color::rgb(187, 154, 247),
                Color::rgb(115, 218, 202),
                Color::rgb(169, 177, 214),
                Color::rgb(65, 72, 104),
                Color::rgb(255, 117, 127),
                Color::rgb(158, 206, 106),
                Color::rgb(255, 199, 119),
                Color::rgb(125, 175, 255),
                Color::rgb(187, 154, 247),
                Color::rgb(115, 218, 202),
                Color::rgb(192, 202, 245),
            ],
        }
    }

    /// Gruvbox Dark Hard
    pub fn gruvbox() -> Self {
        Self {
            background: Color::rgb(29, 32, 33),
            foreground: Color::rgb(235, 219, 178),
            cursor: Color::rgb(235, 219, 178),
            cursor_text: Color::rgb(29, 32, 33),
            selection_bg: Color::rgb(80, 73, 69),
            selection_fg: Color::rgb(235, 219, 178),
            ansi: [
                Color::rgb(40, 40, 40),
                Color::rgb(204, 36, 29),
                Color::rgb(152, 151, 26),
                Color::rgb(215, 153, 33),
                Color::rgb(69, 133, 136),
                Color::rgb(177, 98, 134),
                Color::rgb(104, 157, 106),
                Color::rgb(168, 153, 132),
                Color::rgb(146, 131, 116),
                Color::rgb(251, 73, 52),
                Color::rgb(184, 187, 38),
                Color::rgb(250, 189, 47),
                Color::rgb(131, 165, 152),
                Color::rgb(211, 134, 155),
                Color::rgb(142, 192, 124),
                Color::rgb(235, 219, 178),
            ],
        }
    }

    /// Nord
    pub fn nord() -> Self {
        Self {
            background: Color::rgb(46, 52, 64),
            foreground: Color::rgb(216, 222, 233),
            cursor: Color::rgb(216, 222, 233),
            cursor_text: Color::rgb(46, 52, 64),
            selection_bg: Color::rgb(76, 86, 106),
            selection_fg: Color::rgb(216, 222, 233),
            ansi: [
                Color::rgb(59, 66, 82),
                Color::rgb(191, 97, 106),
                Color::rgb(163, 190, 140),
                Color::rgb(235, 203, 139),
                Color::rgb(129, 161, 193),
                Color::rgb(180, 142, 173),
                Color::rgb(136, 192, 208),
                Color::rgb(229, 233, 240),
                Color::rgb(76, 86, 106),
                Color::rgb(191, 97, 106),
                Color::rgb(163, 190, 140),
                Color::rgb(235, 203, 139),
                Color::rgb(129, 161, 193),
                Color::rgb(180, 142, 173),
                Color::rgb(143, 188, 187),
                Color::rgb(236, 239, 244),
            ],
        }
    }

    /// Dracula
    pub fn dracula() -> Self {
        Self {
            background: Color::rgb(40, 42, 54),
            foreground: Color::rgb(248, 248, 242),
            cursor: Color::rgb(248, 248, 242),
            cursor_text: Color::rgb(40, 42, 54),
            selection_bg: Color::rgb(68, 71, 90),
            selection_fg: Color::rgb(248, 248, 242),
            ansi: [
                Color::rgb(33, 34, 44),
                Color::rgb(255, 85, 85),
                Color::rgb(80, 250, 123),
                Color::rgb(241, 250, 140),
                Color::rgb(189, 147, 249),
                Color::rgb(255, 121, 198),
                Color::rgb(139, 233, 253),
                Color::rgb(191, 191, 191),
                Color::rgb(85, 85, 85),
                Color::rgb(255, 110, 110),
                Color::rgb(90, 255, 148),
                Color::rgb(255, 255, 153),
                Color::rgb(202, 169, 255),
                Color::rgb(255, 153, 215),
                Color::rgb(154, 237, 254),
                Color::rgb(255, 255, 255),
            ],
        }
    }

    pub fn ansi_color(&self, index: u8) -> Color {
        match index {
            0..=15 => self.ansi[index as usize],
            16..=231 => {
                let i = index - 16;
                let level = |v: u8| if v == 0 { 0u8 } else { 55 + v * 40 };
                let b = level(i % 6);
                let g = level((i / 6) % 6);
                let r = level(i / 36);
                Color::rgb(r, g, b)
            }
            232..=255 => {
                let v = 8 + (index - 232) * 10;
                Color::rgb(v, v, v)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_and_rejects_malformed_input() {
        let c = Color::from_hex("#1A2b3C").unwrap();
        assert_eq!(c, Color::rgb(0x1a, 0x2b, 0x3c));
        assert_eq!(c.to_hex(), "#1a2b3c");
        assert_eq!(Color::from_hex("  ff8000 "), Some(Color::rgb(255, 128, 0)));
        for bad in [
            "",
            "#",
            "#fff",
            "#12345",
            "#1234567",
            "#12345g",
            "red",
            "#ff80001",
            "＃ff8000",
        ] {
            assert_eq!(Color::from_hex(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn every_builtin_id_resolves_and_unknown_ids_do_not() {
        for (id, _) in BUILTIN_THEMES {
            assert!(Theme::builtin(id).is_some(), "{id}");
        }
        assert!(Theme::builtin("midnight-clay").is_none());
        assert_eq!(Theme::by_name("no-such-theme"), Theme::dark());
    }
}
