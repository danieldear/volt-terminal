#![cfg(feature = "rust-ui")]
//! The accessibility snapshot and actions a screen-reader adapter relies on.
use volt_ui_kit::{
    declarative::{AccessRole, AccessValue, Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, w: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(w), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    saves: usize,
    alerts: bool,
    name: String,
    secret: String,
    volume: f32,
    open: bool,
    window: std::ops::Range<usize>,
}

#[test]
fn explicit_names_and_composite_semantics_are_exposed() {
    let ui: Element<App> = vstack![
        text_field("Ada")
            .key("named")
            .accessible_label("Profile name")
            .placeholder("Your name")
            .on_change(|_: &mut App, _| {}),
        tab_bar([
            tab_item(0u8, "Overview").key("overview"),
            tab_item(1, "Details").key("details")
        ])
        .key("views")
        .selected(1)
        .on_select(|_: &mut App, _| {}),
        segmented_control([(0u8, "Low"), (1, "High")])
            .key("level")
            .selected(1)
            .on_select(|_: &mut App, _| {}),
        tree_view([tree_node(0u8, "src")
            .key("src")
            .children([tree_node(1, "lib.rs").key("lib")])])
        .key("files")
        .expanded([0])
        .selected(Some(1))
        .on_select(|_: &mut App, _| {})
        .on_expand(|_: &mut App, _, _| {}),
    ]
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(800., 700., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    let nodes = runtime.accessibility_nodes();
    let find = |id: &str| nodes.iter().find(|n| n.id == id).unwrap();
    assert_eq!(find("named").label, "Profile name");
    assert_eq!(find("named").placeholder.as_deref(), Some("Your name"));
    assert_eq!(find("views").role, AccessRole::TabList);
    assert_eq!(find("overview").role, AccessRole::Tab);
    assert_eq!(find("overview").selected, Some(false));
    assert_eq!(find("details").selected, Some(true));
    assert_eq!(find("level").role, AccessRole::RadioGroup);
    assert!(nodes
        .iter()
        .any(|n| n.role == AccessRole::RadioButton && n.checked == Some(true)));
    assert_eq!(find("src").expanded, Some(true));
    assert_eq!(find("lib").role, AccessRole::TreeItem);
    assert_eq!(find("lib").selected, Some(true));
    assert_eq!(find("src").level, Some(0));
    assert_eq!(find("lib").level, Some(1));
    assert_eq!(find("src-expand").label, "Collapse src");
}

#[test]
fn invalid_accessible_names_are_rejected_without_changing_painted_text() {
    for name in ["".to_owned(), "\0".to_owned(), "x".repeat(8193)] {
        let ui: Element<App> = button("Save")
            .accessible_label(name)
            .on_click(|_: &mut App| {})
            .into();
        assert!(ui.build().is_err());
    }
}

#[test]
fn clipped_virtual_viewport_can_page_without_losing_its_window_callback() {
    let ui: Element<App> = vstack![
        text("Above the list").height(500.),
        virtual_list(1000, 32., |i| data_item(i, format!("Row {i}"))
            .key(format!("off-{i}")))
        .key("offscreen-list")
        .height(128.)
        .window(0..7)
        .on_select(|_: &mut App, _| {})
        .on_window_change(|app: &mut App, range| app.window = range),
        button("Hidden")
            .key("zero")
            .width(0.)
            .on_click(|app: &mut App| app.saves += 1),
    ]
    .key("outer")
    .height(160.)
    .scroll(Scroll::Vertical)
    .into();
    let (doc, mut handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(500., 300., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    assert!(!runtime.boxes().iter().any(|b| b.id == "offscreen-list"));
    let mut app = App::default();
    let outcome = runtime
        .accessibility_scroll("offscreen-list", [0, 1], &mut handlers, &mut app)
        .unwrap();
    assert!(outcome.changed && outcome.activated);
    assert!(app.window.start > 0);
    assert!(!runtime.accessibility_focus("zero"));
    assert!(
        !runtime
            .accessibility_activate("zero", &mut handlers, &mut app)
            .unwrap()
            .changed
    );
    assert_eq!(app.saves, 0);
}

#[test]
fn scroll_actions_page_and_clamp_virtual_windows_and_respect_modal_scope() {
    let scrolling_view = |open: bool| -> Element<App> {
        vstack![
            virtual_list(1000, 32., |i| data_item(i, format!("Row {i}"))
                .key(format!("record-{i}")))
            .key("records")
            .height(128.)
            .window(0..7)
            .on_select(|_: &mut App, _| {})
            .on_window_change(|app: &mut App, window| app.window = window),
            dialog(button("Close").key("close").on_click(|_: &mut App| {}))
                .key("modal")
                .open(open)
                .on_dismiss(|_: &mut App| {}),
        ]
        .into()
    };
    let (doc, mut handlers) = scrolling_view(false).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let mut app = App::default();
    let viewport = Viewport::new(600., 500., 2.).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut Metrics)
        .unwrap();
    let nodes = runtime.accessibility_nodes();
    assert_eq!(
        nodes
            .iter()
            .find(|n| n.id == "records")
            .unwrap()
            .size_of_set,
        Some(1000)
    );
    assert_eq!(
        nodes
            .iter()
            .find(|n| n.id == "record-6")
            .unwrap()
            .position_in_set,
        Some(6)
    );
    let outcome = runtime
        .accessibility_scroll("records", [0, 1], &mut handlers, &mut app)
        .unwrap();
    assert!(outcome.changed && outcome.activated);
    assert!(runtime.scroll_offset("records")[1] > 0.);
    assert!(app.window.start > 0);
    let before = runtime.scroll_offset("records");
    for (id, direction) in [
        ("records", [2, 0]),
        ("records", [1, 0]),
        ("missing", [0, 1]),
    ] {
        assert!(
            !runtime
                .accessibility_scroll(id, direction, &mut handlers, &mut app)
                .unwrap()
                .changed
        );
        assert_eq!(runtime.scroll_offset("records"), before);
    }
    runtime
        .accessibility_scroll("records", [0, -1], &mut handlers, &mut app)
        .unwrap();
    assert_eq!(runtime.scroll_offset("records"), [0.; 2]);
    assert!(
        !runtime
            .accessibility_scroll("records", [0, -1], &mut handlers, &mut app)
            .unwrap()
            .changed
    );
    // Open-modal background controls must not scroll or reveal/focus.
    let (doc, next) = scrolling_view(true).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    handlers = next;
    runtime
        .layout(viewport, &Bindings::default(), &mut Metrics)
        .unwrap();
    assert!(!runtime.accessibility_reveal("record-0"));
    assert!(!runtime.accessibility_focus("record-0"));
    assert!(
        !runtime
            .accessibility_scroll("records", [0, 1], &mut handlers, &mut app)
            .unwrap()
            .changed
    );
}

#[test]
fn accessibility_can_focus_and_reveal_a_mounted_offscreen_control() {
    let ui: Element<App> = vstack![
        text("Above").height(280.),
        button("Last action").key("last").on_click(|_: &mut App| {}),
    ]
    .key("scroll")
    .height(100.)
    .scroll(Scroll::Vertical)
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let viewport = Viewport::new(400., 150., 2.).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut Metrics)
        .unwrap();
    assert!(runtime.accessibility_nodes().iter().any(|n| n.id == "last"));
    assert!(runtime.accessibility_focus("last"));
    assert!(runtime.scroll_offset("scroll")[1] > 0.);
    runtime
        .layout(viewport, &Bindings::default(), &mut Metrics)
        .unwrap();
    assert!(runtime.boxes().iter().any(|b| b.id == "last"));
    assert!(
        !runtime.accessibility_focus("last"),
        "already revealed focus is a no-op"
    );
}

#[test]
fn table_hit_targets_have_row_names() {
    let ui: Element<App> = table(
        [
            table_column(0u8, "Name").sortable(false),
            table_column(1, "Status").sortable(false),
        ],
        [table_row(0u8, ["Ada", "Ready"]).key("row-ada")],
    )
    .key("table")
    .on_select(|_: &mut App, _| {})
    .into();
    let (doc, handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(600., 300., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    let nodes = runtime.accessibility_nodes();
    assert_eq!(
        nodes.iter().find(|n| n.id == "row-ada").unwrap().label,
        "Ada, Ready"
    );
}
fn view(app: &App) -> Element<App> {
    vstack![
        text("Profile"),
        button("Save")
            .key("save")
            .on_click(|app: &mut App| app.saves += 1),
        toggle("Alerts", app.alerts)
            .key("alerts")
            .on_change(|app: &mut App, v| app.alerts = v),
        text_field(&app.name)
            .key("name")
            .placeholder("Your name")
            .on_change(|app: &mut App, v| app.name = v),
        text_field(&app.secret)
            .key("secret")
            .placeholder("Password")
            .password(true)
            .on_change(|app: &mut App, v| app.secret = v),
        slider(app.volume)
            .key("volume")
            .range(0.0..=10.0)
            .step(1.)
            .on_change(|app: &mut App, v| app.volume = v),
        dialog(
            button("Close")
                .key("close")
                .on_click(|app: &mut App| app.open = false)
        )
        .key("modal")
        .open(app.open)
        .on_dismiss(|app: &mut App| app.open = false),
    ]
    .into()
}
fn mount(app: &App) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = view(app).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(600., 800., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    (runtime, handlers)
}

#[test]
fn invalid_accessibility_actions_do_not_change_focus_or_call_handlers() {
    let mut app = App::default();
    let (mut runtime, mut handlers) = mount(&app);
    runtime.accessibility_focus("save");
    assert!(runtime
        .accessibility_activate("name", &mut handlers, &mut app)
        .is_ok());
    for (id, value) in [
        ("name", AccessValue::Text("x".repeat(8193))),
        ("save", AccessValue::Number(1.)),
        ("volume", AccessValue::Number(f32::NAN)),
        ("volume", AccessValue::Text("wrong type".into())),
        ("name", AccessValue::Text("\t".repeat(3000))),
        ("name", AccessValue::Text("\0\u{1}".into())),
    ] {
        let result = runtime
            .accessibility_set_value(id, value, &mut handlers, &mut app)
            .unwrap();
        assert!(!result.changed && !result.activated);
        assert!(runtime
            .accessibility_nodes()
            .iter()
            .any(|n| n.id == "save" && n.focused));
    }
    assert!(
        !runtime
            .accessibility_step("name", true, &mut handlers, &mut app)
            .unwrap()
            .changed
    );
    assert_eq!(app.saves, 0);
    assert!(app.name.is_empty());
    assert_eq!(app.volume, 0.);
}

#[test]
fn rejected_text_requests_preserve_editor_selection_and_read_only_state() {
    let mut app = App {
        name: "Ada".into(),
        ..Default::default()
    };
    let (mut runtime, mut handlers) = mount(&app);
    runtime.accessibility_focus("name");
    let before = runtime.editor_selection("name");
    for text in ["x".repeat(8193), "\t".repeat(3000), "\0".into()] {
        let result = runtime
            .accessibility_set_value("name", AccessValue::Text(text), &mut handlers, &mut app)
            .unwrap();
        assert!(!result.changed && !result.activated);
        assert_eq!(runtime.editor_selection("name"), before);
        assert_eq!(app.name, "Ada");
    }
    let ui: Element<App> = text_field("Locked")
        .key("locked")
        .read_only(true)
        .on_change(|app: &mut App, value| app.name = value)
        .into();
    let (doc, mut h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(
        !r.accessibility_set_value(
            "locked",
            AccessValue::Text("overwrite".into()),
            &mut h,
            &mut app
        )
        .unwrap()
        .changed
    );
    assert_eq!(app.name, "Ada");
}
fn rebuild(app: &App, runtime: &mut Runtime, handlers: &mut Handlers<'static, App>) {
    let (document, next) = view(app).build().unwrap().into_parts();
    runtime
        .replace_document(document, &next, &Bindings::default())
        .unwrap();
    *handlers = next;
    runtime
        .layout(
            Viewport::new(600., 800., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
}

#[test]
fn roles_labels_states_and_hidden_passwords() {
    let app = App {
        secret: "hunter2".into(),
        volume: 3.,
        ..Default::default()
    };
    let (runtime, _) = mount(&app);
    let nodes = runtime.accessibility_nodes();
    let find = |id: &str| {
        nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("{id}"))
    };
    assert!(nodes
        .iter()
        .any(|n| n.role == AccessRole::Label && n.label == "Profile"));
    assert_eq!(find("save").role, AccessRole::Button);
    assert_eq!(find("save").label, "Save");
    assert_eq!(find("alerts").role, AccessRole::Switch);
    assert_eq!(find("alerts").checked, Some(false));
    assert_eq!(find("name").role, AccessRole::TextInput);
    assert_eq!(find("name").label, "Your name");
    let secret = find("secret");
    assert!(secret.password);
    assert_eq!(
        secret.value, None,
        "password text and length are never exposed"
    );
    assert_eq!(find("name").value.as_deref(), Some(""));
    assert_eq!(find("volume").role, AccessRole::Slider);
    assert_eq!(find("volume").numeric, Some([3., 0., 10., 1.]));
    // Closed dialogs expose nothing.
    assert!(nodes.iter().all(|n| n.id != "close"));
    assert!(nodes.iter().all(|n| n.bounds[2] > 0. && n.bounds[3] > 0.));
}

#[test]
fn screen_reader_actions_reach_application_state() {
    let mut app = App::default();
    let (mut runtime, mut handlers) = mount(&app);
    assert!(
        runtime
            .accessibility_activate("save", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.saves, 1);
    assert_eq!(runtime.focused(), Some("save"));
    runtime
        .accessibility_activate("alerts", &mut handlers, &mut app)
        .unwrap();
    assert!(app.alerts);
    runtime
        .accessibility_set_value(
            "name",
            AccessValue::Text("Ada".into()),
            &mut handlers,
            &mut app,
        )
        .unwrap();
    assert_eq!(app.name, "Ada");
    rebuild(&app, &mut runtime, &mut handlers);
    runtime
        .accessibility_set_value("volume", AccessValue::Number(6.4), &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.volume, 6., "values snap to the slider step");
    rebuild(&app, &mut runtime, &mut handlers);
    runtime
        .accessibility_step("volume", true, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.volume, 7.);
    // Focus moves only to reachable, enabled controls.
    assert!(!runtime.accessibility_focus("missing"));
}

#[test]
fn an_open_modal_hides_the_content_behind_it() {
    let mut app = App {
        open: true,
        ..Default::default()
    };
    let (mut runtime, mut handlers) = mount(&app);
    let ids: Vec<_> = runtime
        .accessibility_nodes()
        .into_iter()
        .map(|n| n.id)
        .collect();
    assert!(ids.contains(&"close".to_string()));
    assert!(
        !ids.contains(&"save".to_string()),
        "content behind the modal is inert"
    );
    // Actions outside the modal are refused, like pointer and keyboard input.
    assert!(
        !runtime
            .accessibility_activate("save", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.saves, 0);
}

/// The node budget admits dense real screens (forms, tables, dashboards).
#[test]
fn four_thousand_node_screens_build_and_lay_out() {
    let rows: Vec<Element<App>> = (0..1300)
        .map(|i| {
            hstack![
                text(format!("Row {i}")),
                button("Edit")
                    .key(format!("edit-{i}"))
                    .on_click(|app: &mut App| app.saves += 1),
            ]
            .into()
        })
        .collect();
    let deep = (0..40).fold(Element::from(text("leaf")), |inner, _| {
        vstack![inner].into()
    });
    let ui: Element<App> = vstack![vstack(rows).scroll(Scroll::Vertical).height(600.), deep].into();
    let start = std::time::Instant::now();
    let (document, handlers) = ui
        .build()
        .expect("4k nodes and depth 40 are within limits")
        .into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(800., 900., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    // Rows scrolled out of view are laid out but not kept as visible boxes.
    assert!(runtime.boxes().iter().any(|b| b.id == "edit-0"));
    eprintln!(
        "4k-node build+layout: {:.1}ms",
        start.elapsed().as_secs_f64() * 1000.
    );
    // Past the budget, building fails with an error instead of degrading.
    let too_many: Vec<Element<App>> = (0..4200).map(|i| text(format!("{i}")).into()).collect();
    assert!(Element::<App>::from(vstack(too_many)).build().is_err());
}
