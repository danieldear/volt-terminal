//! Rust-only composites lowered to existing layout/button primitives.
use super::*;
use std::{cell::RefCell, rc::Rc};

/// A horizontal action group. Joined groups suppress gaps and share outer corners.
pub struct ButtonBar<C>(pub(super) Element<C>, bool);
/// An application-owned toolbar, not an OS-native menu or titlebar.
pub struct Toolbar<C>(pub(super) Element<C>);
pub fn button_bar<C>(children: impl IntoIterator<Item = Element<C>>) -> ButtonBar<C> {
    let mut e = container(Kind::Row, children).0;
    e.node.gap = 8.;
    e.node.style.radius = Some(Rounding::Radius(6.));
    ButtonBar(e, false)
}
pub fn toolbar<C>(children: impl IntoIterator<Item = Element<C>>) -> Toolbar<C> {
    let mut e = container(Kind::Row, children).0;
    e.node.gap = 8.;
    e.node.align = Align::Center;
    Toolbar(e)
}
/// Square the inner corners of a group's children and separate them with a
/// 1px seam through which the surface behind shows, so the segments read as
/// one divided control. (A group background would fill the full row width
/// when a column stretches the group.)
fn join<C>(e: &mut Element<C>) {
    e.node.gap = 1.;
    e.node.gap_horizontal = None;
    e.node.responsive.gap = [None, None];
    let radius = e.node.style.radius.unwrap_or(Rounding::Radius(6.));
    let corners = e.node.style.corners.unwrap_or([radius; 4]);
    let last = e.children.len().saturating_sub(1);
    for (i, child) in e.children.iter_mut().enumerate() {
        child.node.style.radius = None;
        child.node.style.corners = Some([
            if i == 0 {
                corners[0]
            } else {
                Rounding::Radius(0.)
            },
            if i == last {
                corners[1]
            } else {
                Rounding::Radius(0.)
            },
            if i == last {
                corners[2]
            } else {
                Rounding::Radius(0.)
            },
            if i == 0 {
                corners[3]
            } else {
                Rounding::Radius(0.)
            },
        ]);
    }
}
impl<C> ButtonBar<C> {
    pub fn joined(mut self, joined: bool) -> Self {
        self.1 = joined;
        self
    }
}
impl<C> From<ButtonBar<C>> for Element<C> {
    fn from(mut bar: ButtonBar<C>) -> Self {
        if bar.1 {
            join(&mut bar.0);
        }
        bar.0
    }
}
impl<C> From<Toolbar<C>> for Element<C> {
    fn from(bar: Toolbar<C>) -> Self {
        bar.0
    }
}

/// A typed selection item. Explicit keys are recommended for dynamic/reordered items.
pub struct TabItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
}
pub fn tab_item<T>(value: T, label: impl Into<String>) -> TabItem<T> {
    TabItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
    }
}
impl<T> TabItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
type Select<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
/// Discrete typed selection; Left/Right and Home/End navigate the group in native hosts.
pub struct SelectionControl<C, T>(pub(super) Element<C>, SelectionOptions<C, T>);
struct SelectionOptions<C, T> {
    items: Vec<TabItem<T>>,
    selected: Option<T>,
    callback: Option<Select<C, T>>,
    enabled: bool,
    joined: bool,
}
pub fn tab_bar<C, T>(items: impl IntoIterator<Item = TabItem<T>>) -> SelectionControl<C, T> {
    let mut e = Element::new(Kind::Row);
    e.node.gap = 4.;
    e.node.selection_group = true;
    e.node.access_role = Some(AccessRole::TabList);
    SelectionControl(
        e,
        SelectionOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            enabled: true,
            joined: false,
        },
    )
}
pub fn segmented_control<C, T>(
    items: impl IntoIterator<Item = (T, impl Into<String>)>,
) -> SelectionControl<C, T> {
    let mut bar = tab_bar(
        items
            .into_iter()
            .map(|(value, label)| tab_item(value, label)),
    );
    bar.1.joined = true;
    bar.0.node.access_role = Some(AccessRole::RadioGroup);
    bar
}
impl<C, T> SelectionControl<C, T> {
    pub fn selected(mut self, selected: T) -> Self {
        self.1.selected = Some(selected);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.1.enabled = enabled;
        self
    }
    pub fn joined(mut self, joined: bool) -> Self {
        self.1.joined = joined;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
fn selection_error<T: PartialEq>(items: &[TabItem<T>], selected: &T) -> Option<String> {
    // Reject oversized candidates before the quadratic PartialEq duplicate scan.
    if items.len() >= MAX_NODES {
        return Some("selection item count exceeds the document node limit".into());
    }
    if items.is_empty() {
        return Some("selection requires at least one item".into());
    }
    if !items.iter().any(|item| item.value == *selected) {
        return Some("selected value is not an item".into());
    }
    if items
        .iter()
        .enumerate()
        .any(|(i, item)| items[..i].iter().any(|other| other.value == item.value))
    {
        return Some("duplicate selection value".into());
    }
    None
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<SelectionControl<C, T>> for Element<C> {
    fn from(mut bar: SelectionControl<C, T>) -> Self {
        let selected = bar
            .1
            .selected
            .or_else(|| bar.1.items.first().map(|item| item.value.clone()));
        let Some(selected) = selected else {
            bar.0.validation_error = Some("selection requires at least one item".into());
            return bar.0;
        };
        if let Some(error) = selection_error(&bar.1.items, &selected) {
            bar.0.validation_error = Some(error);
            return bar.0;
        }
        let Some(callback) = bar.1.callback else {
            bar.0.validation_error = Some("selection requires on_select".into());
            return bar.0;
        };
        for item in bar.1.items {
            let active = item.value == selected;
            let value = item.value;
            let handler = callback.clone();
            let mut child = button(item.label)
                .primary(active)
                .enabled(bar.1.enabled && item.enabled)
                .on_click(move |context: &mut C| {
                    if !active {
                        (handler.borrow_mut())(context, value.clone());
                    }
                });
            // The selected surface uses Primary; a parent's general text color
            // must not replace its paired foreground (e.g. white on pale Iris).
            if active {
                child = child.color(ColorToken::PrimaryContent);
            }
            child.0.node.checked = active;
            child.0.node.access_selected = Some(active);
            child.0.node.access_role =
                Some(if bar.0.node.access_role == Some(AccessRole::RadioGroup) {
                    AccessRole::RadioButton
                } else {
                    AccessRole::Tab
                });
            if let Some(key) = item.key {
                child = child.key(key);
            }
            bar.0.children.push(child.into());
        }
        if bar.1.joined {
            join(&mut bar.0);
        }
        bar.0
    }
}

/// Placement of the tab strip relative to its active content, not mobile navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TabPlacement {
    #[default]
    Top,
    Bottom,
}
pub struct Tab<C, T> {
    item: TabItem<T>,
    content: Element<C>,
}
pub fn tab<C, T>(value: T, label: impl Into<String>, content: impl Into<Element<C>>) -> Tab<C, T> {
    Tab {
        item: tab_item(value, label),
        content: content.into(),
    }
}
impl<C, T> Tab<C, T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.item = self.item.key(key);
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.item = self.item.enabled(enabled);
        self
    }
}
pub struct Tabs<C, T>(pub(super) Element<C>, TabsOptions<C, T>);
struct TabsOptions<C, T> {
    items: Vec<Tab<C, T>>,
    selected: Option<T>,
    callback: Option<Select<C, T>>,
    placement: TabPlacement,
}
pub fn tabs<C, T>(items: impl IntoIterator<Item = Tab<C, T>>) -> Tabs<C, T> {
    let mut e = Element::new(Kind::Column);
    e.node.gap = 12.;
    Tabs(
        e,
        TabsOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            placement: TabPlacement::Top,
        },
    )
}
impl<C, T> Tabs<C, T> {
    pub fn selected(mut self, value: T) -> Self {
        self.1.selected = Some(value);
        self
    }
    pub fn placement(mut self, value: TabPlacement) -> Self {
        self.1.placement = value;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<Tabs<C, T>> for Element<C> {
    fn from(mut tabs: Tabs<C, T>) -> Self {
        if tabs.1.items.len() >= MAX_NODES {
            tabs.0.validation_error = Some("tab count exceeds the document node limit".into());
            return tabs.0;
        }
        let selected = tabs
            .1
            .selected
            .or_else(|| tabs.1.items.first().map(|tab| tab.item.value.clone()));
        let Some(selected) = selected else {
            tabs.0.validation_error = Some("tabs require at least one tab".into());
            return tabs.0;
        };
        let mut items = Vec::new();
        let mut content = None;
        for tab in tabs.1.items {
            if tab.item.value == selected && content.is_none() {
                content = Some(tab.content);
            }
            items.push(tab.item);
        }
        let mut bar = tab_bar(items).selected(selected);
        bar.1.callback = tabs.1.callback;
        let mut strip: Element<C> = bar.into();
        if !tabs.0.auto_id {
            strip.node.id = format!("{}-strip", tabs.0.node.id);
            strip.auto_id = false;
        }
        if let Some(content) = content {
            tabs.0.children = match tabs.1.placement {
                TabPlacement::Top => vec![strip, content],
                TabPlacement::Bottom => vec![content, strip],
            };
        } else {
            tabs.0.children = vec![strip];
        }
        tabs.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_label_uses_the_paired_primary_foreground() {
        let e: Element<()> = tab_bar([tab_item(0, "One"), tab_item(1, "Two")])
            .selected(0)
            .on_select(|_: &mut (), _| {})
            .color(ColorToken::BaseContent)
            .into();
        assert_eq!(
            e.children[0].node.style.color.as_deref(),
            Some("@primary-content")
        );
        assert_eq!(e.children[1].node.style.color, None);
    }
    #[test]
    fn oversized_selection_is_rejected_before_lowering() {
        let items = (0..MAX_NODES)
            .map(|i| tab_item(i, "Item"))
            .collect::<Vec<_>>();
        assert!(selection_error(&items, &0).unwrap().contains("node limit"));
        let e: Element<()> = tab_bar(items).on_select(|_: &mut (), _| {}).into();
        assert!(e.children.is_empty());
        assert!(e.build().is_err());
    }
    fn buttons() -> ButtonBar<()> {
        button_bar([
            button("One").on_click(|_: &mut ()| {}).into(),
            button("Two").on_click(|_: &mut ()| {}).into(),
            button("Three").on_click(|_: &mut ()| {}).into(),
        ])
    }
    #[test]
    fn joined_corners_keep_symbolic_full_and_square_inner_edges() {
        let e: Element<()> = buttons()
            .joined(true)
            .rounded(Rounding::Full)
            .gap(20., Axis::Both)
            .into();
        let zero = Rounding::Radius(0.);
        assert_eq!(e.node.gap, 1., "a 1px seam between joined segments");
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Full, zero, zero, Rounding::Full])
        );
        assert_eq!(e.children[1].node.style.corners, Some([zero; 4]));
        assert_eq!(
            e.children[2].node.style.corners,
            Some([zero, Rounding::Full, Rounding::Full, zero])
        );
        assert!(e.build().is_ok());
    }
    #[test]
    fn joined_corners_respect_selected_outer_corners() {
        let e: Element<()> = buttons()
            .joined(true)
            .rounded_on(10., [Corner::TopLeft, Corner::BottomRight])
            .into();
        let zero = Rounding::Radius(0.);
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Radius(10.), zero, zero, zero])
        );
        assert_eq!(
            e.children[2].node.style.corners,
            Some([zero, zero, Rounding::Radius(10.), zero])
        );
    }
    #[test]
    fn single_joined_button_keeps_all_outer_corners() {
        let e: Element<()> = button_bar![button("Only").on_click(|_: &mut ()| {})]
            .joined(true)
            .rounded(8.)
            .into();
        assert_eq!(
            e.children[0].node.style.corners,
            Some([Rounding::Radius(8.); 4])
        );
    }
}
