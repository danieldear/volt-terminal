//! Controlled in-window overlays. These are not operating-system dialogs.
use super::*;
use crate::declarative::document::{Overlay, OverlayKind};

/// A modal card rendered above the window's ordinary content.
/// Put dialogs in the tree after the content they cover; their children are only
/// mounted while `open` is true. The application owns open/closed state.
pub struct Dialog<C>(pub(super) Element<C>, DialogOptions<C>);
struct DialogOptions<C> {
    open: bool,
    title: Option<String>,
    dismiss: Option<Click<C>>,
    backdrop_dismiss: bool,
    escape_dismiss: bool,
}
/// Compose a controlled modal. Its width/styles apply to the card, not backdrop.
pub fn dialog<C>(content: impl Into<Element<C>>) -> Dialog<C> {
    let card = panel([content.into()])
        .width(420.)
        .padding(24., Edge::All)
        .gap(12., Axis::Vertical)
        .scroll(Scroll::Auto)
        .rounded(16)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.);
    Dialog(
        card.into(),
        DialogOptions {
            open: false,
            title: None,
            dismiss: None,
            backdrop_dismiss: true,
            escape_dismiss: true,
        },
    )
}
impl<C> Dialog<C> {
    pub fn open(mut self, open: bool) -> Self {
        self.1.open = open;
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.1.title = Some(title.into());
        self
    }
    /// Called for an allowed Escape or press/release on the backdrop. It must
    /// update application state (usually set `open = false`) for recomposition.
    pub fn on_dismiss(mut self, dismiss: impl FnMut(&mut C) + 'static) -> Self {
        self.1.dismiss = Some(Box::new(dismiss));
        self
    }
    pub fn backdrop_dismiss(mut self, dismiss: bool) -> Self {
        self.1.backdrop_dismiss = dismiss;
        self
    }
    pub fn escape_dismiss(mut self, dismiss: bool) -> Self {
        self.1.escape_dismiss = dismiss;
        self
    }
}
impl<C> From<Dialog<C>> for Element<C> {
    fn from(mut dialog: Dialog<C>) -> Self {
        let key = (!dialog.0.auto_id).then(|| dialog.0.node.id.clone());
        if !dialog.1.open {
            let mut empty = Element::new(Kind::ZStack);
            empty.node.width = Some(0.);
            empty.node.height = Some(0.);
            empty.node.effects.scroll_passthrough = true;
            if let Some(key) = key {
                empty.node.id = key;
                empty.auto_id = false;
            }
            return empty;
        }
        if let Some(title) = dialog.1.title {
            dialog
                .0
                .children
                .insert(0, text(title).font_size(20.).font_weight(600).into());
        }
        if let Some(key) = &key {
            dialog.0.node.id = format!("{key}-content");
        }
        let mut root = scope(dialog.0, OverlayKind::Modal, dialog.1.dismiss);
        root.node.overlay.as_mut().unwrap().backdrop_dismiss = dialog.1.backdrop_dismiss;
        root.node.overlay.as_mut().unwrap().escape_dismiss = dialog.1.escape_dismiss;
        if let Some(key) = key {
            root.node.id = key;
            root.auto_id = false;
        }
        root
    }
}
fn scope<C>(content: Element<C>, kind: OverlayKind, dismiss: Option<Click<C>>) -> Element<C> {
    let mut root = Element::new(Kind::ZStack);
    root.node.overlay = Some(Overlay {
        kind,
        dismiss_action: None,
        backdrop_dismiss: true,
        escape_dismiss: true,
        anchor: None,
        position: None,
        keep_anchor: false,
    });
    root.node.style.background = Some(
        match kind {
            OverlayKind::Modal => "#00000080",
            OverlayKind::Popup | OverlayKind::Tooltip => "#00000000",
        }
        .into(),
    );
    root.node.flow = Some(Flow::Center);
    root.children = vec![content];
    root.callback = dismiss.map(Callback::Dismiss);
    if root.callback.is_none() {
        root.validation_error = Some("open overlay requires on_dismiss".into());
    }
    root
}
/// Shared portal composition for controlled selection popups.
pub(super) fn overlay_scope<C>(
    content: Element<C>,
    key: String,
    kind: OverlayKind,
    dismiss: Click<C>,
) -> Element<C> {
    let mut root = scope(content, kind, Some(dismiss));
    root.node.id = key;
    root.auto_id = false;
    root
}
