//! Controlled anchored cards and runtime-driven nonblocking help bubbles.
use super::*;
use crate::declarative::document::{Overlay, OverlayKind};
pub struct Popover<C>(pub(super) Element<C>, PopoverOptions<C>);
struct PopoverOptions<C> {
    open: bool,
    anchor: Option<String>,
    dismiss: Option<Click<C>>,
    keep_anchor: bool,
}
pub fn popover<C>(content: impl Into<Element<C>>) -> Popover<C> {
    let card = panel([content.into()])
        .width(280.)
        .padding(16., Edge::All)
        .gap(10., Axis::Vertical)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(12)
        .scroll(Scroll::Auto);
    Popover(
        card.into(),
        PopoverOptions {
            open: false,
            anchor: None,
            dismiss: None,
            keep_anchor: false,
        },
    )
}
impl<C> Popover<C> {
    pub fn anchor(mut self, key: impl Into<String>) -> Self {
        self.1.anchor = Some(key.into());
        self
    }
    /// Keep the anchor usable while open: focus stays on it and pointer
    /// presses on it do not dismiss (searchable selects type into it).
    pub fn keep_anchor_interactive(mut self, value: bool) -> Self {
        self.1.keep_anchor = value;
        self
    }
    pub fn open(mut self, open: bool) -> Self {
        self.1.open = open;
        self
    }
    pub fn on_dismiss(mut self, callback: impl FnMut(&mut C) + 'static) -> Self {
        self.1.dismiss = Some(Box::new(callback));
        self
    }
}
impl<C> From<Popover<C>> for Element<C> {
    fn from(mut p: Popover<C>) -> Self {
        if p.0.auto_id {
            p.0.validation_error = Some("popover requires a stable .key(...)".into());
            return p.0;
        }
        let key = p.0.node.id.clone();
        if !p.1.open {
            return Element::new(Kind::ZStack)
                .key(key)
                .width(0.)
                .height(0.)
                .scroll_passthrough(true);
        }
        let Some(anchor) = p.1.anchor else {
            p.0.validation_error = Some("open popover requires .anchor(...)".into());
            return p.0;
        };
        let Some(dismiss) = p.1.dismiss else {
            p.0.validation_error = Some("open popover requires on_dismiss".into());
            return p.0;
        };
        p.0.node.id = format!("{key}-content");
        let mut root = super::dialogs::overlay_scope(p.0, key, OverlayKind::Popup, dismiss);
        let overlay = root.node.overlay.as_mut().unwrap();
        overlay.anchor = Some(anchor);
        overlay.keep_anchor = p.1.keep_anchor;
        root
    }
}
/// Noninteractive help shown while its target is hovered or keyboard-focused.
/// No application callback, modal scope, timer or operating-system tooltip.
pub struct Tooltip<C>(pub(super) Element<C>, Element<C>);
pub fn tooltip<C>(label: impl Into<String>, target: impl Into<Element<C>>) -> Tooltip<C> {
    let card = panel([text(label).into()])
        .font_size(12.)
        .width(240.)
        .padding(10., Edge::Horizontal)
        .padding(8., Edge::Vertical)
        .rounded(8)
        .background(ColorToken::Base300)
        .border_color(ColorToken::Border)
        .border_width(1.);
    Tooltip(card.into(), target.into())
}
/// A richer hover preview: any noninteractive content shown while the
/// target is hovered or keyboard-focused (a tooltip with a card body).
pub fn hover_card<C>(content: impl Into<Element<C>>, target: impl Into<Element<C>>) -> Tooltip<C> {
    let card = panel([content.into()])
        .width(300.)
        .padding(14., Edge::All)
        .gap(8., Axis::Vertical)
        .rounded(12)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .shadow(super::tokens::shadow::lg());
    Tooltip(card.into(), target.into())
}
impl<C> From<Tooltip<C>> for Element<C> {
    fn from(mut tooltip: Tooltip<C>) -> Self {
        if tooltip.0.auto_id {
            tooltip.0.validation_error = Some("tooltip requires a stable .key(...)".into());
            return tooltip.0;
        }
        let key = tooltip.0.node.id.clone();
        if tooltip.1.auto_id {
            tooltip.1.node.id = format!("{key}-anchor");
            tooltip.1.auto_id = false;
        }
        let anchor = tooltip.1.node.id.clone();
        tooltip.0.node.id = format!("{key}-tooltip-content");
        fn passthrough<C>(element: &mut Element<C>) {
            element.node.effects.scroll_passthrough = true;
            for child in &mut element.children {
                passthrough(child);
            }
        }
        passthrough(&mut tooltip.0);
        let mut portal = Element::new(Kind::ZStack)
            .key(format!("{key}-tooltip"))
            .scroll_passthrough(true);
        portal.node.overlay = Some(Overlay {
            kind: OverlayKind::Tooltip,
            dismiss_action: None,
            backdrop_dismiss: false,
            escape_dismiss: false,
            anchor: Some(anchor),
            position: None,
            keep_anchor: false,
        });
        portal.children.push(tooltip.0);
        let mut root = Element::new(Kind::ZStack).key(key);
        root.children = vec![tooltip.1, portal];
        root
    }
}
