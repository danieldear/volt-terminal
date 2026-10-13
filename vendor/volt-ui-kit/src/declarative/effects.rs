//! Typed, opt-in appearance and time-based motion. Values use logical pixels.
use super::document::parse_color;
use super::DocumentError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interaction {
    Hover,
    Pressed,
    Focus,
}
impl Interaction {
    #[cfg(feature = "rust-ui")]
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Hover => 0,
            Self::Pressed => 1,
            Self::Focus => 2,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Easing {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    /// CSS `cubic-bezier(x1, y1, x2, y2)`; x values are clamped to 0..=1.
    CubicBezier(f32, f32, f32, f32),
}
/// Solve a CSS cubic-bezier timing curve: find the parameter whose x is `t`,
/// then return its y.
fn cubic_bezier(t: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let (x1, x2) = (x1.clamp(0., 1.), x2.clamp(0., 1.));
    let curve = |u: f32, a: f32, b: f32| {
        let v = 1. - u;
        3. * v * v * u * a + 3. * v * u * u * b + u * u * u
    };
    let (mut lo, mut hi) = (0_f32, 1_f32);
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if curve(mid, x1, x2) < t {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    curve((lo + hi) * 0.5, y1, y2)
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Animation {
    pub(crate) duration: u64,
    pub(crate) easing: Easing,
    pub(crate) delay: u64,
}
impl Animation {
    pub fn linear() -> Self {
        Self {
            duration: 150,
            easing: Easing::Linear,
            delay: 0,
        }
    }
    pub fn ease_in() -> Self {
        Self {
            easing: Easing::EaseIn,
            ..Self::linear()
        }
    }
    /// Any curve, e.g. `Easing::CubicBezier(0.16, 1., 0.3, 1.)`.
    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
    /// Wait before starting; stagger list items by giving each a larger delay.
    pub fn delay_ms(mut self, milliseconds: u64) -> Self {
        self.delay = milliseconds;
        self
    }
    pub fn ease_out() -> Self {
        Self {
            easing: Easing::EaseOut,
            ..Self::linear()
        }
    }
    pub fn ease_in_out() -> Self {
        Self {
            easing: Easing::EaseInOut,
            ..Self::linear()
        }
    }
    pub fn duration_ms(mut self, milliseconds: u64) -> Self {
        self.duration = milliseconds;
        self
    }
    pub(crate) fn progress(self, elapsed: f32) -> f32 {
        let elapsed = elapsed - self.delay as f32;
        let t = if self.duration == 0 {
            if elapsed >= 0. {
                1.
            } else {
                0.
            }
        } else {
            (elapsed / self.duration as f32).clamp(0., 1.)
        };
        match self.easing {
            Easing::Linear => t,
            Easing::EaseIn => t * t * t,
            Easing::EaseOut => 1. - (1. - t).powi(3),
            Easing::EaseInOut => t * t * (3. - 2. * t),
            Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(t, x1, y1, x2, y2),
        }
    }
    pub(crate) fn valid(&self) -> bool {
        self.duration <= 60_000
            && self.delay <= 60_000
            && match self.easing {
                Easing::CubicBezier(a, b, c, d) => [a, b, c, d].iter().all(|v| v.is_finite()),
                _ => true,
            }
    }
}
/// Animate a keyed subtree in and out: a fade, optionally sliding from an
/// offset. Exiting nodes are paint-only, never interactive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transition(pub(crate) Animation, pub(crate) [f32; 2]);
impl Transition {
    pub fn fade() -> Self {
        Self(Animation::linear(), [0.; 2])
    }
    /// Fade in while sliding from (`x`, `y`) logical pixels to place; exit
    /// reverses it. `slide(0., 12.)` rises into view.
    pub fn slide(x: f32, y: f32) -> Self {
        Self(Animation::ease_out().duration_ms(220), [x, y])
    }
    pub fn duration_ms(mut self, milliseconds: u64) -> Self {
        self.0.duration = milliseconds;
        self
    }
    pub fn easing(mut self, easing: Easing) -> Self {
        self.0.easing = easing;
        self
    }
    pub fn delay_ms(mut self, milliseconds: u64) -> Self {
        self.0.delay = milliseconds;
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    pub(crate) color: String,
    pub(crate) blur: f32,
    pub(crate) spread: f32,
    pub(crate) offset: [f32; 2],
}
impl Default for Shadow {
    fn default() -> Self {
        Self {
            color: "#00000040".into(),
            blur: 8.,
            spread: 0.,
            offset: [0., 2.],
        }
    }
}
impl Shadow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }
    pub fn blur(mut self, radius: f32) -> Self {
        self.blur = radius;
        self
    }
    pub fn spread(mut self, pixels: f32) -> Self {
        self.spread = pixels;
        self
    }
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = [x, y];
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Glow(pub(crate) Shadow);
impl Default for Glow {
    fn default() -> Self {
        Self(Shadow::new().color("#60a5fa80").offset(0., 0.))
    }
}
impl Glow {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.0.color = color.into();
        self
    }
    pub fn blur(mut self, radius: f32) -> Self {
        self.0.blur = radius;
        self
    }
    pub fn spread(mut self, pixels: f32) -> Self {
        self.0.spread = pixels;
        self
    }
}
/// A declarative interaction patch; its closure executes once while building.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VisualStyle {
    pub(crate) color: Option<String>,
    pub(crate) background: Option<String>,
    pub(crate) border_color: Option<String>,
    pub(crate) border_width: Option<f32>,
    pub(crate) opacity: Option<f32>,
    pub(crate) shadow: Option<Shadow>,
    pub(crate) glow: Option<Glow>,
    pub(crate) offset: Option<[f32; 2]>,
}
impl VisualStyle {
    /// Translate the element (logical px) in this state, e.g. a hover lift
    /// `offset(0., -2.)`; animates with the state's animation.
    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Some([x, y]);
        self
    }
    pub fn color(mut self, value: impl Into<String>) -> Self {
        self.color = Some(value.into());
        self
    }
    pub fn background(mut self, value: impl Into<String>) -> Self {
        self.background = Some(value.into());
        self
    }
    pub fn border_color(mut self, value: impl Into<String>) -> Self {
        self.border_color = Some(value.into());
        self
    }
    pub fn border_width(mut self, value: f32) -> Self {
        self.border_width = Some(value);
        self
    }
    pub fn opacity(mut self, value: f32) -> Self {
        self.opacity = Some(value);
        self
    }
    pub fn shadow(mut self, value: Shadow) -> Self {
        self.shadow = Some(value);
        self
    }
    pub fn glow(mut self, value: Glow) -> Self {
        self.glow = Some(value);
        self
    }
}
/// Continuous, decorative paint-time motion: a subtree that drifts on an
/// ellipse, or a canvas that spins about its centre. It never relayouts,
/// recomposes or moves hit targets, and stops under reduced motion.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub(crate) kind: AmbientKind,
    pub(crate) period_ms: u32,
    pub(crate) phase: f32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum AmbientKind {
    Float([f32; 2]),
    Spin,
    /// Opacity breathes between this minimum and 1.
    Pulse(f32),
}
impl Ambient {
    /// Drift up to `x`/`y` logical pixels either side of the laid-out place.
    pub fn float(x: f32, y: f32) -> Self {
        Self {
            kind: AmbientKind::Float([x, y]),
            period_ms: 8000,
            phase: 0.,
        }
    }
    /// Rotate a canvas's drawing once per period (other content is unrotated).
    pub fn spin() -> Self {
        Self {
            kind: AmbientKind::Spin,
            period_ms: 30_000,
            phase: 0.,
        }
    }
    /// Breathe the subtree's opacity between `min` and 1 (skeleton loaders).
    pub fn pulse(min: f32) -> Self {
        Self {
            kind: AmbientKind::Pulse(min),
            period_ms: 1600,
            phase: 0.,
        }
    }
    pub fn period_ms(mut self, milliseconds: u32) -> Self {
        self.period_ms = milliseconds;
        self
    }
    /// Starting point in the cycle, 0..1, so siblings do not move in step.
    pub fn phase(mut self, phase: f32) -> Self {
        self.phase = phase;
        self
    }
    pub(crate) fn valid(&self) -> bool {
        (200..=600_000).contains(&self.period_ms)
            && self.phase.is_finite()
            && match self.kind {
                AmbientKind::Float(a) => a.iter().all(|v| v.is_finite() && v.abs() <= 256.),
                AmbientKind::Spin => true,
                AmbientKind::Pulse(min) => min.is_finite() && (0. ..=1.).contains(&min),
            }
    }
    /// Cycle position in turns at `now`.
    pub(crate) fn turns(&self, now: std::time::Duration) -> f32 {
        let cycle = (now.as_millis() % u128::from(self.period_ms)) as f32 / self.period_ms as f32;
        (cycle + self.phase).rem_euclid(1.)
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Effects {
    pub scroll_passthrough: bool,
    pub base: VisualStyle,
    pub states: [VisualStyle; 3],
    /// Applied while the element is disabled.
    pub disabled: VisualStyle,
    pub animations: [Option<Animation>; 3],
    pub animation: Option<Animation>,
    pub transition: Option<Transition>,
    pub ambient: Option<Ambient>,
    pub drag_region: bool,
}
impl Effects {
    pub fn validate(&self, path: &str) -> Result<(), DocumentError> {
        let invalid = || DocumentError(format!("{path}: invalid effect value"));
        if self.ambient.is_some_and(|a| !a.valid()) {
            return Err(invalid());
        }
        for style in std::iter::once(&self.base)
            .chain(&self.states)
            .chain(std::iter::once(&self.disabled))
        {
            for color in [&style.color, &style.background, &style.border_color]
                .into_iter()
                .flatten()
            {
                if parse_color(color).is_none() && crate::ColorToken::parse(color).is_none() {
                    return Err(invalid());
                }
            }
            if style
                .opacity
                .is_some_and(|v| !v.is_finite() || !(0. ..=1.).contains(&v))
                || style
                    .border_width
                    .is_some_and(|v| !v.is_finite() || !(0. ..=16.).contains(&v))
            {
                return Err(invalid());
            }
            for shadow in style.shadow.iter().chain(style.glow.iter().map(|v| &v.0)) {
                if (parse_color(&shadow.color).is_none()
                    && crate::ColorToken::parse(&shadow.color).is_none())
                    || !shadow.blur.is_finite()
                    || !(0. ..=64.).contains(&shadow.blur)
                    || !shadow.spread.is_finite()
                    || !(0. ..=32.).contains(&shadow.spread)
                    || shadow
                        .offset
                        .iter()
                        .any(|v| !v.is_finite() || !(-128. ..=128.).contains(v))
                {
                    return Err(invalid());
                }
            }
        }
        for a in self
            .animations
            .iter()
            .flatten()
            .copied()
            .chain(self.animation)
            .chain(self.transition.map(|v| v.0))
        {
            if a.duration > 10_000 || !a.valid() {
                return Err(invalid());
            }
        }
        let offset_ok = |o: &[f32; 2]| o.iter().all(|v| v.is_finite() && v.abs() <= 512.);
        if std::iter::once(&self.base)
            .chain(&self.states)
            .chain(std::iter::once(&self.disabled))
            .any(|s| s.offset.as_ref().is_some_and(|o| !offset_ok(o)))
            || self.transition.is_some_and(|t| !offset_ok(&t.1))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
