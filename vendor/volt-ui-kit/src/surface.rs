//! Optional Glass-inspired material tokens, independent of GPU or browser state.
//! Real backdrop blur must be supplied by a host compositor. The normal toolkit
//! path paints an opaque fallback, including when transparency is reduced.
use crate::{Colors, Theme};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tint {
    Light,
    #[default]
    Dark,
}
/// Host-fed brightness hysteresis. No sampling, polling or redraws are started.
#[derive(Clone, Copy, Debug)]
pub struct TintPicker {
    tint: Tint,
}
impl TintPicker {
    pub fn new(tint: Tint) -> Self {
        Self { tint }
    }
    pub fn tint(self) -> Tint {
        self.tint
    }
    /// Ignore invalid samples; retain the previous tint in the 0.45..=0.55 band.
    pub fn update(&mut self, lightness: f32) -> bool {
        if !lightness.is_finite() || !(0. ..=1.).contains(&lightness) {
            return false;
        }
        let next = if lightness < 0.45 {
            Tint::Dark
        } else if lightness > 0.55 {
            Tint::Light
        } else {
            self.tint
        };
        let changed = next != self.tint;
        self.tint = next;
        changed
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceStyle {
    pub fill: [f32; 4],
    /// Fully opaque fill when no backdrop compositor is available.
    pub fallback: [f32; 4],
    pub border: [f32; 4],
    pub highlight: [f32; 4],
    pub text: [f32; 4],
    pub muted: [f32; 4],
    pub shadow_alpha: f32,
    /// Logical pixels; the host scales once for a physical-pixel request.
    pub blur_radius: f32,
    pub saturation: f32,
}
impl SurfaceStyle {
    pub fn from_theme(theme: &Theme) -> Self {
        let c = Colors::from_theme(theme);
        Self {
            fill: c.panel,
            fallback: c.panel,
            border: c.border,
            highlight: c.line,
            text: c.text,
            muted: c.muted,
            shadow_alpha: 0.18,
            blur_radius: 0.,
            saturation: 1.,
        }
    }
    pub fn glass(tint: Tint) -> Self {
        let (fill, fallback, text, muted, border, highlight) = match tint {
            Tint::Light => (
                [1., 1., 1., 0.52],
                [0.96, 0.96, 0.97, 1.],
                [0.086, 0.086, 0.102, 1.],
                [0.42, 0.42, 0.45, 1.],
                0.70,
                0.90,
            ),
            Tint::Dark => (
                [0.157, 0.157, 0.180, 0.36],
                [0.157, 0.157, 0.180, 1.],
                [0.957, 0.957, 0.965, 1.],
                [0.651, 0.651, 0.69, 1.],
                0.18,
                0.28,
            ),
        };
        Self {
            fill,
            fallback,
            text,
            muted,
            border: [1., 1., 1., border],
            highlight: [1., 1., 1., highlight],
            shadow_alpha: 0.16,
            blur_radius: 26.,
            saturation: 1.8,
        }
    }
    pub fn reduced_transparency(mut self) -> Self {
        self.fill = self.fallback;
        self.blur_radius = 0.;
        self.saturation = 1.;
        self
    }
    pub(crate) fn valid(self) -> bool {
        [
            self.fill,
            self.fallback,
            self.border,
            self.highlight,
            self.text,
            self.muted,
        ]
        .into_iter()
        .flatten()
        .all(|c| c.is_finite() && (0. ..=1.).contains(&c))
            && self.fallback[3] == 1.
            && self.shadow_alpha.is_finite()
            && (0. ..=1.).contains(&self.shadow_alpha)
            && self.blur_radius.is_finite()
            && self.blur_radius >= 0.
            && self.saturation.is_finite()
            && self.saturation >= 0.
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hysteresis_does_not_flicker_or_accept_bad_samples() {
        let mut p = TintPicker::new(Tint::Light);
        assert!(p.update(0.2));
        for sample in [0.45, 0.5, 0.55, f32::NAN, f32::INFINITY, -1., 2.] {
            assert!(!p.update(sample));
            assert_eq!(p.tint(), Tint::Dark);
        }
        assert!(p.update(0.8));
        assert!(!p.update(0.8));
    }
    #[test]
    fn opaque_fallback_and_reduced_transparency() {
        for tint in [Tint::Light, Tint::Dark] {
            let style = SurfaceStyle::glass(tint);
            assert!(style.valid());
            assert!(style.fill[3] < 1.);
            assert_eq!(style.fallback[3], 1.);
            let reduced = style.reduced_transparency();
            assert_eq!(reduced.fill[3], 1.);
            assert_eq!(reduced.blur_radius, 0.);
            assert_eq!(reduced.saturation, 1.);
        }
    }
}
