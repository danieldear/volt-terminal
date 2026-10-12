//! Determinate progress and opt-in continuously animated loading indicators.
use super::*;
use crate::declarative::document::{ProgressKind, ProgressProps};
pub struct Progress<C>(pub(super) Element<C>);
fn make<C>(value: Option<f32>, kind: ProgressKind) -> Progress<C> {
    let mut e = Element::new(Kind::Panel);
    let spinner = kind == ProgressKind::Spinner;
    e.node.width = Some(if spinner { 20. } else { 180. });
    e.node.height = Some(if spinner { 20. } else { 8. });
    if !spinner {
        e.node.style.background = Some(ColorToken::Base300.into());
        e.node.style.radius = Some(Rounding::Radius(4.));
    }
    e.node.progress = Some(ProgressProps {
        value,
        kind,
        period_ms: 1000,
    });
    Progress(e)
}
pub fn progress<C>(value: f32) -> Progress<C> {
    make(Some(value), ProgressKind::Bar)
}
pub fn progress_indeterminate<C>() -> Progress<C> {
    make(None, ProgressKind::Bar)
}
pub fn spinner<C>() -> Progress<C> {
    make(None, ProgressKind::Spinner)
}
/// In-window status row, not an OS busy indicator. Hide it when work completes.
pub fn loading<C>(label: impl Into<String>) -> Container<C> {
    row([spinner().into(), text(label).into()])
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
}
impl<C> Progress<C> {
    pub fn period_ms(mut self, value: u32) -> Self {
        self.0.node.progress.as_mut().unwrap().period_ms = value;
        self
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.0.node.enabled = value;
        self
    }
}
impl<C> From<Progress<C>> for Element<C> {
    fn from(value: Progress<C>) -> Self {
        value.0
    }
}
