//! Optional validated JSON documents, typed Rust callbacks and shared layout/input.
//! This module owns no filesystem, window, GPU device or event loop.
use std::collections::BTreeMap;

/// Event signatures accepted by the initial component contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Activate,
    TextChanged,
    ToggleChanged,
    ValueChanged,
    ContextRequested,
    WindowChanged,
    FilesDropped,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Activate,
    TextChanged(String),
    ToggleChanged(bool),
    ValueChanged {
        bits: u32,
    },
    ContextRequested {
        x_bits: u32,
        y_bits: u32,
    },
    WindowChanged {
        start: usize,
        end: usize,
    },
    /// Files dragged from the system onto a drop zone (never empty).
    FilesDropped(Vec<std::path::PathBuf>),
}
impl Event {
    pub fn kind(&self) -> EventKind {
        match self {
            Self::Activate => EventKind::Activate,
            Self::TextChanged(_) => EventKind::TextChanged,
            Self::ToggleChanged(_) => EventKind::ToggleChanged,
            Self::ValueChanged { .. } => EventKind::ValueChanged,
            Self::ContextRequested { .. } => EventKind::ContextRequested,
            Self::WindowChanged { .. } => EventKind::WindowChanged,
            Self::FilesDropped(_) => EventKind::FilesDropped,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HandlerError {
    InvalidName,
    InvalidValue,
    Duplicate(String),
    Missing(String),
    WrongKind {
        name: String,
        expected: EventKind,
        actual: EventKind,
    },
}
impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "UI handler error: {self:?}")
    }
}
impl std::error::Error for HandlerError {}

type Callback<'a, C> = Box<dyn FnMut(&mut C, Event) + 'a>;
/// Host-owned registry. Context is supplied at dispatch, not captured globally.
/// Callbacks execute synchronously; enqueue expensive/async work in the host.
/// Registration rejects duplicates. Rebuild the registry for deliberate replacement.
pub struct Handlers<'a, C> {
    entries: BTreeMap<String, (EventKind, Callback<'a, C>)>,
}
impl<C> Default for Handlers<'_, C> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}
impl<'a, C> Handlers<'a, C> {
    fn insert(
        &mut self,
        name: &str,
        kind: EventKind,
        callback: Callback<'a, C>,
    ) -> Result<(), HandlerError> {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(HandlerError::InvalidName);
        }
        if self.entries.contains_key(name) {
            return Err(HandlerError::Duplicate(name.into()));
        }
        self.entries.insert(name.into(), (kind, callback));
        Ok(())
    }
    pub fn on_activate(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::Activate,
            Box::new(move |ctx, event| {
                if let Event::Activate = event {
                    f(ctx);
                }
            }),
        )
    }
    pub fn on_text_changed(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, String) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::TextChanged,
            Box::new(move |ctx, event| {
                if let Event::TextChanged(value) = event {
                    f(ctx, value);
                }
            }),
        )
    }
    pub fn on_toggle_changed(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, bool) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::ToggleChanged,
            Box::new(move |ctx, event| {
                if let Event::ToggleChanged(value) = event {
                    f(ctx, value);
                }
            }),
        )
    }
    /// Finite numeric events preserve Event equality through their IEEE bit payload.
    pub fn on_value_changed(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, f32) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::ValueChanged,
            Box::new(move |ctx, event| {
                if let Event::ValueChanged { bits } = event {
                    f(ctx, f32::from_bits(bits));
                }
            }),
        )
    }
    pub fn on_context_requested(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, [f32; 2]) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::ContextRequested,
            Box::new(move |ctx, event| {
                if let Event::ContextRequested { x_bits, y_bits } = event {
                    f(ctx, [f32::from_bits(x_bits), f32::from_bits(y_bits)]);
                }
            }),
        )
    }
    pub fn on_window_changed(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, std::ops::Range<usize>) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::WindowChanged,
            Box::new(move |ctx, event| {
                if let Event::WindowChanged { start, end } = event {
                    f(ctx, start..end);
                }
            }),
        )
    }
    pub fn on_files_dropped(
        &mut self,
        name: &str,
        mut f: impl FnMut(&mut C, Vec<std::path::PathBuf>) + 'a,
    ) -> Result<(), HandlerError> {
        self.insert(
            name,
            EventKind::FilesDropped,
            Box::new(move |ctx, event| {
                if let Event::FilesDropped(paths) = event {
                    f(ctx, paths);
                }
            }),
        )
    }
    /// Validate a document connection without running application code.
    pub fn validate(&self, name: &str, kind: EventKind) -> Result<(), HandlerError> {
        let (expected, _) = self
            .entries
            .get(name)
            .ok_or_else(|| HandlerError::Missing(name.into()))?;
        if *expected != kind {
            return Err(HandlerError::WrongKind {
                name: name.into(),
                expected: *expected,
                actual: kind,
            });
        }
        Ok(())
    }
    pub fn dispatch(
        &mut self,
        name: &str,
        event: Event,
        context: &mut C,
    ) -> Result<(), HandlerError> {
        self.validate(name, event.kind())?;
        if matches!(event, Event::ValueChanged { bits } if !f32::from_bits(bits).is_finite()) {
            return Err(HandlerError::InvalidValue);
        }
        if matches!(event, Event::ContextRequested { x_bits, y_bits } if !f32::from_bits(x_bits).is_finite() || !f32::from_bits(y_bits).is_finite())
            || matches!(event, Event::WindowChanged { start, end } if start > end)
            || matches!(&event, Event::FilesDropped(paths) if paths.is_empty())
        {
            return Err(HandlerError::InvalidValue);
        }
        let (_, callback) = self.entries.get_mut(name).expect("validated handler");
        callback(context, event);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_callbacks_and_function_handlers() {
        fn save(count: &mut usize) {
            *count += 1;
        }
        let mut h = Handlers::default();
        h.on_activate("save", save).unwrap();
        let mut count = 0;
        h.dispatch("save", Event::Activate, &mut count).unwrap();
        assert_eq!(count, 1);
        let mut h = Handlers::default();
        h.on_text_changed("rename", |s: &mut String, value| *s = value)
            .unwrap();
        let mut value = String::new();
        h.dispatch("rename", Event::TextChanged("日本語".into()), &mut value)
            .unwrap();
        assert_eq!(value, "日本語");
        let mut h = Handlers::default();
        h.on_toggle_changed("toggle", |s: &mut bool, value| *s = value)
            .unwrap();
        let mut checked = false;
        h.dispatch("toggle", Event::ToggleChanged(true), &mut checked)
            .unwrap();
        assert!(checked);
    }
    #[test]
    fn invalid_dispatch_never_calls_handler_and_duplicates_preserve_original() {
        let mut h = Handlers::default();
        assert_eq!(
            h.on_activate("bad name", |_: &mut usize| {}),
            Err(HandlerError::InvalidName)
        );
        h.on_activate("save", |c| *c += 1).unwrap();
        assert_eq!(
            h.on_activate("save", |c| *c += 100),
            Err(HandlerError::Duplicate("save".into()))
        );
        let mut count = 0;
        assert!(h
            .dispatch("save", Event::ToggleChanged(true), &mut count)
            .is_err());
        assert_eq!(
            h.dispatch("missing", Event::Activate, &mut count),
            Err(HandlerError::Missing("missing".into()))
        );
        assert_eq!(count, 0);
        h.dispatch("save", Event::Activate, &mut count).unwrap();
        assert_eq!(count, 1);
    }
    #[test]
    fn supports_borrowed_closure_state() {
        let mut calls = 0;
        {
            let mut h = Handlers::default();
            h.on_activate("save", |_: &mut ()| calls += 1).unwrap();
            h.dispatch("save", Event::Activate, &mut ()).unwrap();
        }
        assert_eq!(calls, 1);
    }
}

pub(crate) mod document;
pub(crate) mod effects;
pub(crate) mod extras;
pub use extras::{
    BorderStyle, Cursor, Gradient, GridColumns, Justify, Sides, SplitHandle, TextAlign,
};
mod responsive;
pub use effects::{Ambient, Animation, Easing, Glow, Interaction, Shadow, Transition, VisualStyle};
mod motion;
mod runtime;
pub use document::{
    Align, Bindings, Document, DocumentError, Flow, Kind, Node, Rounding, Scroll, Scrollbars, Tone,
    Value,
};
pub use responsive::{responsive, Breakpoint, Direction, Responsive};
pub use runtime::{
    ButtonState, DocumentPainter, InputOutcome, LayoutBox, PaintBatch, Runtime, SelectionKey,
    TextMetrics, TextStyle,
};

#[cfg(feature = "rust-ui")]
pub use runtime::accessibility::{AccessNode, AccessRole, AccessTableInfo, AccessValue};
#[cfg(feature = "rust-ui")]
pub use runtime::editing::EditKey;
#[cfg(feature = "rust-ui")]
pub use runtime::value_controls::ValueKey;
