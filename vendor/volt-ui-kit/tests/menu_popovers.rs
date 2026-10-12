#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, SelectionKey, TextMetrics},
    ui::prelude::*,
    Painter, Viewport,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Save,
    Delete,
    Export,
}
#[derive(Default)]
struct App {
    menu: bool,
    context: bool,
    popover: bool,
    point: [f32; 2],
    actions: Vec<Action>,
    clicks: usize,
}
struct Metrics(f32);
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(width),
            size * 1.2,
        ]
    }
}
impl Painter for Metrics {
    type Shape = ();
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600. * self.0, 400. * self.0, self.0).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(&mut self, _: &mut Vec<()>, _: &str, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<()>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for Metrics {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}
fn mount(ui: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    layout(&mut r, scale);
    (r, h)
}
fn layout(r: &mut Runtime, scale: f32) {
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
fn replace(r: &mut Runtime, h: &mut Handlers<'static, App>, ui: Element<App>, scale: f32) {
    let (doc, next) = ui.build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    *h = next;
    layout(r, scale);
}
fn point(r: &Runtime, key: &str) -> [f32; 2] {
    let [x, y, w, h] = r
        .boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap()
        .bounds
        .bounds();
    [x + w / 2., y + h / 2.]
}
fn click(r: &mut Runtime, h: &mut Handlers<'static, App>, a: &mut App, key: &str) -> bool {
    let [x, y] = point(r, key);
    r.pointer_down(x, y);
    r.pointer_up_outcome(x, y, h, a).unwrap().activated
}
fn actions(a: &App) -> Element<App> {
    menu(
        "Actions",
        [
            menu_item(Action::Save, "Save").key("save"),
            menu_separator(),
            menu_item(Action::Delete, "Delete")
                .key("delete")
                .enabled(false)
                .danger(true),
            menu_item(Action::Export, "Export").key("export"),
        ],
    )
    .key("actions")
    .open(a.menu)
    .on_action(|a: &mut App, v| a.actions.push(v))
    .on_open_change(|a: &mut App, v| a.menu = v)
    .into()
}
#[test]
fn menu_keyboard_skips_separator_and_disabled_items_commits_typed_action() {
    for scale in [1., 2.] {
        let mut a = App::default();
        let (mut r, mut h) = mount(actions(&a), scale);
        assert!(click(&mut r, &mut h, &mut a, "actions-trigger"));
        replace(&mut r, &mut h, actions(&a), scale);
        assert_eq!(r.focused(), Some("save"));
        assert!(r.overlay_key(SelectionKey::Next));
        layout(&mut r, scale);
        assert_eq!(r.focused(), Some("export"));
        assert!(a.actions.is_empty());
        r.key_down();
        assert!(r.key_up_outcome(&mut h, &mut a).unwrap().activated);
        assert_eq!(a.actions, vec![Action::Export]);
        assert!(!a.menu);
        replace(&mut r, &mut h, actions(&a), scale);
        assert_eq!(r.focused(), Some("actions-trigger"));
    }
}
#[test]
fn menu_escape_and_outside_release_close_without_actions() {
    let mut a = App {
        menu: true,
        ..App::default()
    };
    let (mut r, mut h) = mount(actions(&a), 1.);
    assert!(r.dismiss_overlay(&mut h, &mut a).unwrap().activated);
    assert!(!a.menu);
    assert!(a.actions.is_empty());
    a.menu = true;
    replace(&mut r, &mut h, actions(&a), 1.);
    r.pointer_down(590., 390.);
    r.pointer_up_outcome(590., 390., &mut h, &mut a).unwrap();
    assert!(!a.menu);
    assert!(a.actions.is_empty());
}
fn contextual(a: &App) -> Element<App> {
    context_menu(
        button("Document")
            .key("document")
            .on_click(|a: &mut App| a.clicks += 1),
        [
            menu_item(Action::Save, "Save").key("context-save"),
            menu_item(Action::Export, "Export").key("context-export"),
        ],
    )
    .key("context")
    .open(a.context)
    .position(a.point)
    .on_context_open(|a: &mut App, p| {
        a.point = p;
        a.context = true;
    })
    .on_open_change(|a: &mut App, v| a.context = v)
    .on_action(|a: &mut App, v| a.actions.push(v))
    .into()
}
#[test]
fn context_request_uses_logical_pointer_coordinates_and_does_not_click_target() {
    for scale in [1., 2.] {
        let mut a = App::default();
        let (mut r, mut h) = mount(contextual(&a), scale);
        let p = point(&r, "document");
        assert!(
            r.context_menu_at(p[0], p[1], &mut h, &mut a)
                .unwrap()
                .activated
        );
        assert_eq!(a.point, [p[0] / scale, p[1] / scale]);
        assert_eq!(a.clicks, 0);
        replace(&mut r, &mut h, contextual(&a), scale);
        assert_eq!(r.focused(), Some("context-save"));
        assert!(click(&mut r, &mut h, &mut a, "context-export"));
        assert_eq!(a.actions, vec![Action::Export]);
        assert!(!a.context);
    }
}
#[test]
fn context_popup_clamps_or_flips_at_both_viewport_edges() {
    for scale in [1., 2.] {
        let a = App {
            context: true,
            point: [590., 390.],
            ..App::default()
        };
        let (r, _) = mount(contextual(&a), scale);
        let card = r
            .boxes()
            .iter()
            .find(|b| b.id == "context-popup-content")
            .unwrap();
        let [x, y, w, h] = card.bounds.bounds();
        assert!(x >= 0. && y >= 0. && x + w <= 600. * scale + 0.01 && y + h <= 400. * scale + 0.01);
    }
}
fn detail(a: &App) -> Element<App> {
    zstack![
        button("Details")
            .key("anchor")
            .on_click(|a: &mut App| a.popover = true),
        popover(
            button("Inside")
                .key("inside")
                .on_click(|a: &mut App| a.clicks += 1)
        )
        .key("details")
        .anchor("anchor")
        .open(a.popover)
        .on_dismiss(|a: &mut App| a.popover = false),
    ]
    .align(Align::Stretch)
    .into()
}
#[test]
fn anchored_popover_traps_focus_and_restores_anchor_after_escape() {
    let mut a = App::default();
    let (mut r, mut h) = mount(detail(&a), 1.);
    click(&mut r, &mut h, &mut a, "anchor");
    replace(&mut r, &mut h, detail(&a), 1.);
    assert_eq!(r.focused(), Some("inside"));
    r.focus_next(false);
    assert_eq!(r.focused(), Some("inside"));
    assert!(r.dismiss_overlay(&mut h, &mut a).unwrap().activated);
    replace(&mut r, &mut h, detail(&a), 1.);
    assert_eq!(r.focused(), Some("anchor"));
}
fn hint() -> Element<App> {
    tooltip(
        "Save this document",
        button("Save")
            .key("hint-target")
            .on_click(|a: &mut App| a.clicks += 1),
    )
    .key("hint")
    .into()
}
#[test]
fn tooltip_appears_on_hover_and_disappears_without_installing_a_modal_scope() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(hint(), scale);
        let mut a = App::default();
        assert!(!r.boxes().iter().any(|b| b.id == "hint-tooltip"));
        let [x, y] = point(&r, "hint-target");
        r.pointer_move(x, y);
        layout(&mut r, scale);
        assert!(r.boxes().iter().any(|b| b.id == "hint-tooltip"));
        assert!(!r.dismiss_overlay(&mut h, &mut a).unwrap().activated);
        assert!(click(&mut r, &mut h, &mut a, "hint-target"));
        assert_eq!(a.clicks, 1);
        r.pointer_leave();
        r.focus_next(false); // Keyboard focus still exposes the tooltip.
        layout(&mut r, scale);
        assert!(r.boxes().iter().any(|b| b.id == "hint-tooltip"));
        let (doc, next) = Element::from(text::<App>("Removed"))
            .build()
            .unwrap()
            .into_parts();
        r.replace_document(doc, &next, &Bindings::default())
            .unwrap();
        layout(&mut r, scale);
        assert!(!r.boxes().iter().any(|b| b.id == "hint-tooltip"));
    }
}
#[test]
fn tooltip_keyboard_focus_exposes_help_and_mouse_leave_without_focus_hides_it() {
    let (mut r, _) = mount(hint(), 1.);
    let [x, y] = point(&r, "hint-target");
    r.pointer_move(x, y);
    layout(&mut r, 1.);
    r.pointer_leave();
    layout(&mut r, 1.);
    assert!(!r.boxes().iter().any(|b| b.id == "hint-tooltip"));
    r.focus_next(false);
    layout(&mut r, 1.);
    assert!(r.boxes().iter().any(|b| b.id == "hint-tooltip"));
}
#[test]
fn menus_and_popovers_reject_missing_handlers_identity_and_invalid_positions() {
    let e: Element<App> = menu("Missing", [menu_item(Action::Save, "Save")])
        .key("missing")
        .into();
    assert!(e.build().is_err());
    let e: Element<App> = popover(text("No anchor"))
        .key("no-anchor")
        .open(true)
        .on_dismiss(|_| {})
        .into();
    assert!(e.build().is_err());
    let a = App {
        context: true,
        point: [f32::NAN, 2.],
        ..App::default()
    };
    assert!(contextual(&a).build().is_err());
}

#[test]
fn long_menu_navigation_counts_separators_but_never_focuses_them() {
    for scale in [1., 2.] {
        let mut rows = Vec::new();
        for index in 0..64 {
            rows.push(
                menu_item(Action::Save, format!("Action {index}"))
                    .key(format!("long-action-{index}")),
            );
            if index < 63 {
                rows.push(menu_separator());
            }
        }
        let ui: Element<App> = menu("Many actions", rows)
            .key("long-menu")
            .open(true)
            .on_action(|a: &mut App, v| a.actions.push(v))
            .on_open_change(|a: &mut App, v| a.menu = v)
            .into();
        let (mut r, mut h) = mount(ui, scale);
        let mut a = App {
            menu: true,
            ..App::default()
        };
        assert_eq!(r.focused(), Some("long-action-0"));
        r.overlay_key(SelectionKey::Last);
        layout(&mut r, scale);
        assert_eq!(r.focused(), Some("long-action-63"));
        assert!(r.boxes().iter().any(|b| b.id == "long-action-63"));
        r.focus_next(false);
        layout(&mut r, scale);
        assert_eq!(r.focused(), Some("long-action-0"));
        r.key_down();
        r.key_up_outcome(&mut h, &mut a).unwrap();
        assert_eq!(a.actions, vec![Action::Save]);
        assert!(!a.menu);
    }
}
#[test]
fn noninteractive_hover_tooltip_does_not_block_wheel_scroll_or_drag_regions() {
    let ui: Element<App> = vstack![
        tooltip("Header help", text("Drag this header").key("drag-label")).key("drag-help"),
        text("Large document").height(1800.),
    ]
    .key("scroll-root")
    .height(400.)
    .scroll(Scroll::Vertical)
    .drag_region(true)
    .into();
    let (mut r, _) = mount(ui, 1.);
    let [x, y] = point(&r, "drag-label");
    r.pointer_move(x, y);
    layout(&mut r, 1.);
    assert!(r.boxes().iter().any(|b| b.id == "drag-help-tooltip"));
    assert!(r.drag_region_hit(x, y));
    assert!(r.scroll_at(560., 300., [0., 120.]));
    assert!(r.scroll_offset("scroll-root")[1] > 0.);
}
#[test]
fn active_modal_hides_underlying_tooltip_and_prevents_underlying_context_requests() {
    let target: Element<App> = tooltip("Hidden help", contextual(&App::default()))
        .key("under-help")
        .into();
    let ui: Element<App> = zstack![
        target,
        dialog(
            button("Inside")
                .key("modal-inside")
                .on_click(|_: &mut App| {})
        )
        .key("blocking-modal")
        .open(true)
        .on_dismiss(|_: &mut App| {}),
    ]
    .align(Align::Stretch)
    .into();
    let (mut r, mut h) = mount(ui, 1.);
    let mut a = App::default();
    let [x, y] = point(&r, "document");
    r.pointer_move(x, y);
    layout(&mut r, 1.);
    assert!(!r.boxes().iter().any(|b| b.id == "under-help-tooltip"));
    assert!(!r.context_menu_at(x, y, &mut h, &mut a).unwrap().activated);
    assert!(!a.context);
}
#[test]
fn context_requests_and_disabled_menus_reject_stale_or_invalid_input() {
    let ui: Element<App> = menu("Disabled", [menu_item(Action::Save, "Save")])
        .key("disabled-menu")
        .enabled(false)
        .open(true)
        .on_action(|a: &mut App, v| a.actions.push(v))
        .on_open_change(|a: &mut App, v| a.menu = v)
        .into();
    let (r, _) = mount(ui, 1.);
    assert!(!r.boxes().iter().any(|b| b.id == "disabled-menu-popup"));
    let (mut r, mut h) = mount(contextual(&App::default()), 1.);
    let mut a = App::default();
    assert!(
        !r.context_menu_at(f32::NAN, 20., &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(
        !r.context_menu_at(590., 390., &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert!(!a.context);
    assert!(a.actions.is_empty());
}

#[test]
fn inactive_host_suppresses_keyboard_tooltip_and_restores_help_on_focus() {
    let (mut r, _) = mount(hint(), 1.);
    r.focus_next(false);
    layout(&mut r, 1.);
    assert!(r.boxes().iter().any(|b| b.id == "hint-tooltip"));
    assert!(r.set_tooltips_active(false));
    layout(&mut r, 1.);
    assert!(!r.boxes().iter().any(|b| b.id == "hint-tooltip"));
    assert!(r.set_tooltips_active(true));
    layout(&mut r, 1.);
    assert!(r.boxes().iter().any(|b| b.id == "hint-tooltip"));
}

#[test]
fn clipped_tooltip_and_context_targets_ignore_invisible_shape_corners() {
    for scale in [1., 2.] {
        let target = || {
            panel([text("Circular target").into()])
                .key("circle-target")
                .width(100.)
                .height(100.)
                .rounded(0)
                .clip(Clip::Circle)
        };
        let ui: Element<App> = tooltip("Only visible pixels", target())
            .key("circle-help")
            .into();
        let (mut r, _) = mount(ui, scale);
        let [x, y, _, _] = r
            .boxes()
            .iter()
            .find(|b| b.id == "circle-target")
            .unwrap()
            .bounds
            .bounds();
        r.pointer_move(x + 2. * scale, y + 2. * scale);
        layout(&mut r, scale);
        assert!(!r.boxes().iter().any(|b| b.id == "circle-help-tooltip"));
        let [cx, cy] = point(&r, "circle-target");
        r.pointer_move(cx, cy);
        layout(&mut r, scale);
        assert!(r.boxes().iter().any(|b| b.id == "circle-help-tooltip"));

        let ui: Element<App> = context_menu(target(), [menu_item(Action::Save, "Save")])
            .key("circle-context")
            .on_context_open(|a: &mut App, point| {
                a.context = true;
                a.point = point;
            })
            .on_action(|a: &mut App, action| a.actions.push(action))
            .on_open_change(|a: &mut App, open| a.context = open)
            .into();
        let (mut r, mut h) = mount(ui, scale);
        let mut app = App::default();
        let [x, y, _, _] = r
            .boxes()
            .iter()
            .find(|b| b.id == "circle-target")
            .unwrap()
            .bounds
            .bounds();
        assert!(
            !r.context_menu_at(x + 2. * scale, y + 2. * scale, &mut h, &mut app)
                .unwrap()
                .activated
        );
        let [cx, cy] = point(&r, "circle-target");
        assert!(
            r.context_menu_at(cx, cy, &mut h, &mut app)
                .unwrap()
                .activated
        );
    }
}

#[test]
fn popover_rejects_missing_and_self_referential_anchor_keys() {
    let ui: Element<App> = popover(text("Missing"))
        .key("missing-reference")
        .anchor("absent")
        .open(true)
        .on_dismiss(|_| {})
        .into();
    assert!(ui.build().is_err());
    let ui: Element<App> = popover(
        button("Own child")
            .key("self-anchor")
            .on_click(|_: &mut App| {}),
    )
    .key("self-reference")
    .anchor("self-anchor")
    .open(true)
    .on_dismiss(|_| {})
    .into();
    assert!(ui.build().is_err());
}

#[test]
fn tooltip_inside_modal_appears_for_initial_focus_without_another_event() {
    let ui: Element<App> = dialog(
        tooltip(
            "Inside help",
            button("Inside")
                .key("initial-target")
                .on_click(|_: &mut App| {}),
        )
        .key("initial-help"),
    )
    .key("initial-modal")
    .open(true)
    .on_dismiss(|_: &mut App| {})
    .into();
    let (r, _) = mount(ui, 1.);
    assert_eq!(r.focused(), Some("initial-target"));
    assert!(r.boxes().iter().any(|b| b.id == "initial-help-tooltip"));
}
#[test]
fn tooltip_flips_near_bottom_and_right_edges_instead_of_centering() {
    for scale in [1., 2.] {
        let ui: Element<App> = zstack![tooltip(
            "Near the edge",
            button("Edge")
                .key("edge-target")
                .width(90.)
                .height(36.)
                .on_click(|_: &mut App| {})
        )
        .key("edge-help")]
        .flow(Flow::BottomRight)
        .into();
        let (mut r, _) = mount(ui, scale);
        let [x, y] = point(&r, "edge-target");
        r.pointer_move(x, y);
        layout(&mut r, scale);
        let anchor = r
            .boxes()
            .iter()
            .find(|b| b.id == "edge-target")
            .unwrap()
            .bounds
            .bounds();
        let bubble = r
            .boxes()
            .iter()
            .find(|b| b.id == "edge-help-tooltip-content")
            .unwrap()
            .bounds
            .bounds();
        assert!(bubble[0] > 300. * scale && bubble[0] + bubble[2] <= 600. * scale + 0.01);
        assert!(bubble[1] + bubble[3] <= anchor[1] && bubble[1] >= 0.);
    }
}
fn nested_popover(app: &App) -> Element<App> {
    dialog(vstack![
        button("Details")
            .key("modal-anchor")
            .on_click(|a: &mut App| a.popover = true),
        popover(
            button("Nested action")
                .key("nested-popover-action")
                .on_click(|a: &mut App| a.clicks += 1)
        )
        .key("nested-popover")
        .anchor("modal-anchor")
        .open(app.popover)
        .on_dismiss(|a: &mut App| a.popover = false),
    ])
    .key("popover-host-modal")
    .open(true)
    .on_dismiss(|a: &mut App| a.clicks += 10)
    .into()
}
#[test]
fn nested_popover_traps_and_restores_parent_scope_before_modal_dismissal() {
    let mut a = App::default();
    let (mut r, mut h) = mount(nested_popover(&a), 1.);
    click(&mut r, &mut h, &mut a, "modal-anchor");
    replace(&mut r, &mut h, nested_popover(&a), 1.);
    assert_eq!(r.focused(), Some("nested-popover-action"));
    r.focus_next(false);
    layout(&mut r, 1.);
    assert_eq!(r.focused(), Some("nested-popover-action"));
    r.dismiss_overlay(&mut h, &mut a).unwrap();
    assert!(!a.popover);
    assert_eq!(a.clicks, 0);
    replace(&mut r, &mut h, nested_popover(&a), 1.);
    assert_eq!(r.focused(), Some("modal-anchor"));
    r.dismiss_overlay(&mut h, &mut a).unwrap();
    assert_eq!(a.clicks, 10);
}
