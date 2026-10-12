#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, SelectionKey, TextMetrics},
    ui::prelude::*,
    Painter, Viewport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Overview,
    Activity,
    Settings,
}
struct App {
    page: Page,
    changes: usize,
}

#[test]
fn typed_tabs_only_install_active_content_and_callbacks() {
    let app = App {
        page: Page::Overview,
        changes: 0,
    };
    let ui: Element<App> = tabs![
        tab(
            Page::Overview,
            "Overview",
            button("Live")
                .key("live")
                .on_click(|app: &mut App| app.changes += 1)
        ),
        tab(
            Page::Activity,
            "Activity",
            button("Hidden")
                .key("hidden")
                .on_click(|app: &mut App| app.changes += 10)
        ),
    ]
    .key("pages")
    .selected(app.page)
    .on_select(|app: &mut App, page| app.page = page)
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    fn has(n: &volt_ui_kit::declarative::Node, id: &str) -> bool {
        n.id() == id || n.children().iter().any(|n| has(n, id))
    }
    assert!(has(document.root(), "live"));
    assert!(!has(document.root(), "hidden"));
}

#[test]
fn selections_reject_duplicates_missing_selection_and_missing_handlers() {
    let make = || segmented_control::<App, _>([(Page::Overview, "One"), (Page::Activity, "Two")]);
    assert!(
        Element::from(make().selected(Page::Settings).on_select(|_, _| {}))
            .build()
            .is_err()
    );
    assert!(Element::from(make()).build().is_err());
    assert!(Element::from(
        segmented_control::<App, _>([(Page::Overview, "One"), (Page::Overview, "Two")])
            .on_select(|_, _| {})
    )
    .build()
    .is_err());
}

#[test]
fn bars_keep_callbacks_and_validate_layout() {
    let ui: Element<App> = toolbar![
        text("Workspace"),
        spacer(),
        button_bar![button("Save")
            .key("save")
            .on_click(|app: &mut App| app.changes += 1)]
        .joined(true)
        .rounded(Rounding::Full)
        .gap(8., Axis::Horizontal),
    ]
    .padding(12., [Edge::Horizontal])
    .into();
    let (mut runtime, mut handlers) = mount(ui, 1.);
    let mut app = App {
        page: Page::Overview,
        changes: 0,
    };
    click(&mut runtime, &mut handlers, &mut app, "save");
    assert_eq!(app.changes, 1);
    assert!(
        !runtime
            .selection_key(SelectionKey::Next, &mut handlers, &mut app)
            .unwrap()
            .changed
    );
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
        Viewport::new(600. * self.0, 600. * self.0, self.0).unwrap()
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
fn mount(element: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let mut painter = Metrics(scale);
    runtime
        .layout(painter.viewport(), &Bindings::default(), &mut painter)
        .unwrap();
    (runtime, handlers)
}
fn click(
    runtime: &mut Runtime,
    handlers: &mut Handlers<'static, App>,
    app: &mut App,
    key: &str,
) -> bool {
    let [x, y, w, h] = runtime
        .boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap()
        .bounds
        .bounds();
    runtime.pointer_down(x + w / 2., y + h / 2.);
    runtime
        .pointer_up_outcome(x + w / 2., y + h / 2., handlers, app)
        .unwrap()
        .activated
}
fn selection(app: &App) -> Element<App> {
    tab_bar![
        tab_item(Page::Overview, "Overview").key("overview"),
        tab_item(Page::Settings, "Unavailable")
            .key("disabled")
            .enabled(false),
        tab_item(Page::Activity, "Activity").key("activity"),
    ]
    .key("nav")
    .selected(app.page)
    .on_select(|app: &mut App, value| {
        app.page = value;
        app.changes += 1;
    })
    .into()
}
fn recompose(runtime: &mut Runtime, handlers: &mut Handlers<'static, App>, app: &App, scale: f32) {
    let (document, next) = selection(app).build().unwrap().into_parts();
    runtime
        .replace_document(document, &next, &Bindings::default())
        .unwrap();
    *handlers = next;
    let mut painter = Metrics(scale);
    runtime
        .layout(painter.viewport(), &Bindings::default(), &mut painter)
        .unwrap();
}
#[test]
fn selection_pointer_and_keyboard_skip_disabled_and_do_not_reselect() {
    for scale in [1., 2.] {
        let mut app = App {
            page: Page::Overview,
            changes: 0,
        };
        let (mut r, mut h) = mount(selection(&app), scale);
        assert!(!click(&mut r, &mut h, &mut app, "overview"));
        assert_eq!(app.changes, 0);
        assert!(
            r.selection_key(SelectionKey::Next, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.page, Page::Activity);
        assert_eq!(app.changes, 1);
        recompose(&mut r, &mut h, &app, scale);
        assert_eq!(r.focused(), Some("activity"));
        assert!(
            !r.selection_key(SelectionKey::Last, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.changes, 1);
        assert!(
            r.selection_key(SelectionKey::Next, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.page, Page::Overview);
        recompose(&mut r, &mut h, &app, scale);
        assert!(
            r.selection_key(SelectionKey::Previous, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.page, Page::Activity);
        recompose(&mut r, &mut h, &app, scale);
        assert!(
            r.selection_key(SelectionKey::First, &mut h, &mut app)
                .unwrap()
                .activated
        );
        recompose(&mut r, &mut h, &app, scale);
        assert!(!click(&mut r, &mut h, &mut app, "disabled"));
        assert_eq!(app.page, Page::Overview);
    }
}
#[test]
fn bottom_tabs_mount_only_selected_page_and_preserve_callback_types() {
    let mut app = App {
        page: Page::Activity,
        changes: 0,
    };
    let ui = tabs![
        tab(
            Page::Overview,
            "Overview",
            button("Gone")
                .key("gone")
                .on_click(|app: &mut App| app.changes += 100)
        ),
        tab(
            Page::Activity,
            "Activity",
            button("Live")
                .key("live")
                .on_click(|app: &mut App| app.changes += 1)
        ),
    ]
    .key("pages")
    .selected(app.page)
    .placement(TabPlacement::Bottom)
    .on_select(|app: &mut App, value| app.page = value)
    .into();
    let (mut r, mut h) = mount(ui, 1.);
    let children = r.document().root().children();
    assert_eq!(children[0].id(), "live");
    assert_eq!(children[1].id(), "pages-strip");
    assert!(click(&mut r, &mut h, &mut app, "live"));
    assert_eq!(app.changes, 1);
    assert!(!r.boxes().iter().any(|b| b.id == "gone"));
}
#[test]
fn all_disabled_group_cannot_dispatch_or_focus() {
    let mut app = App {
        page: Page::Overview,
        changes: 0,
    };
    let ui = segmented_control([(Page::Overview, "One"), (Page::Activity, "Two")])
        .enabled(false)
        .on_select(|app: &mut App, _| app.changes += 1)
        .into();
    let (mut r, mut h) = mount(ui, 1.);
    assert!(!r.focus_next(false));
    assert!(
        !r.selection_key(SelectionKey::Next, &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.changes, 0);
}
#[test]
fn empty_and_invalid_tabs_fail_before_installation() {
    assert!(Element::from(tab_bar::<App, Page>([]).on_select(|_, _| {}))
        .build()
        .is_err());
    assert!(Element::from(tabs::<App, Page>([]).on_select(|_, _| {}))
        .build()
        .is_err());
    assert!(Element::from(
        tabs![tab::<App, _>(Page::Overview, "One", text("One"))]
            .selected(Page::Settings)
            .on_select(|_, _| {})
    )
    .build()
    .is_err());
    assert!(Element::from(
        tabs![
            tab::<App, _>(Page::Overview, "One", text("One")),
            tab(Page::Overview, "Again", text("Again"))
        ]
        .on_select(|_, _| {})
    )
    .build()
    .is_err());
}
#[test]
fn legacy_json_schema_does_not_gain_rust_composite_fields() {
    assert!(volt_ui_kit::declarative::Document::parse(
        r#"{"type":"row","id":"r","selection_group":true,"children":[]}"#
    )
    .is_err());
}
