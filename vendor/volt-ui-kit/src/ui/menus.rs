//! Typed, application-owned in-window menus; not operating-system menus.
use super::*;
use crate::declarative::document::OverlayKind;
use std::{cell::RefCell, rc::Rc};

/// One action row or separator in a declarative menu.
pub struct MenuItem<T> {
    kind: ItemKind<T>,
    key: Option<String>,
}
enum ItemKind<T> {
    Action {
        value: T,
        label: String,
        enabled: bool,
        danger: bool,
        shortcut: Option<String>,
        checked: Option<bool>,
        icon: Option<Icon>,
    },
    Separator,
    /// Opens in place of the parent list, with a back row (drill-down).
    Submenu {
        key: String,
        label: String,
        items: Vec<MenuItem<T>>,
    },
}
pub fn menu_item<T>(value: T, label: impl Into<String>) -> MenuItem<T> {
    MenuItem {
        kind: ItemKind::Action {
            value,
            label: label.into(),
            enabled: true,
            danger: false,
            shortcut: None,
            checked: None,
            icon: None,
        },
        key: None,
    }
}
/// A row that opens a nested list. Requires the menu's `on_submenu`.
pub fn menu_submenu<T>(
    key: impl Into<String>,
    label: impl Into<String>,
    items: impl IntoIterator<Item = MenuItem<T>>,
) -> MenuItem<T> {
    let key = key.into();
    MenuItem {
        kind: ItemKind::Submenu {
            key: key.clone(),
            label: label.into(),
            items: items.into_iter().take(128).collect(),
        },
        key: Some(key),
    }
}
pub fn menu_separator<T>() -> MenuItem<T> {
    MenuItem {
        kind: ItemKind::Separator,
        key: None,
    }
}
impl<T> MenuItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        if let ItemKind::Action { enabled: value, .. } = &mut self.kind {
            *value = enabled;
        }
        self
    }
    pub fn danger(mut self, danger: bool) -> Self {
        if let ItemKind::Action { danger: value, .. } = &mut self.kind {
            *value = danger;
        }
        self
    }
    /// A keyboard hint shown at the trailing edge, e.g. "⌘S". Display only:
    /// the app binds the shortcut itself.
    pub fn shortcut(mut self, hint: impl Into<String>) -> Self {
        if let ItemKind::Action { shortcut, .. } = &mut self.kind {
            *shortcut = Some(hint.into());
        }
        self
    }
    /// A checkable row (check or radio item): shows a check mark when true.
    pub fn checked(mut self, value: bool) -> Self {
        if let ItemKind::Action { checked, .. } = &mut self.kind {
            *checked = Some(value);
        }
        self
    }
    pub fn icon(mut self, glyph: Icon) -> Self {
        if let ItemKind::Action { icon, .. } = &mut self.kind {
            *icon = Some(glyph);
        }
        self
    }
}
type Action<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
type Open<C> = Rc<RefCell<dyn FnMut(&mut C, bool)>>;
type SubmenuChange<C> = Rc<RefCell<dyn FnMut(&mut C, Option<String>)>>;
struct MenuOptions<C, T> {
    items: Vec<MenuItem<T>>,
    open: bool,
    enabled: bool,
    action: Option<Action<C, T>>,
    change: Option<Open<C>>,
    submenu: Option<String>,
    on_submenu: Option<SubmenuChange<C>>,
}
impl<C, T> MenuOptions<C, T> {
    fn new(items: impl IntoIterator<Item = MenuItem<T>>) -> Self {
        Self {
            items: items.into_iter().take(129).collect(),
            open: false,
            enabled: true,
            action: None,
            change: None,
            submenu: None,
            on_submenu: None,
        }
    }
}
/// Controlled dropdown action menu. A stable key and both callbacks are required.
pub struct Menu<C, T>(pub(super) Element<C>, String, MenuOptions<C, T>);
pub fn menu<C, T>(
    label: impl Into<String>,
    items: impl IntoIterator<Item = MenuItem<T>>,
) -> Menu<C, T> {
    let mut e = Element::new(Kind::Column);
    e.node.width = Some(208.);
    Menu(e, label.into(), MenuOptions::new(items))
}
type ContextOpen<C> = Box<dyn FnMut(&mut C, [f32; 2])>;
/// A right-click menu attached to a target subtree. Positions are logical pixels.
pub struct ContextMenu<C, T>(
    pub(super) Element<C>,
    Element<C>,
    MenuOptions<C, T>,
    [f32; 2],
    Option<ContextOpen<C>>,
);
pub fn context_menu<C, T>(
    target: impl Into<Element<C>>,
    items: impl IntoIterator<Item = MenuItem<T>>,
) -> ContextMenu<C, T> {
    ContextMenu(
        Element::new(Kind::ZStack),
        target.into(),
        MenuOptions::new(items),
        [0.; 2],
        None,
    )
}
macro_rules! menu_options {
    ($kind:ident,$field:tt) => {
        impl<C, T> $kind<C, T> {
            pub fn open(mut self, open: bool) -> Self {
                self.$field.open = open;
                self
            }
            pub fn enabled(mut self, enabled: bool) -> Self {
                self.$field.enabled = enabled;
                self
            }
            pub fn on_action(mut self, action: impl FnMut(&mut C, T) + 'static) -> Self {
                self.$field.action = Some(Rc::new(RefCell::new(action)));
                self
            }
            pub fn on_open_change(mut self, change: impl FnMut(&mut C, bool) + 'static) -> Self {
                self.$field.change = Some(Rc::new(RefCell::new(change)));
                self
            }
            /// The open submenu's key (app state), shown in place of the list.
            pub fn submenu(mut self, key: Option<impl Into<String>>) -> Self {
                self.$field.submenu = key.map(Into::into);
                self
            }
            /// Called with a submenu key when one opens, `None` to go back.
            pub fn on_submenu(
                mut self,
                change: impl FnMut(&mut C, Option<String>) + 'static,
            ) -> Self {
                self.$field.on_submenu = Some(Rc::new(RefCell::new(change)));
                self
            }
        }
    };
}
menu_options!(Menu, 2);
menu_options!(ContextMenu, 2);
impl<C, T> ContextMenu<C, T> {
    pub fn position(mut self, point: [f32; 2]) -> Self {
        self.3 = point;
        self
    }
    /// Native hosts pass a right-press position; update position/open app state.
    pub fn on_context_open(mut self, callback: impl FnMut(&mut C, [f32; 2]) + 'static) -> Self {
        self.4 = Some(Box::new(callback));
        self
    }
}
fn error<C, T>(root: &Element<C>, options: &MenuOptions<C, T>) -> Option<String> {
    if root.auto_id {
        return Some("menu requires a stable .key(...)".into());
    }
    if options.items.is_empty()
        || options.items.len() > 128
        || !options
            .items
            .iter()
            .any(|i| matches!(i.kind, ItemKind::Action { .. }))
    {
        return Some("menu requires 1..=128 rows including an action".into());
    }
    if options.action.is_none() || options.change.is_none() {
        return Some("menu requires on_action and on_open_change".into());
    }
    fn has_submenu<T>(items: &[MenuItem<T>]) -> bool {
        items
            .iter()
            .any(|i| matches!(i.kind, ItemKind::Submenu { .. }))
    }
    if has_submenu(&options.items) && options.on_submenu.is_none() {
        return Some("menu with submenus requires on_submenu".into());
    }
    None
}
type Rows<T> = Vec<MenuItem<T>>;
/// The open submenu's label and rows, searched at any depth.
fn open_submenu<T>(
    items: Vec<MenuItem<T>>,
    key: &str,
) -> Result<(String, Vec<MenuItem<T>>), Rows<T>> {
    let mut rest = Vec::new();
    let mut found = None;
    for item in items {
        match item.kind {
            ItemKind::Submenu {
                key: k,
                label,
                items,
            } if found.is_none() => {
                if k == key {
                    found = Some((label, items));
                } else {
                    match open_submenu(items, key) {
                        Ok(hit) => found = Some(hit),
                        Err(items) => rest.push(MenuItem {
                            kind: ItemKind::Submenu {
                                key: k,
                                label,
                                items,
                            },
                            key: item.key,
                        }),
                    }
                }
            }
            kind => rest.push(MenuItem {
                kind,
                key: item.key,
            }),
        }
    }
    found.ok_or(rest)
}
const ROW: f32 = 34.;
/// A decorated row: the button stays the first child so menu keyboard
/// navigation and focus target it; glyphs and hints sit above, passthrough.
fn decorated<C>(
    b: Element<C>,
    leading: Option<Element<C>>,
    trailing: Option<Element<C>>,
    width: f32,
) -> Element<C> {
    let key = format!("{}-row", b.node.id);
    let mut parts: Vec<Element<C>> = Vec::new();
    parts.push(leading.unwrap_or_else(|| spacer().width(16.).flex(0.).into()));
    parts.push(spacer().into());
    if let Some(t) = trailing {
        parts.push(t);
    }
    zstack![
        b,
        hstack(parts)
            .key(format!("{key}-decor"))
            .width(width)
            .height(ROW)
            .padding(10., Edge::Horizontal)
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .scroll_passthrough(true),
    ]
    .key(key)
    .width(width)
    .height(ROW)
    .into()
}
struct Popup<C, T> {
    width: f32,
    key: String,
    anchor: String,
    action: Action<C, T>,
    change: Open<C>,
    submenu: Option<String>,
    on_submenu: Option<SubmenuChange<C>>,
}
fn popup<C: 'static, T: Clone + 'static>(items: Vec<MenuItem<T>>, p: Popup<C, T>) -> Element<C> {
    let Popup {
        width,
        key,
        anchor,
        action,
        change,
        submenu,
        on_submenu,
    } = p;
    let mut card = Element::new(Kind::Panel);
    card.node.id = format!("{key}-content");
    card.auto_id = false;
    card.node.width = Some(width);
    card.node.padding = 6.;
    card.node.gap = 2.;
    card.node.surface.scroll = Some(Scroll::Vertical);
    card.node.style.radius = Some(Rounding::Radius(10.));
    card.node.style.background = Some(ColorToken::Base200.into());
    card.node.style.border_color = Some(ColorToken::Border.into());
    card.node.style.border_width = Some(1.);
    let inner = (width - 12.).max(0.);
    let row_button = |label: String| {
        button(label)
            .height(ROW)
            .width(inner)
            .rounded(6)
            .background("#00000000")
            .border_width(0.)
            .label_align(Align::Start)
            .label_inset(34.)
            .hover_style(|s| s.background(ColorToken::Base300))
    };
    let mut rows: Vec<Element<C>> = Vec::new();
    let mut height = 12.;
    // Drill into the open submenu, with a back row first.
    let drilled = match submenu.as_deref() {
        Some(k) => open_submenu(items, k),
        None => Err(items),
    };
    let items = match drilled {
        Ok((label, sub)) => {
            let back = on_submenu.clone().unwrap();
            let b: Element<C> = row_button(label)
                .key(format!("{key}-back"))
                .accessible_label("Back")
                .font_weight(600)
                .on_click(move |app: &mut C| (back.borrow_mut())(app, None))
                .into();
            rows.push(decorated(
                b,
                Some(icon(Icon::ChevronLeft).size(14.).into()),
                None,
                inner,
            ));
            rows.push(separator().height(9.).into());
            height += ROW + 9.;
            sub
        }
        Err(items) => items,
    };
    for (index, item) in items.into_iter().enumerate() {
        let decorated_id = format!("{key}-item-{index}");
        let item_key = item.key;
        let mut child: Element<C> = match item.kind {
            ItemKind::Separator => {
                height += 9.;
                separator().height(9.).into()
            }
            ItemKind::Submenu { key: k, label, .. } => {
                height += ROW;
                let open = on_submenu.clone().unwrap();
                let mut b: Element<C> = row_button(label)
                    .on_click(move |app: &mut C| (open.borrow_mut())(app, Some(k.clone())))
                    .into();
                b.node.access_expanded = Some(false);
                if let Some(k) = &item_key {
                    b.node.id = k.clone();
                    b.auto_id = false;
                }
                let row = decorated(
                    b,
                    None,
                    Some(
                        icon(Icon::ChevronRight)
                            .size(14.)
                            .color(ColorToken::Muted)
                            .into(),
                    ),
                    inner,
                );
                rows.push(row);
                continue;
            }
            ItemKind::Action {
                value,
                label,
                enabled,
                danger,
                shortcut,
                checked,
                icon: glyph,
            } => {
                height += ROW;
                let action = action.clone();
                let change = change.clone();
                let reset = on_submenu.clone();
                let plain = shortcut.is_none() && checked.is_none() && glyph.is_none();
                let mut b = button(label)
                    .height(ROW)
                    .width(inner)
                    .rounded(6)
                    .background("#00000000")
                    .border_width(0.)
                    .enabled(enabled)
                    .hover_style(|s| s.background(ColorToken::Base300))
                    .on_click(move |app: &mut C| {
                        (action.borrow_mut())(app, value.clone());
                        (change.borrow_mut())(app, false);
                        if let Some(reset) = &reset {
                            (reset.borrow_mut())(app, None);
                        }
                    });
                if !plain {
                    b = b.label_align(Align::Start).label_inset(34.);
                }
                if danger {
                    b = b.color(ColorToken::Error);
                }
                let mut b: Element<C> = b.into();
                if let Some(on) = checked {
                    b.node.checked = on;
                    b.node.access_selected = Some(on);
                }
                if let Some(k) = &item_key {
                    b.node.id = k.clone();
                    b.auto_id = false;
                } else if !plain {
                    b.node.id = decorated_id;
                    b.auto_id = false;
                }
                if plain {
                    b
                } else {
                    let tint = if danger {
                        ColorToken::Error
                    } else {
                        ColorToken::BaseContent
                    };
                    let leading = match (checked, glyph) {
                        (Some(true), _) => Some(icon(Icon::Check).size(14.).color(tint).into()),
                        (Some(false), _) => None,
                        (None, Some(g)) => Some(icon(g).size(15.).color(tint).into()),
                        (None, None) => None,
                    };
                    let trailing = shortcut
                        .map(|hint| text(hint).font_size(12.).color(ColorToken::Muted).into());
                    rows.push(decorated(b, leading, trailing, inner));
                    continue;
                }
            }
        };
        if let Some(key) = item_key {
            child.node.id = key;
            child.auto_id = false;
        }
        rows.push(child);
    }
    let count = rows.len();
    card.children = rows;
    card.node.height = Some((height + 2. * count.saturating_sub(1) as f32).min(360.));
    let close = change.clone();
    let reset = on_submenu;
    let mut root = super::dialogs::overlay_scope(
        card,
        key,
        OverlayKind::Popup,
        Box::new(move |app: &mut C| {
            (close.borrow_mut())(app, false);
            if let Some(reset) = &reset {
                (reset.borrow_mut())(app, None);
            }
        }),
    );
    root.node.overlay.as_mut().unwrap().anchor = Some(anchor);
    root
}
impl<C: 'static, T: Clone + 'static> From<Menu<C, T>> for Element<C> {
    fn from(mut menu: Menu<C, T>) -> Self {
        if let Some(error) = error(&menu.0, &menu.2) {
            menu.0.validation_error = Some(error);
            return menu.0;
        }
        let width = menu.0.node.width.unwrap_or(208.);
        let trigger_key = format!("{}-trigger", menu.0.node.id);
        let change = menu.2.change.unwrap();
        let open = menu.2.open;
        let callback = change.clone();
        let mut trigger = button(menu.1.clone())
            .disclosure(open)
            .key(trigger_key.clone())
            .height(menu.0.node.height.unwrap_or(36.))
            .enabled(menu.2.enabled)
            .on_click(move |app: &mut C| (callback.borrow_mut())(app, !open));
        trigger.0.node.width = menu.0.node.width;
        trigger.0.node.accessible_label = menu.0.node.accessible_label.take();
        trigger.0.node.style = menu.0.node.style.clone();
        menu.0.children.push(trigger.into());
        if open && menu.2.enabled {
            let key = format!("{}-popup", menu.0.node.id);
            menu.0.children.push(popup(
                menu.2.items,
                Popup {
                    width,
                    key,
                    anchor: trigger_key,
                    action: menu.2.action.unwrap(),
                    change,
                    submenu: menu.2.submenu,
                    on_submenu: menu.2.on_submenu,
                },
            ));
        }
        menu.0
    }
}
impl<C: 'static, T: Clone + 'static> From<ContextMenu<C, T>> for Element<C> {
    fn from(mut menu: ContextMenu<C, T>) -> Self {
        if let Some(error) = error(&menu.0, &menu.2) {
            menu.0.validation_error = Some(error);
            return menu.0;
        }
        let Some(callback) = menu.4 else {
            menu.0.validation_error = Some("context_menu requires on_context_open".into());
            return menu.0;
        };
        if !menu
            .3
            .iter()
            .all(|v| v.is_finite() && (0. ..=65_536.).contains(v))
        {
            menu.0.validation_error =
                Some("context_menu position must be finite nonnegative logical pixels".into());
            return menu.0;
        }
        menu.0.node.enabled = menu.2.enabled;
        menu.0.callback = Some(Callback::Context(callback));
        if menu.1.auto_id {
            menu.1.node.id = format!("{}-target", menu.0.node.id);
            menu.1.auto_id = false;
        }
        let anchor = menu.1.node.id.clone();
        menu.0.children.push(menu.1);
        if menu.2.open && menu.2.enabled {
            let key = format!("{}-popup", menu.0.node.id);
            let mut popup = popup(
                menu.2.items,
                Popup {
                    width: menu.0.node.width.unwrap_or(208.),
                    key,
                    anchor,
                    action: menu.2.action.unwrap(),
                    change: menu.2.change.unwrap(),
                    submenu: menu.2.submenu,
                    on_submenu: menu.2.on_submenu,
                },
            );
            popup.node.overlay.as_mut().unwrap().position = Some(menu.3);
            menu.0.children.push(popup);
        }
        menu.0
    }
}

#[cfg(test)]
mod style_tests {
    use super::*;
    #[test]
    fn menu_visual_modifiers_style_the_actual_trigger() {
        let ui: Element<()> = menu("Actions", [menu_item(1usize, "Save")])
            .key("actions")
            .fill_width()
            .height(40.)
            .rounded(10.)
            .font_size(13.)
            .background(ColorToken::Base300)
            .on_action(|_, _| {})
            .on_open_change(|_, _| {})
            .into();
        assert!(ui.node.width.is_none());
        let trigger = &ui.children[0].node;
        assert!(trigger.width.is_none());
        assert_eq!(trigger.height, Some(40.));
        assert_eq!(trigger.style.radius, Some(Rounding::Radius(10.)));
        assert_eq!(trigger.style.font_size, Some(13.));
        assert_eq!(trigger.style.background.as_deref(), Some("@base-300"));
        ui.build().unwrap();
    }
}
