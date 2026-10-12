#![cfg(feature = "rust-ui")]
use std::sync::{Arc, Mutex};
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Kind, Runtime, TextMetrics},
    ui::{column, prelude::*},
    Painter, Theme, Viewport,
};

#[derive(Default)]
struct App {
    count: usize,
    enabled: bool,
    items: Vec<usize>,
}
struct RecordingPainter;
impl TextMetrics for RecordingPainter {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(width),
            size * 1.2,
        ]
    }
}
impl Painter for RecordingPainter {
    type Shape = ();
    type Glyph = String;
    fn viewport(&self) -> Viewport {
        Viewport::new(600., 600., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(
        &mut self,
        out: &mut Vec<String>,
        text: &str,
        _: f32,
        _: f32,
        _: f32,
        _: [f32; 4],
    ) {
        out.push(text.into());
    }
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<String>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl DocumentPainter for RecordingPainter {
    fn ui_text(&mut self, out: &mut Vec<String>, text: &str, _: [f32; 4], _: f32, _: [f32; 4]) {
        out.push(text.into());
    }
}
fn runtime(element: Element<App>) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(600., 600., 1.).unwrap(),
            &Bindings::default(),
            &mut RecordingPainter,
        )
        .unwrap();
    (runtime, handlers)
}
fn activate(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, id: &str) {
    let [x, y, w, height] = r
        .boxes()
        .iter()
        .find(|node| node.id == id)
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down(x + w * 0.5, y + height * 0.5);
    r.pointer_up(x + w * 0.5, y + height * 0.5, h, app).unwrap();
}
fn item(id: usize) -> Element<App> {
    button(format!("Item {id}"))
        .key(format!("item-{id}"))
        .on_click(move |app: &mut App| app.count += id)
        .into()
}
fn view(app: &App) -> Element<App> {
    let items: Vec<Element<App>> = app.items.iter().map(|id| item(*id)).collect();
    column![
        text(format!("Count: {}", app.count)).key("count"),
        panel![row![
            button("Add")
                .key("add")
                .on_click(|app: &mut App| app.count += 1),
            toggle("Enable", app.enabled)
                .key("enabled")
                .on_change(|app: &mut App, value| app.enabled = value),
            checkbox("Also enable", app.enabled)
                .key("check")
                .on_change(|app: &mut App, value| app.enabled = value),
        ]]
        .padding(12., Edge::All)
        .gap(8., Axis::Both),
        if app.enabled {
            Element::from(badge("Enabled"))
        } else {
            Element::from(text("Disabled"))
        },
        column(items).key("items"),
        separator(),
    ]
    .key("root")
    .padding(16., Edge::All)
    .gap(12., Axis::Both)
    .font_family("system")
    .font_size(14.)
    .into()
}
#[test]
fn typed_nested_composition_and_callbacks_rebuild_visible_state() {
    let mut app = App {
        items: vec![2, 3],
        ..App::default()
    };
    let (mut r, mut h) = runtime(view(&app));
    assert_eq!(r.document().root().kind(), Kind::Column);
    assert!(r.boxes().iter().any(|b| b.kind == Kind::Panel));
    activate(&mut r, &mut h, &mut app, "add");
    assert_eq!(app.count, 1);
    activate(&mut r, &mut h, &mut app, "enabled");
    assert!(app.enabled);
    let (new_r, _) = runtime(view(&app));
    assert!(new_r.boxes().iter().any(|b| b.kind == Kind::Badge));
    let texts: Vec<_> = new_r
        .paint(&mut RecordingPainter, &Theme::dark())
        .into_iter()
        .flat_map(|batch| batch.glyphs)
        .collect();
    assert!(texts.contains(&"Count: 1".into()));
    activate(&mut r, &mut h, &mut app, "item-3");
    assert_eq!(app.count, 4);
}
#[test]
fn owned_captures_functions_and_typed_bool_callbacks_are_connected_directly() {
    fn increment(app: &mut App) {
        app.count += 1;
    }
    let calls = Arc::new(Mutex::new(0));
    let captured = calls.clone();
    let element = column![
        button("Function").key("function").on_click(increment),
        button("Capture")
            .key("capture")
            .on_click(move |app: &mut App| {
                *captured.lock().unwrap() += 1;
                app.count += 7;
            }),
        checkbox("Enabled", false)
            .key("bool")
            .on_change(|app: &mut App, value| app.enabled = value)
    ]
    .into();
    let (mut r, mut h) = runtime(element);
    let mut app = App::default();
    activate(&mut r, &mut h, &mut app, "function");
    activate(&mut r, &mut h, &mut app, "capture");
    activate(&mut r, &mut h, &mut app, "bool");
    assert_eq!(app.count, 8);
    assert!(app.enabled);
    assert_eq!(*calls.lock().unwrap(), 1);
}
#[test]
fn keyed_repeated_component_ids_survive_reordering_and_structural_ids_repeat() {
    let app = App {
        items: vec![2, 3],
        ..App::default()
    };
    let (first, _) = runtime(view(&app));
    let app = App {
        items: vec![3, 2, 4],
        ..App::default()
    };
    let (second, _) = runtime(view(&app));
    for id in ["root", "items", "item-2", "item-3"] {
        assert!(first.boxes().iter().any(|b| b.id == id));
        assert!(second.boxes().iter().any(|b| b.id == id));
    }
    let static_tree = || column![text("A"), panel![text("B"), separator()]].into();
    let (a, _) = runtime(static_tree());
    let (b, _) = runtime(static_tree());
    assert_eq!(
        a.boxes().iter().map(|b| b.id.clone()).collect::<Vec<_>>(),
        b.boxes().iter().map(|b| b.id.clone()).collect::<Vec<_>>()
    );
}
#[test]
fn validation_rejects_duplicate_keys_missing_callbacks_limits_and_bad_styles() {
    let duplicate: Element<App> =
        column![text("A").key("duplicate"), text("B").key("duplicate")].into();
    assert!(duplicate.build().is_err());
    assert!(Element::<App>::from(text("A").key("bad key"))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("A").key("")).build().is_err());
    assert!(Element::<App>::from(button("Missing callback"))
        .build()
        .is_err());
    assert!(Element::<App>::from(toggle("Missing callback", false))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("A").color("not hex"))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("A").font_weight(450))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("A").font_size(1.))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("A").width(f32::NAN))
        .build()
        .is_err());
    assert!(Element::<App>::from(text("a".repeat(8193)))
        .build()
        .is_err());
    // Document limits: 4096 nodes, depth 48.
    let nodes: Vec<Element<App>> = (0..4097).map(|i| text(i.to_string()).into()).collect();
    assert!(Element::from(column(nodes)).build().is_err());
    let mut deep: Element<App> = text("Leaf").into();
    for _ in 0..50 {
        deep = column(vec![deep]).into();
    }
    assert!(deep.build().is_err());
}
#[test]
fn typed_styles_match_shared_runtime_and_disabled_controls_do_not_fire() {
    let element: Element<App> = column![
        text("Bold")
            .key("bold")
            .font_size(20.)
            .font_weight(700)
            .font_style(FontStyle::Italic)
            .color("#aabbcc"),
        button("Disabled")
            .key("disabled")
            .enabled(false)
            .on_click(|app: &mut App| app.count += 1)
    ]
    .font_family("serif")
    .background("#112233")
    .gap(8., Axis::Both)
    .align(Align::Start)
    .into();
    let (mut r, mut h) = runtime(element);
    let mut app = App::default();
    activate(&mut r, &mut h, &mut app, "disabled");
    assert_eq!(app.count, 0);
    let style = &r
        .boxes()
        .iter()
        .find(|b| b.id == "bold")
        .unwrap()
        .text_style;
    assert_eq!(style.family, "serif");
    assert_eq!(style.size, 20.);
    assert_eq!(style.weight, 700);
    assert!(style.italic);
}

#[test]
fn replacement_preserves_exact_keyed_kind_and_cancels_transient_input() {
    let tree = || {
        column![
            button("A")
                .key("a")
                .on_click(|app: &mut App| app.count += 1),
            button("B")
                .key("b")
                .on_click(|app: &mut App| app.count += 10),
        ]
        .into()
    };
    let (mut r, mut handlers) = runtime(tree());
    r.focus_next(false);
    r.key_down();
    let next: Element<App> = column![
        button("B")
            .key("b")
            .on_click(|app: &mut App| app.count += 10),
        button("A")
            .key("a")
            .on_click(|app: &mut App| app.count += 1),
    ]
    .into();
    let (doc, next_handlers) = next.build().unwrap().into_parts();
    r.replace_document(doc, &next_handlers, &Bindings::default())
        .unwrap();
    assert_eq!(r.focused(), Some("a"));
    // A second replacement before layout must not rely on stale/cleared boxes.
    let (doc, next_handlers) = tree().build().unwrap().into_parts();
    r.replace_document(doc, &next_handlers, &Bindings::default())
        .unwrap();
    assert_eq!(r.focused(), Some("a"));
    assert!(
        !r.key_up_outcome(&mut handlers, &mut App::default())
            .unwrap()
            .activated
    );
    let changed_kind: Element<App> = toggle("A", false)
        .key("a")
        .on_change(|app: &mut App, v| app.enabled = v)
        .into();
    let (doc, next_handlers) = changed_kind.build().unwrap().into_parts();
    r.replace_document(doc, &next_handlers, &Bindings::default())
        .unwrap();
    assert_eq!(r.focused(), None);
}
#[test]
fn replacement_clears_removed_and_disabled_focus_but_invalid_candidate_is_atomic() {
    let tree = || {
        button("A")
            .key("a")
            .on_click(|app: &mut App| app.count += 1)
            .into()
    };
    let (mut r, mut handlers) = runtime(tree());
    let bounds = r.boxes().iter().find(|b| b.id == "a").unwrap().bounds;
    let [x, y, w, h] = bounds.bounds();
    let (x, y) = (x + w * 0.5, y + h * 0.5);
    r.pointer_move(x, y);
    r.pointer_down(x, y);
    let (candidate, _) = tree().build().unwrap().into_parts();
    let missing = Handlers::<App>::default();
    assert!(r
        .replace_document(candidate, &missing, &Bindings::default())
        .is_err());
    assert_eq!(r.boxes().len(), 1);
    assert_eq!(r.focused(), Some("a"));
    assert!(r.button_state("a").pressed);
    let mut app = App::default();
    assert!(
        r.pointer_up_outcome(x, y, &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.count, 1);
    let disabled: Element<App> = button("A")
        .key("a")
        .enabled(false)
        .on_click(|_: &mut App| {})
        .into();
    let (doc, h) = disabled.build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    assert_eq!(r.focused(), None);
    let (mut r, _) = runtime(tree());
    r.focus_next(false);
    let removed: Element<App> = text("Gone").into();
    let (doc, h) = removed.build().unwrap().into_parts();
    r.replace_document(doc, &h, &Bindings::default()).unwrap();
    assert_eq!(r.focused(), None);
}
#[test]
fn input_outcome_distinguishes_cancelled_release_and_real_callback() {
    let (mut r, mut h) = runtime(
        button("A")
            .key("a")
            .on_click(|app: &mut App| app.count += 1)
            .into(),
    );
    let mut app = App::default();
    r.pointer_down(10., 10.);
    let outcome = r.pointer_up_outcome(900., 900., &mut h, &mut app).unwrap();
    assert!(outcome.changed);
    assert!(!outcome.activated);
    assert_eq!(app.count, 0);
    assert_eq!(
        r.pointer_up_outcome(10., 10., &mut h, &mut app).unwrap(),
        Default::default()
    );
    r.key_down();
    assert!(r.key_up_outcome(&mut h, &mut app).unwrap().activated);
    assert_eq!(app.count, 1);
    r.key_down();
    r.cancel_input();
    assert!(!r.key_up_outcome(&mut h, &mut app).unwrap().activated);
    r.pointer_down(10., 10.);
    assert!(r.pointer_up(900., 900., &mut h, &mut app).unwrap()); // legacy bool semantics
    assert_eq!(app.count, 1);
}
