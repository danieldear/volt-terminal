#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, EditKey, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Plan {
    #[default]
    Free,
    Pro,
    Team,
}
#[derive(Default)]
struct App {
    plan: Plan,
    quantity: f64,
    fruit: Option<u8>,
    combo: ComboboxState,
    tags: Vec<String>,
    tag_state: TagInputState,
    colour: String,
    code: String,
    completed: Option<String>,
    stars: u8,
    search: String,
    cleared: usize,
}
fn view(app: &App) -> Element<App> {
    vstack![
        radio_group([
            (Plan::Free, "Free"),
            (Plan::Pro, "Pro"),
            (Plan::Team, "Team")
        ])
        .key("plan")
        .selected(app.plan)
        .on_select(|app: &mut App, p| app.plan = p),
        number_input(app.quantity)
            .key("qty")
            .range(0., 3.)
            .step(1.)
            .on_change(|app: &mut App, v| app.quantity = v),
        combobox([(1u8, "Apple"), (2, "Banana"), (3, "Apricot")], &app.combo)
            .key("fruit")
            .selected(app.fruit)
            .state(|app: &mut App| &mut app.combo)
            .on_select(|app: &mut App, v| app.fruit = Some(v)),
        tag_input(&app.tags, &app.tag_state)
            .key("tags")
            .state(|app: &mut App| &mut app.tag_state)
            .on_change(|app: &mut App, t| app.tags = t),
        color_picker(app.colour.clone())
            .key("colour")
            .on_change(|app: &mut App, c| app.colour = c),
        otp_input(app.code.clone(), 4)
            .key("otp")
            .on_change(|app: &mut App, c| app.code = c)
            .on_complete(|app: &mut App, c| app.completed = Some(c)),
        rating(app.stars)
            .key("stars")
            .on_change(|app: &mut App, n| app.stars = n),
        form_field(
            "Search",
            text_field(app.search.clone())
                .key("search")
                .leading_icon(Icon::Search)
                .trailing_action("search-clear", Icon::Close, "Clear", |app: &mut App| {
                    app.search.clear();
                    app.cleared += 1;
                })
                .on_change(|app: &mut App, v| app.search = v),
        )
        .key("search-field")
        .required(true)
        .error((app.search.len() > 5).then(|| "Too long".to_owned())),
    ]
    .key("page")
    .gap(12., Axis::Vertical)
    .into()
}
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App {
            colour: "#3B82F6".into(),
            ..Default::default()
        };
        let (d, handlers) = view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
        let mut h = Self {
            app,
            runtime,
            handlers,
        };
        h.layout();
        h
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                Viewport::new(900., 2000., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn rebuild(&mut self) {
        let (d, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(d, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
    fn press(&mut self, id: &str) -> bool {
        let b = self
            .runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .clone();
        let [x, y, w, h] = b.bounds.bounds();
        let (x, y) = (x + w * 0.5, y + h * 0.5);
        self.runtime.pointer_down(x, y);
        let outcome = self
            .runtime
            .pointer_up_outcome(x, y, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
        outcome.activated
    }
    fn click(&mut self, id: &str) {
        assert!(self.press(id), "{id} did not activate");
    }
    fn type_into(&mut self, id: &str, text: &str) {
        self.press(id);
        assert_eq!(self.runtime.focused(), Some(id), "{id} not focused");
        self.runtime
            .text_input(text, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
    }
    fn enter(&mut self) {
        self.runtime
            .editor_key(EditKey::Enter, false, &mut self.handlers, &mut self.app)
            .unwrap();
        self.rebuild();
    }
    fn has(&self, id: &str) -> bool {
        self.runtime.boxes().iter().any(|b| b.id == id)
    }
    fn enabled(&self, id: &str) -> bool {
        self.runtime
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .enabled
    }
}

#[test]
fn radio_group_selects_one_option() {
    let mut h = Harness::new();
    h.click("plan-2");
    assert_eq!(h.app.plan, Plan::Team);
    h.click("plan-1");
    assert_eq!(h.app.plan, Plan::Pro);
}

#[test]
fn number_input_steps_clamps_and_parses_typed_values() {
    let mut h = Harness::new();
    assert!(!h.enabled("qty-dec"), "at the minimum");
    for _ in 0..3 {
        h.click("qty-inc");
    }
    assert_eq!(h.app.quantity, 3.);
    assert!(!h.enabled("qty-inc"), "at the maximum");
    h.click("qty-dec");
    assert_eq!(h.app.quantity, 2.);
    h.type_into("qty-field", "9");
    assert_eq!(h.app.quantity, 3., "typed values clamp");
}

#[test]
fn combobox_filters_and_picks_with_click_or_enter() {
    let mut h = Harness::new();
    h.type_into("fruit-field", "ap");
    assert!(h.app.combo.open);
    assert!(h.has("fruit-option-0") && h.has("fruit-option-2"));
    assert!(!h.has("fruit-option-1"), "Banana filtered out");
    h.click("fruit-option-2");
    assert_eq!(h.app.fruit, Some(3));
    assert!(!h.app.combo.open);
    h.type_into("fruit-field", "ban");
    h.enter();
    assert_eq!(h.app.fruit, Some(2), "Enter picks the best match");
    h.click("fruit-toggle");
    assert!(h.app.combo.open && h.app.combo.query.is_empty());
}

#[test]
fn tag_input_adds_on_enter_ignores_duplicates_and_removes() {
    let mut h = Harness::new();
    h.type_into("tags-field", "rust");
    h.enter();
    h.type_into("tags-field", "ui");
    h.enter();
    h.type_into("tags-field", "rust");
    h.enter();
    assert_eq!(h.app.tags, ["rust", "ui"]);
    assert!(h.app.tag_state.draft.is_empty());
    h.click("tags-remove-0");
    assert_eq!(h.app.tags, ["ui"]);
}

#[test]
fn color_picker_swatches_and_hex_field() {
    let mut h = Harness::new();
    h.click("colour-swatch-0");
    assert_eq!(h.app.colour, "#EF4444");
    h.press("colour-hex");
    h.runtime.editor_select_all();
    h.runtime
        .text_input("#10b981", &mut h.handlers, &mut h.app)
        .unwrap();
    assert_eq!(h.app.colour, "#10B981");
}

#[test]
fn otp_input_keeps_digits_and_reports_completion() {
    let mut h = Harness::new();
    h.type_into("otp-field", "12a");
    assert_eq!(h.app.code, "12");
    assert!(h.app.completed.is_none());
    h.runtime
        .text_input("345", &mut h.handlers, &mut h.app)
        .unwrap();
    assert_eq!(h.app.code, "1234", "capped at the length");
    assert_eq!(h.app.completed.as_deref(), Some("1234"));
}

#[test]
fn rating_sets_and_clears() {
    let mut h = Harness::new();
    h.click("stars-4");
    assert_eq!(h.app.stars, 4);
    h.click("stars-4");
    assert_eq!(h.app.stars, 0, "clicking the current rating clears it");
}

#[test]
fn adorned_text_field_insets_text_and_trailing_action_fires() {
    let mut h = Harness::new();
    assert!(h.has("search-adorned"));
    assert!(h.has("search-field-required"));
    h.type_into("search", "volt-ui");
    assert_eq!(h.app.search, "volt-ui");
    assert!(h.has("search-field-error"), "error shown past 5 chars");
    h.click("search-clear");
    assert_eq!(h.app.cleared, 1);
    assert!(h.app.search.is_empty());
    assert!(!h.has("search-field-error"));
}

#[test]
fn form_components_require_keys_and_handlers() {
    let cases: Vec<Element<App>> = vec![
        radio_group([(1, "a")])
            .on_select(|_: &mut App, _| {})
            .into(),
        radio_group::<App, i32>(Vec::<(i32, &str)>::new())
            .key("r")
            .on_select(|_, _| {})
            .into(),
        number_input(1.).key("n").into(),
        number_input(1.)
            .key("n")
            .step(0.)
            .on_change(|_: &mut App, _| {})
            .into(),
        combobox([(1, "a")], &ComboboxState::default())
            .key("c")
            .into(),
        tag_input(&[], &TagInputState::default()).key("t").into(),
        color_picker("#fff").key("p").into(),
        otp_input("", 1)
            .key("o")
            .on_change(|_: &mut App, _| {})
            .into(),
        rating(1).key("s").max(0).into(),
        form_field("x", text("y")).into(),
    ];
    for (i, e) in cases.into_iter().enumerate() {
        assert!(e.build().is_err(), "case {i} should be rejected");
    }
}

#[test]
fn range_slider_drags_the_nearer_thumb_and_never_crosses() {
    use volt_ui_kit::declarative::ValueKey;
    #[derive(Default)]
    struct Price {
        range: (f32, f32),
    }
    let build = |p: &Price| -> Element<Price> {
        vstack![range_slider(p.range.0, p.range.1)
            .key("price")
            .range(0. ..=100.)
            .step(1.)
            .on_change(|p: &mut Price, lo, hi| p.range = (lo, hi))
            .width(216.)]
        .key("root")
        .into()
    };
    let mut app = Price { range: (20., 80.) };
    let (d, mut handlers) = build(&app).build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &handlers, &Bindings::default()).unwrap();
    let vp = Viewport::new(400., 200., 1.).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    let b = rt.boxes().iter().find(|b| b.id == "price").unwrap().clone();
    let [x, y, _, h] = b.bounds.bounds();
    // Track runs from x+8 to x+208: value v sits at x + 8 + 2v.
    let at = |v: f32| x + 8. + 2. * v;
    let cy = y + h * 0.5;
    rt.pointer_down_outcome(at(30.), cy, &mut handlers, &mut app)
        .unwrap();
    rt.pointer_move_outcome(at(40.), cy, &mut handlers, &mut app)
        .unwrap();
    rt.pointer_up_outcome(at(40.), cy, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.range, (40., 80.), "the lower thumb moved");
    let (d, h2) = build(&app).build().unwrap().into_parts();
    rt.replace_document(d, &h2, &Bindings::default()).unwrap();
    handlers = h2;
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    rt.pointer_down_outcome(at(70.), cy, &mut handlers, &mut app)
        .unwrap();
    rt.pointer_move_outcome(at(10.), cy, &mut handlers, &mut app)
        .unwrap();
    rt.pointer_up_outcome(at(10.), cy, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.range, (40., 40.), "the upper thumb stops at the lower");
    rt.slider_key(ValueKey::Increase, &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app.range, (40., 41.));
}

#[test]
fn drop_zone_receives_files_under_the_pointer_or_as_the_only_zone() {
    use std::path::PathBuf;
    #[derive(Default)]
    struct Files {
        got: Vec<PathBuf>,
        other: usize,
    }
    let two = |both: bool| -> Element<Files> {
        let mut items: Vec<Element<Files>> = vec![drop_zone()
            .key("images")
            .accept(["png"])
            .on_drop(|f: &mut Files, p| f.got.extend(p))
            .into()];
        if both {
            items.push(
                drop_zone()
                    .key("docs")
                    .on_drop(|f: &mut Files, _| f.other += 1)
                    .into(),
            );
        }
        vstack(items).key("root").gap(10., Axis::Vertical).into()
    };
    let vp = Viewport::new(400., 600., 1.).unwrap();
    let mut app = Files::default();
    // One zone: no pointer position needed.
    let (d, mut h) = two(false).build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    assert!(rt.file_hover(None));
    assert_eq!(rt.file_hover_target(), Some("images"));
    let paths = vec![PathBuf::from("a.png"), PathBuf::from("b.txt")];
    assert!(
        rt.drop_files(None, paths, &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(
        app.got,
        [PathBuf::from("a.png")],
        "extension filter applies"
    );
    assert_eq!(rt.file_hover_target(), None);
    // Two zones: the pointer decides; without it nothing is delivered.
    let (d, mut h) = two(true).build().unwrap().into_parts();
    let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
    rt.layout(vp, &Bindings::default(), &mut Metrics).unwrap();
    let docs = rt.boxes().iter().find(|b| b.id == "docs").unwrap().clone();
    let [x, y, w, hh] = docs.bounds.bounds();
    let centre = Some([x + w * 0.5, y + hh * 0.5]);
    assert!(
        !rt.drop_files(None, vec![PathBuf::from("x")], &mut h, &mut app)
            .unwrap()
            .activated
    );
    rt.drop_files(centre, vec![PathBuf::from("x")], &mut h, &mut app)
        .unwrap();
    assert_eq!(app.other, 1);
}
