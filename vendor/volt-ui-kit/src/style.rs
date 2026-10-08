//! Normalized RGBA theme tokens. Hosts can map any theme format onto these.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub background: [f32; 4],
    pub foreground: [f32; 4],
    pub ansi: [[f32; 4]; 16],
}
impl Theme {
    /// Amber/dark reference theme; applications may supply their own palette.
    pub fn dark() -> Self {
        let mut ansi = [[0.65, 0.67, 0.69, 1.]; 16];
        ansi[1] = [0.94, 0.47, 0.49, 1.];
        ansi[2] = [0.48, 0.79, 0.59, 1.];
        ansi[3] = [0.87, 0.70, 0.40, 1.];
        ansi[11] = [1., 0.81, 0.49, 1.];
        Self {
            background: [0.09, 0.10, 0.10, 1.],
            foreground: [0.90, 0.85, 0.72, 1.],
            ansi,
        }
    }
    pub fn light() -> Self {
        let mut t = Self::dark();
        t.background = [0.96, 0.95, 0.92, 1.];
        t.foreground = [0.15, 0.16, 0.18, 1.];
        t.ansi[3] = [0.57, 0.36, 0.06, 1.];
        t.ansi[11] = [0.72, 0.45, 0.08, 1.];
        t
    }
}
/// Panel colors derived from the active application theme: surfaces are the
/// background nudged toward the foreground, and the theme's yellow is the
/// accent (amber in Gruvbox, soft gold in Nord, and so on).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Colors {
    pub shadow: [f32; 4],
    pub border: [f32; 4],
    pub panel: [f32; 4],
    pub line: [f32; 4],
    pub selected: [f32; 4],
    pub text: [f32; 4],
    pub muted: [f32; 4],
    pub quiet: [f32; 4],
    pub accent: [f32; 4],
    pub accent_bright: [f32; 4],
    pub accent_dim: [f32; 4],
}

impl Colors {
    pub fn from_theme(theme: &Theme) -> Self {
        let bg = theme.background;
        let fg = theme.foreground;
        let mix = |t: f32| {
            [
                bg[0] + (fg[0] - bg[0]) * t,
                bg[1] + (fg[1] - bg[1]) * t,
                bg[2] + (fg[2] - bg[2]) * t,
                1.,
            ]
        };
        let accent = theme.ansi[3];
        Self {
            shadow: [0., 0., 0., 0.45],
            border: mix(0.22),
            panel: mix(0.04),
            line: mix(0.11),
            selected: mix(0.10),
            text: fg,
            muted: mix(0.66),
            quiet: mix(0.48),
            accent,
            accent_bright: theme.ansi[11],
            accent_dim: [accent[0], accent[1], accent[2], 0.18],
        }
    }
}
