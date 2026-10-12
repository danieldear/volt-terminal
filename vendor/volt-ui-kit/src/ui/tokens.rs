//! Design-token scales in the spirit of Tailwind's defaults, so spacing, type
//! and radii stay consistent across an app. Values are logical pixels unless
//! noted. Use them anywhere a number is expected:
//!
//! ```ignore
//! vstack![..].padding(space::S6, Edge::All).gap(space::S3, Axis::Vertical)
//! text("Title").font_size(text_size::XL2).line_height(leading::TIGHT)
//! ```

/// Spacing on a 4 px grid: `S4` = 16 px (Tailwind's `p-4`).
pub mod space {
    pub const PX: f32 = 1.;
    pub const S0_5: f32 = 2.;
    pub const S1: f32 = 4.;
    pub const S1_5: f32 = 6.;
    pub const S2: f32 = 8.;
    pub const S2_5: f32 = 10.;
    pub const S3: f32 = 12.;
    pub const S4: f32 = 16.;
    pub const S5: f32 = 20.;
    pub const S6: f32 = 24.;
    pub const S8: f32 = 32.;
    pub const S10: f32 = 40.;
    pub const S12: f32 = 48.;
    pub const S16: f32 = 64.;
    pub const S20: f32 = 80.;
    pub const S24: f32 = 96.;
    pub const S32: f32 = 128.;
    /// Any step of the 4 px scale: `step(7)` = 28 px.
    pub const fn step(n: u32) -> f32 {
        n as f32 * 4.
    }
}

/// Corner radii (`rounded-*`).
pub mod radius {
    pub const NONE: f32 = 0.;
    pub const XS: f32 = 2.;
    pub const SM: f32 = 4.;
    pub const MD: f32 = 6.;
    pub const LG: f32 = 8.;
    pub const XL: f32 = 12.;
    pub const XL2: f32 = 16.;
    pub const XL3: f32 = 24.;
}

/// Font sizes (`text-*`).
pub mod text_size {
    pub const XS: f32 = 12.;
    pub const SM: f32 = 14.;
    pub const BASE: f32 = 16.;
    pub const LG: f32 = 18.;
    pub const XL: f32 = 20.;
    pub const XL2: f32 = 24.;
    pub const XL3: f32 = 30.;
    pub const XL4: f32 = 36.;
    pub const XL5: f32 = 48.;
    pub const XL6: f32 = 60.;
    pub const XL7: f32 = 72.;
    pub const XL8: f32 = 96.;
}

/// Line heights as multiples of the font size (`leading-*`).
pub mod leading {
    pub const NONE: f32 = 1.;
    pub const TIGHT: f32 = 1.25;
    pub const SNUG: f32 = 1.375;
    pub const NORMAL: f32 = 1.5;
    pub const RELAXED: f32 = 1.625;
    pub const LOOSE: f32 = 2.;
}

/// Letter spacing in em (`tracking-*`).
pub mod tracking {
    pub const TIGHTER: f32 = -0.05;
    pub const TIGHT: f32 = -0.025;
    pub const NORMAL: f32 = 0.;
    pub const WIDE: f32 = 0.025;
    pub const WIDER: f32 = 0.05;
    pub const WIDEST: f32 = 0.1;
}

/// Font weights (`font-*`).
pub mod weight {
    pub const THIN: u16 = 100;
    pub const EXTRA_LIGHT: u16 = 200;
    pub const LIGHT: u16 = 300;
    pub const NORMAL: u16 = 400;
    pub const MEDIUM: u16 = 500;
    pub const SEMIBOLD: u16 = 600;
    pub const BOLD: u16 = 700;
    pub const EXTRA_BOLD: u16 = 800;
    pub const BLACK: u16 = 900;
}

/// Elevation shadows (`shadow-*`), tuned for light and dark surfaces.
pub mod shadow {
    use crate::declarative::Shadow;
    pub fn xs() -> Shadow {
        Shadow::new().color("#0000000D").blur(2.).offset(0., 1.)
    }
    pub fn sm() -> Shadow {
        Shadow::new().color("#0000001A").blur(3.).offset(0., 1.)
    }
    pub fn md() -> Shadow {
        Shadow::new().color("#0000001A").blur(8.).offset(0., 4.)
    }
    pub fn lg() -> Shadow {
        Shadow::new().color("#0000001A").blur(16.).offset(0., 10.)
    }
    pub fn xl() -> Shadow {
        Shadow::new().color("#0000001F").blur(28.).offset(0., 20.)
    }
    pub fn xl2() -> Shadow {
        Shadow::new().color("#00000040").blur(50.).offset(0., 25.)
    }
}

/// Durations (ms) and easing curves (`duration-*`, `ease-*`).
pub mod motion {
    use crate::declarative::Easing;
    pub const FAST: u64 = 100;
    pub const BASE: u64 = 150;
    pub const MEDIUM: u64 = 200;
    pub const SLOW: u64 = 300;
    pub const SLOWER: u64 = 500;
    pub const EASE_IN: Easing = Easing::CubicBezier(0.4, 0., 1., 1.);
    pub const EASE_OUT: Easing = Easing::CubicBezier(0., 0., 0.2, 1.);
    pub const EASE_IN_OUT: Easing = Easing::CubicBezier(0.4, 0., 0.2, 1.);
    /// A soft overshoot-free "expo out" for panels and sheets.
    pub const EMPHASIZED: Easing = Easing::CubicBezier(0.16, 1., 0.3, 1.);
}

/// Breakpoint widths, matching `Breakpoint::{Sm, Md, Lg, Xl, Xxl}`.
pub mod screen {
    pub const SM: f32 = 640.;
    pub const MD: f32 = 768.;
    pub const LG: f32 = 1024.;
    pub const XL: f32 = 1280.;
    pub const XL2: f32 = 1536.;
}
