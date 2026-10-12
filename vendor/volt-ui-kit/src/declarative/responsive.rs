use super::DocumentError;
use serde::Deserialize;

/// Stack arrangement, independent of content placement (`Flow`).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Horizontal,
    #[default]
    Vertical,
}
impl Direction {
    pub(crate) fn axis(self) -> usize {
        usize::from(self == Self::Vertical)
    }
}

/// Inclusive minimum viewport width in logical pixels, not framebuffer pixels.
/// Named defaults are conveniences; use `MinWidth` for application thresholds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Breakpoint {
    Sm,
    Md,
    Lg,
    Xl,
    Xxl,
    MinWidth(f32),
}
impl Breakpoint {
    pub fn min_width(self) -> f32 {
        match self {
            Self::Sm => 640.,
            Self::Md => 768.,
            Self::Lg => 1024.,
            Self::Xl => 1280.,
            Self::Xxl => 1536.,
            Self::MinWidth(width) => width,
        }
    }
}

/// A base value plus mobile-first minimum-width overrides.
/// Distinct thresholds are order-independent; equal thresholds use the last rule.
/// UI build validates every stored value and threshold, including inactive rules.
#[derive(Clone, Debug)]
pub struct Responsive<T> {
    base: T,
    rules: Vec<(Breakpoint, T)>,
}
pub fn responsive<T>(base: T) -> Responsive<T> {
    Responsive {
        base,
        rules: vec![],
    }
}
impl<T> From<T> for Responsive<T> {
    fn from(value: T) -> Self {
        responsive(value)
    }
}
impl<T> Responsive<T> {
    /// Override at this logical viewport width and above. At most 32 stored rules.
    pub fn at(mut self, breakpoint: Breakpoint, value: T) -> Self {
        self.rules.push((breakpoint, value));
        self
    }
    pub(crate) fn resolve(&self, logical_width: f32) -> &T {
        let mut selected = &self.base;
        let mut highest = -1.;
        for (breakpoint, value) in &self.rules {
            let threshold = breakpoint.min_width();
            if logical_width >= threshold && threshold >= highest {
                highest = threshold;
                selected = value;
            }
        }
        selected
    }
    pub(crate) fn validate(
        &self,
        path: &str,
        valid: impl Fn(&T) -> bool,
    ) -> Result<(), DocumentError> {
        if self.rules.len() > 32 {
            return Err(DocumentError(format!(
                "{path}: exceeds 32 responsive rules"
            )));
        }
        if !valid(&self.base)
            || self.rules.iter().any(|(bp, value)| {
                let w = bp.min_width();
                !w.is_finite() || !(0. ..=65_536.).contains(&w) || !valid(value)
            })
        {
            return Err(DocumentError(format!(
                "{path}: invalid responsive value or breakpoint"
            )));
        }
        Ok(())
    }
}

/// Rust-only property rules. JSON retains its scalar schema and precedence.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResponsiveLayout {
    pub direction: Option<Responsive<Direction>>,
    pub padding: [Option<Responsive<f32>>; 4],
    pub gap: [Option<Responsive<f32>>; 2],
}
impl ResponsiveLayout {
    pub fn validate(&self, path: &str) -> Result<(), DocumentError> {
        if let Some(direction) = &self.direction {
            direction.validate(&format!("{path}.direction"), |_| true)?;
        }
        for (name, rule) in [
            "padding_top",
            "padding_right",
            "padding_bottom",
            "padding_left",
        ]
        .into_iter()
        .zip(&self.padding)
        .chain(
            ["gap_horizontal", "gap_vertical"]
                .into_iter()
                .zip(&self.gap),
        ) {
            if let Some(rule) = rule {
                rule.validate(&format!("{path}.{name}"), |v| {
                    v.is_finite() && (0. ..=16_384.).contains(v)
                })?;
            }
        }
        Ok(())
    }
}
