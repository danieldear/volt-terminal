//! Independent destination navigation. Application routing remains app-owned.
use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NavigationPlacement {
    #[default]
    Sidebar,
    Rail,
    Bottom,
}
pub struct NavigationItem<T> {
    value: T,
    label: String,
    key: Option<String>,
    enabled: bool,
}
pub fn navigation_item<T>(value: T, label: impl Into<String>) -> NavigationItem<T> {
    NavigationItem {
        value,
        label: label.into(),
        key: None,
        enabled: true,
    }
}
impl<T> NavigationItem<T> {
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.key = Some(key.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}
type Selection<C, T> = Rc<RefCell<dyn FnMut(&mut C, T)>>;
pub struct Navigation<C, T>(pub(super) Element<C>, NavigationOptions<C, T>);
struct NavigationOptions<C, T> {
    items: Vec<NavigationItem<T>>,
    selected: Option<T>,
    callback: Option<Selection<C, T>>,
    placement: NavigationPlacement,
}
pub fn navigation<C, T>(items: impl IntoIterator<Item = NavigationItem<T>>) -> Navigation<C, T> {
    let mut element = Element::new(Kind::Column);
    element.node.gap = 4.;
    element.node.access_role = Some(AccessRole::Group);
    Navigation(
        element,
        NavigationOptions {
            items: items.into_iter().collect(),
            selected: None,
            callback: None,
            placement: NavigationPlacement::Sidebar,
        },
    )
}
impl<C, T> Navigation<C, T> {
    pub fn selected(mut self, value: T) -> Self {
        self.1.selected = Some(value);
        self
    }
    pub fn placement(mut self, value: NavigationPlacement) -> Self {
        self.1.placement = value;
        self
    }
    pub fn on_select(mut self, callback: impl FnMut(&mut C, T) + 'static) -> Self {
        self.1.callback = Some(Rc::new(RefCell::new(callback)));
        self
    }
}
impl<C: 'static, T: Clone + PartialEq + 'static> From<Navigation<C, T>> for Element<C> {
    fn from(mut nav: Navigation<C, T>) -> Self {
        let invalid = nav.1.items.is_empty()
            || nav.1.items.len() >= MAX_NODES
            || nav
                .1
                .items
                .iter()
                .any(|i| i.key.as_deref().is_none_or(str::is_empty))
            || nav.1.items.iter().enumerate().any(|(n, i)| {
                nav.1.items[..n]
                    .iter()
                    .any(|p| p.value == i.value || p.key == i.key)
            })
            || nav
                .1
                .selected
                .as_ref()
                .is_some_and(|v| !nav.1.items.iter().any(|i| &i.value == v));
        if invalid || nav.1.callback.is_none() || nav.0.auto_id {
            nav.0.validation_error = Some("navigation needs a group key, nonempty unique keyed items, valid selection and on_select".into());
            return nav.0;
        }
        let selected = nav
            .1
            .selected
            .unwrap_or_else(|| nav.1.items[0].value.clone());
        let callback = nav.1.callback.unwrap();
        nav.0.node.kind = if nav.1.placement == NavigationPlacement::Bottom {
            Kind::Row
        } else {
            Kind::Column
        };
        for item in nav.1.items {
            let active = item.value == selected;
            let value = item.value;
            let handler = callback.clone();
            let mut child = button(item.label)
                .key(format!("{}-{}", nav.0.node.id, item.key.unwrap()))
                .variant(if active {
                    ButtonVariant::Soft
                } else {
                    ButtonVariant::Ghost
                })
                .label_align(if nav.1.placement == NavigationPlacement::Sidebar {
                    Align::Start
                } else {
                    Align::Center
                })
                .enabled(item.enabled)
                .on_click(move |app: &mut C| {
                    if !active {
                        (handler.borrow_mut())(app, value.clone());
                    }
                });
            child.0.node.access_selected = Some(active);
            nav.0.children.push(child.into());
        }
        nav.0
    }
}
