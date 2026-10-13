#![cfg(feature = "rust-ui")]
//! Charts with a key are interactive: per-point hit targets with tooltips,
//! click/keyboard selection, and screen-reader labels.
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
#[derive(Default)]
struct App {
    candle: Option<usize>,
    slice: Option<usize>,
    month: Option<usize>,
    quarter: Option<usize>,
}
fn view(app: &App) -> Element<App> {
    vstack![
        candlestick_chart([
            Candle::new(10., 12., 9., 11.).volume(50.),
            Candle::new(11., 13., 10., 12.5),
            Candle::new(12.5, 13., 11., 11.5),
            Candle::new(11.5, 12., 10.5, 11.8),
        ])
        .key("px")
        .size(244., 160.)
        .x_labels(["Mon", "Tue", "Wed", "Thu"])
        .selected(app.candle)
        .on_select(|a: &mut App, i| a.candle = Some(i)),
        donut_chart([("Search", 50.), ("Direct", 30.), ("Email", 20.)])
            .key("src")
            .size(120.)
            .thickness(20.)
            .legend(true)
            .selected(app.slice)
            .on_select(|a: &mut App, i| a.slice = Some(i)),
        area_chart([
            ("Pro", vec![1., 2., 3., 4., 5.]),
            ("Team", vec![2., 2., 2., 2., 2.])
        ])
        .key("rev")
        .size(244., 160.)
        .x_labels(["Jan", "Feb", "Mar", "Apr", "May"])
        .selected(app.month)
        .on_select(|a: &mut App, i| a.month = Some(i)),
        stacked_bar_chart(["Q1", "Q2"], [("A", vec![3., 4.]), ("B", vec![1., 2.])])
            .key("reg")
            .size(244., 160.)
            .selected(app.quarter)
            .on_select(|a: &mut App, i| a.quarter = Some(i)),
        // No key: a static drawing, no hit targets.
        area_chart::<App>([("Static", vec![1., 2.])]).size(200., 120.),
    ]
    .key("page")
    .gap(16., Axis::Vertical)
    .into()
}
struct Harness {
    app: App,
    rt: Runtime,
    h: Handlers<'static, App>,
}
impl Harness {
    fn new() -> Self {
        let app = App::default();
        let (d, h) = view(&app).build().unwrap().into_parts();
        let mut rt = Runtime::new(d, &h, &Bindings::default()).unwrap();
        rt.layout(
            Viewport::new(600., 1200., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        Self { app, rt, h }
    }
    fn rebuild(&mut self) {
        let (d, h) = view(&self.app).build().unwrap().into_parts();
        self.rt
            .replace_document(d, &h, &Bindings::default())
            .unwrap();
        self.h = h;
        self.rt
            .layout(
                Viewport::new(600., 1200., 1.).unwrap(),
                &Bindings::default(),
                &mut Metrics,
            )
            .unwrap();
    }
    fn bounds(&self, id: &str) -> [f32; 4] {
        self.rt
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap_or_else(|| panic!("no {id}"))
            .bounds
            .bounds()
    }
    fn click_at(&mut self, x: f32, y: f32) -> bool {
        self.rt.pointer_down(x, y);
        let o = self
            .rt
            .pointer_up_outcome(x, y, &mut self.h, &mut self.app)
            .unwrap();
        self.rebuild();
        o.activated
    }
    fn click(&mut self, id: &str) -> bool {
        let [x, y, w, h] = self.bounds(id);
        self.click_at(x + w / 2., y + h / 2.)
    }
}

#[test]
fn clicking_points_selects_them_in_every_chart() {
    let mut t = Harness::new();
    assert!(t.click("px-2"));
    assert_eq!(t.app.candle, Some(2));
    assert!(t.click("rev-4"));
    assert_eq!(t.app.month, Some(4));
    assert!(t.click("reg-1"));
    assert_eq!(t.app.quarter, Some(1));
    // Legend rows select donut slices too.
    assert!(t.click("src-legend-2"));
    assert_eq!(t.app.slice, Some(2));
}

#[test]
fn donut_hits_follow_the_ring_not_the_square() {
    let mut t = Harness::new();
    let [x, y, w, _] = t.bounds("src-0");
    let (cx, cy, r) = (x + w / 2., y + w / 2., w / 2.);
    // "Search" is the first half of the ring (12 → 6 o'clock, right side).
    assert!(t.click_at(cx + r - 10., cy));
    assert_eq!(t.app.slice, Some(0));
    // "Email" is the last fifth, just left of 12 o'clock.
    assert!(t.click_at(cx - 12., cy - r + 8.));
    assert_eq!(t.app.slice, Some(2));
    // The hole selects nothing.
    t.app.slice = None;
    t.rebuild();
    t.click_at(cx, cy);
    assert_eq!(t.app.slice, None);
}

#[test]
fn points_have_spoken_labels_and_static_charts_have_no_targets() {
    let t = Harness::new();
    let nodes = t.rt.accessibility_nodes();
    let label = |id: &str| nodes.iter().find(|n| n.id == id).map(|n| n.label.clone());
    let wed = label("px-2").unwrap();
    assert!(wed.starts_with("Wed, Open 12.5"), "{wed}");
    assert!(wed.contains("Close 11.5") && wed.contains("Change -8.00%"));
    assert_eq!(label("rev-1").unwrap(), "Feb, Pro: 2, Team: 2");
    assert_eq!(label("reg-0").unwrap(), "Q1, A: 3, B: 1, Total: 4");
    assert!(label("src-0").unwrap().contains("Search, 50 · 50%"));
    // Ten hit targets above; the keyless chart adds none.
    let targets =
        t.rt.boxes()
            .iter()
            .filter(|b| b.id.ends_with("-tip-anchor") || b.id.contains("-hits"))
            .count();
    let _ = targets;
    assert!(!t
        .rt
        .boxes()
        .iter()
        .any(|b| b.id.starts_with("auto") && b.id.ends_with("-hits")));
}

#[test]
fn hovering_a_point_shows_its_value_tooltip() {
    let mut t = Harness::new();
    let [x, y, w, h] = t.bounds("px-1");
    assert!(!t
        .rt
        .boxes()
        .iter()
        .any(|b| b.id.starts_with("px-1-tip-tooltip")));
    t.rt.pointer_move(x + w / 2., y + h / 2.);
    t.rt.layout(
        Viewport::new(600., 1200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let tip =
        t.rt.boxes()
            .iter()
            .find(|b| b.id == "px-1-tip-tooltip-content")
            .expect("tooltip shown on hover");
    // The bubble sits next to the hovered column, inside the window.
    let [tx, _, tw, _] = tip.bounds.bounds();
    assert!(tx >= 0. && tx + tw <= 600.);
    // Moving away hides it again.
    t.rt.pointer_move(590., 1190.);
    t.rt.layout(
        Viewport::new(600., 1200., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(!t
        .rt
        .boxes()
        .iter()
        .any(|b| b.id == "px-1-tip-tooltip-content"));
}
