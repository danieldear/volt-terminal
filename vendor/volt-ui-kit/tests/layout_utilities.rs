#![cfg(feature = "rust-ui")]
//! Tailwind-style layout utilities: grid, wrap, justify, sizing constraints,
//! aspect ratio and responsive visibility.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        let w = t.chars().count() as f32 * size * 0.5;
        [w.min(max), size * 1.2 * (w / max.max(1.)).ceil().max(1.)]
    }
}
fn layout(e: Element<()>, size: [f32; 2]) -> Runtime {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(size[0], size[1], 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r
}
fn rect(r: &Runtime, id: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no box {id}"))
        .bounds
        .bounds()
}
fn cell(i: usize, h: f32) -> Element<()> {
    vstack![].key(format!("c{i}")).height(h).into()
}

#[test]
fn grid_columns_spans_and_row_heights() {
    let e = grid((0..5).map(|i| -> Element<()> {
        if i == 3 {
            vstack![].key("c3").height(30.).column_span(2).into()
        } else {
            cell(i, 20. + i as f32 * 10.)
        }
    }))
    .key("g")
    .columns(3)
    .gap(10., Axis::Both)
    .width(320.);
    let e: Element<()> = vstack![e].key("page").into();
    let r = layout(e, [400., 400.]);
    // (320 - 2×10) / 3 = 100px columns; explicit heights are kept.
    assert_eq!(rect(&r, "c0"), [0., 0., 100., 20.]);
    assert_eq!(rect(&r, "c1"), [110., 0., 100., 30.]);
    assert_eq!(rect(&r, "c2")[0], 220.);
    // Row one is as tall as its tallest item (40), so row two starts at 50;
    // c3 spans two columns.
    assert_eq!(rect(&r, "c3"), [0., 50., 210., 30.]);
    assert_eq!(rect(&r, "c4"), [220., 50., 100., 60.]);
    assert_eq!(rect(&r, "g")[3], 110.);
}

#[test]
fn auto_fit_grid_adds_columns_as_width_grows() {
    let build = || -> Element<()> {
        grid((0..6).map(|i| cell(i, 10.)))
            .key("g")
            .min_column_width(150.)
            .gap(10., Axis::Both)
            .into()
    };
    let narrow = layout(build(), [330., 300.]);
    let wide = layout(build(), [650., 300.]);
    assert_eq!(
        rect(&narrow, "c2")[1],
        20.,
        "two columns: c2 starts row two"
    );
    assert_eq!(
        rect(&wide, "c3")[1],
        0.,
        "four columns: c3 still on row one"
    );
}

#[test]
fn wrapping_rows_break_lines_instead_of_squeezing() {
    let e: Element<()> =
        vstack![hstack((0..5).map(|i| -> Element<()> {
            vstack![].key(format!("w{i}")).width(90.).height(20.).into()
        }))
        .key("row")
        .wrap(true)
        .gap(10., Axis::Both)
        .width(300.)]
        .key("page")
        .into();
    let r = layout(e, [400., 300.]);
    assert_eq!(rect(&r, "w0"), [0., 0., 90., 20.]);
    assert_eq!(rect(&r, "w2"), [200., 0., 90., 20.]);
    assert_eq!(rect(&r, "w3"), [0., 30., 90., 20.], "fourth wraps");
    assert_eq!(rect(&r, "row")[3], 50.);
}

#[test]
fn justify_distributes_free_space() {
    let make = |j: Justify| -> Element<()> {
        hstack((0..3).map(|i| -> Element<()> {
            vstack![].key(format!("j{i}")).width(50.).height(10.).into()
        }))
        .key("row")
        .justify(j)
        .width(350.)
        .into()
    };
    let x = |j| {
        let r = layout(make(j), [400., 100.]);
        [rect(&r, "j0")[0], rect(&r, "j1")[0], rect(&r, "j2")[0]]
    };
    assert_eq!(x(Justify::Start), [0., 50., 100.]);
    assert_eq!(x(Justify::End), [200., 250., 300.]);
    assert_eq!(x(Justify::Center), [100., 150., 200.]);
    assert_eq!(x(Justify::SpaceBetween), [0., 150., 300.]);
    assert_eq!(x(Justify::SpaceEvenly), [50., 150., 250.]);
}

#[test]
fn min_max_fraction_and_aspect_ratio() {
    let e: Element<()> = vstack![
        vstack![].key("capped").height(10.).max_width(120.),
        vstack![].key("half").height(10.).width_fraction(0.5),
        vstack![].key("video").width(160.).aspect_ratio(16. / 9.),
        hstack![vstack![].key("grow").flex(1.).height(10.).max_width(80.)].key("row"),
        vstack![].key("tall").min_height(40.),
    ]
    .key("col")
    .width(300.)
    .into();
    let r = layout(e, [400., 600.]);
    assert_eq!(rect(&r, "capped")[2], 120.);
    assert_eq!(rect(&r, "half")[2], 150.);
    assert_eq!(rect(&r, "video")[2..], [160., 90.]);
    assert_eq!(rect(&r, "grow")[2], 80., "flex growth stops at max");
    assert_eq!(rect(&r, "tall")[3], 40.);
}

#[test]
fn responsive_visibility_removes_layout_space() {
    let build = || -> Element<()> {
        hstack![
            vstack![]
                .key("side")
                .width(100.)
                .height(10.)
                .visible(responsive(false).at(Breakpoint::Md, true)),
            vstack![].key("main").width(50.).height(10.),
        ]
        .key("row")
        .gap(10., Axis::Horizontal)
        .into()
    };
    let small = layout(build(), [500., 100.]);
    assert!(small.boxes().iter().all(|b| b.id != "side"));
    assert_eq!(rect(&small, "main")[0], 0.);
    let large = layout(build(), [900., 100.]);
    assert_eq!(rect(&large, "main")[0], 110.);
}

#[test]
fn invalid_extensions_are_rejected() {
    for bad in [
        vstack![].key("x").max_width(f32::NAN),
        vstack![].key("x").width_fraction(1.5),
        vstack![].key("x").aspect_ratio(0.),
        vstack![].key("x").columns(0),
    ] {
        assert!(Element::<()>::from(bad).build().is_err());
    }
}

#[test]
fn cursor_defaults_and_explicit_overrides() {
    struct App;
    let e: Element<App> = vstack![
        button("Go").key("go").on_click(|_: &mut App| {}),
        button("Off")
            .key("off")
            .enabled(false)
            .on_click(|_: &mut App| {}),
        text_field("x").key("field").on_change(|_: &mut App, _| {}),
        vstack![crate_text()]
            .key("help")
            .height(30.)
            .cursor(Cursor::Help),
        vstack![].key("plain").height(30.),
    ]
    .key("page")
    .into();
    fn crate_text() -> Element<App> {
        text("Hover me").key("hint").into()
    }
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(300., 400., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    let centre = |id: &str| {
        let [x, y, w, h] = r
            .boxes()
            .iter()
            .find(|b| b.id == id)
            .unwrap()
            .bounds
            .bounds();
        (x + w * 0.5, y + h * 0.5)
    };
    let at = |id: &str| {
        let (x, y) = centre(id);
        r.cursor_at(x, y)
    };
    assert_eq!(at("go"), Cursor::Pointer);
    assert_eq!(
        at("off"),
        Cursor::Default,
        "disabled controls are not clickable"
    );
    assert_eq!(at("field"), Cursor::Text);
    assert_eq!(at("hint"), Cursor::Help, "inherited from the ancestor");
    assert_eq!(at("plain"), Cursor::Default);
}
