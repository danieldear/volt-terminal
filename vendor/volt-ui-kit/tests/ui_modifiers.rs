#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Kind, Runtime, TextMetrics},
    ui::{column, prelude::*},
    Painter, Theme, Viewport,
};
#[derive(Clone, Debug)]
struct Rounded {
    bounds: [f32; 4],
    radius: f32,
}
struct Recorder;
impl TextMetrics for Recorder {
    fn measure(&mut self, _: &str, size: f32, _: f32) -> [f32; 2] {
        [20., size * 1.2]
    }
}
impl Painter for Recorder {
    type Shape = Rounded;
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(300., 200., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<Rounded>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<Rounded>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_round(&self, out: &mut Vec<Rounded>, bounds: [f32; 4], radius: f32, _: [f32; 4]) {
        out.push(Rounded { bounds, radius });
    }
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
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}
fn runtime(element: Element<usize>, scale: f32) -> (Runtime, Handlers<'static, usize>) {
    let (doc, h) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(300. * scale, 200. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Recorder,
        )
        .unwrap();
    (runtime, h)
}
fn bounds(r: &Runtime, id: &str) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds()
}
#[test]
fn rounding_accepts_integer_fractional_and_full_and_full_tracks_layout_size_and_hits() {
    for scale in [1., 1.5, 2.] {
        for (rounding, width, height, radius) in [
            (Rounding::Full, 100., 40., 20.),
            (4.into(), 100., 40., 4.),
            (4.5.into(), 100., 40., 4.5),
            (Rounding::Full, 40., 40., 20.),
        ] {
            let (mut r, mut h) = runtime(
                button("Pill")
                    .key("pill")
                    .width(width)
                    .height(height)
                    .rounded(rounding)
                    .on_click(|n: &mut usize| *n += 1)
                    .into(),
                scale,
            );
            let batches = r.paint(&mut Recorder, &Theme::dark());
            assert!(batches[0]
                .shapes
                .iter()
                .any(|s| s.bounds == [0., 0., width * scale, height * scale]
                    && (s.radius - radius * scale).abs() < 0.01));
            let mut calls = 0;
            r.pointer_down(0.1, 0.1);
            assert!(
                !r.pointer_up_outcome(0.1, 0.1, &mut h, &mut calls)
                    .unwrap()
                    .activated
            );
            r.pointer_down(width * scale * 0.5, height * scale * 0.5);
            assert!(
                r.pointer_up_outcome(
                    width * scale * 0.5,
                    height * scale * 0.5,
                    &mut h,
                    &mut calls
                )
                .unwrap()
                .activated
            );
            assert_eq!(calls, 1);
            if rounding == Rounding::Full {
                r.layout(
                    Viewport::new(60. * scale, 30. * scale, scale).unwrap(),
                    &Bindings::default(),
                    &mut Recorder,
                )
                .unwrap();
                let shape = &r.paint(&mut Recorder, &Theme::dark())[0].shapes[0];
                assert!((shape.radius - 15. * scale).abs() < 0.01);
            }
        }
    }
    let _: Element<usize> = text("Integer literal").rounded(4).into();
    let _: Element<usize> = text("Fractional literal").rounded(4.5).into();
}
#[test]
fn rounding_is_local_last_wins_and_numeric_values_are_validated() {
    let (r, _) = runtime(
        panel![button("Default")
            .key("child")
            .width(100.)
            .height(40.)
            .on_click(|_: &mut usize| {})]
        .key("panel")
        .width(120.)
        .height(60.)
        .rounded(Rounding::Full)
        .into(),
        1.,
    );
    let batches = r.paint(&mut Recorder, &Theme::dark());
    assert_eq!(batches[0].shapes[0].radius, 30.);
    assert!(batches[1].shapes.iter().all(|s| s.radius != 30.)); // no radius inheritance
    let (r, _) = runtime(
        button("Override")
            .width(100.)
            .height(40.)
            .rounded(Rounding::Full)
            .rounded(4)
            .on_click(|_: &mut usize| {})
            .into(),
        1.,
    );
    assert_eq!(
        r.paint(&mut Recorder, &Theme::dark())[0].shapes[0].radius,
        4.
    );
    for value in [-1., f64::NAN, f64::INFINITY, 257.] {
        assert!(Element::<usize>::from(text("Bad").rounded(value))
            .build()
            .is_err());
    }
}
#[test]
fn panel_flow_positions_content_group_in_both_axes_and_retains_default_when_unset() {
    for scale in [1., 1.5, 2.] {
        for (flow, x, y) in [
            (Flow::TopLeft, 10., 10.),
            (Flow::Top, 40., 10.),
            (Flow::TopRight, 70., 10.),
            (Flow::Left, 10., 20.),
            (Flow::Center, 40., 20.),
            (Flow::Right, 70., 20.),
            (Flow::BottomLeft, 10., 30.),
            (Flow::Bottom, 40., 30.),
            (Flow::BottomRight, 70., 30.),
        ] {
            let (r, _) = runtime(
                panel![
                    text("A").key("a").width(20.).height(10.),
                    text("B").key("b").width(20.).height(10.)
                ]
                .width(100.)
                .height(70.)
                .padding(10., Edge::All)
                .gap(10., Axis::Vertical)
                .flow(flow)
                .into(),
                scale,
            );
            assert_eq!(
                bounds(&r, "a"),
                [x * scale, y * scale, 20. * scale, 10. * scale]
            );
            assert_eq!(
                bounds(&r, "b"),
                [x * scale, (y + 20.) * scale, 20. * scale, 10. * scale]
            );
        }
    }
    let (r, _) = runtime(
        row![
            text("A").key("a").width(20.).height(10.),
            text("B").key("b").width(20.).height(10.)
        ]
        .width(100.)
        .height(70.)
        .padding(10., Edge::All)
        .gap(10., Axis::Horizontal)
        .flow(Flow::Center)
        .into(),
        1.,
    );
    assert_eq!(bounds(&r, "a"), [25., 30., 20., 10.]);
    assert_eq!(bounds(&r, "b"), [55., 30., 20., 10.]);
}
#[test]
fn divider_reuses_separator_and_flexible_spacer_consumes_only_remaining_main_space() {
    let (mut r, mut h) = runtime(
        column![
            text("A").key("a").height(10.),
            spacer().key("space"),
            divider().key("divider"),
            button("Bottom")
                .key("bottom")
                .height(20.)
                .on_click(|n: &mut usize| *n += 1)
        ]
        .width(100.)
        .height(100.)
        .gap(5., Axis::Vertical)
        .into(),
        1.,
    );
    assert_eq!(bounds(&r, "space"), [0., 15., 100., 54.]);
    assert_eq!(bounds(&r, "divider"), [0., 74., 100., 1.]);
    assert_eq!(bounds(&r, "bottom"), [0., 80., 100., 20.]);
    assert!(r
        .boxes()
        .iter()
        .any(|b| b.id == "divider" && b.kind == Kind::Separator));
    assert!(r
        .boxes()
        .iter()
        .any(|b| b.id == "space" && b.kind == Kind::Spacer));
    let mut count = 0;
    r.pointer_down(50., 40.);
    assert!(
        !r.pointer_up_outcome(50., 40., &mut h, &mut count)
            .unwrap()
            .activated
    );
    r.focus_next(false);
    assert_eq!(r.focused(), Some("bottom"));
    let (r, _) = runtime(
        row![
            text("A").key("a").width(20.).height(10.),
            spacer(),
            text("B").key("b").width(20.).height(10.)
        ]
        .width(100.)
        .height(30.)
        .gap(5., Axis::Horizontal)
        .into(),
        1.,
    );
    assert_eq!(bounds(&r, "b"), [80., 0., 20., 10.]);
}
#[test]
fn stack_aliases_functions_and_macros_preserve_row_column_geometry() {
    fn children() -> Vec<Element<usize>> {
        vec![
            text("A").key("a").width(20.).height(10.).into(),
            text("B").key("b").width(20.).height(10.).into(),
        ]
    }
    for (alias, original) in [
        (
            hstack(children()).gap(8., Axis::Horizontal).into(),
            row(children()).gap(8., Axis::Horizontal).into(),
        ),
        (
            vstack(children()).gap(8., Axis::Vertical).into(),
            column(children()).gap(8., Axis::Vertical).into(),
        ),
        (
            hstack![
                text("A").key("a").width(20.).height(10.),
                text("B").key("b").width(20.).height(10.)
            ]
            .gap(8., Axis::Horizontal)
            .into(),
            row(children()).gap(8., Axis::Horizontal).into(),
        ),
        (
            vstack![
                text("A").key("a").width(20.).height(10.),
                text("B").key("b").width(20.).height(10.)
            ]
            .gap(8., Axis::Vertical)
            .into(),
            column(children()).gap(8., Axis::Vertical).into(),
        ),
    ] {
        let (a, _) = runtime(alias, 1.);
        let (b, _) = runtime(original, 1.);
        assert_eq!(bounds(&a, "a"), bounds(&b, "a"));
        assert_eq!(bounds(&a, "b"), bounds(&b, "b"));
    }
    let _: Element<usize> = vstack![].into();
    let _: Element<usize> = hstack![].into();
}
#[test]
fn flow_disables_default_cross_stretch_but_honors_explicit_stretch_and_padding_constraints() {
    for (flow, align, width, x, y) in [
        (None, Align::Start, 80., 10., 10.),
        (Some(Flow::Center), Align::Start, 20., 40., 26.6),
        (Some(Flow::Center), Align::Stretch, 80., 10., 26.6),
    ] {
        let mut p = panel![text("A").key("a")]
            .width(100.)
            .height(70.)
            .padding(10., Edge::All)
            .align(align);
        if let Some(flow) = flow {
            p = p.flow(flow)
        }
        let (r, _) = runtime(p.into(), 1.);
        let a = bounds(&r, "a");
        for (actual, expected) in a.into_iter().zip([x, y, width, 16.8]) {
            assert!((actual - expected).abs() < 0.01)
        }
    }
    // Overfull content is shrunk and remains inside the content area for every placement.
    for flow in [Flow::TopLeft, Flow::Center, Flow::BottomRight] {
        let (r, _) = runtime(
            hstack![
                text("A").key("a").width(100.).height(50.),
                text("B").key("b").width(100.).height(50.)
            ]
            .width(80.)
            .height(40.)
            .padding(10., Edge::All)
            .gap(8., Axis::Horizontal)
            .flow(flow)
            .into(),
            1.,
        );
        assert_eq!(bounds(&r, "a"), [10., 10., 26., 20.]);
        assert_eq!(bounds(&r, "b"), [44., 10., 26., 20.]);
    }
}
#[test]
fn multiple_and_fixed_spacers_share_remaining_space_proportionally() {
    for scale in [1., 1.5, 2.] {
        let (r, _) = runtime(
            hstack![
                text("A").width(20.).height(10.),
                spacer().key("one"),
                spacer().key("fixed").flex(0.).width(10.),
                spacer().key("two").flex(2.),
                text("B").width(20.).height(10.)
            ]
            .width(120.)
            .height(40.)
            .padding(10., Edge::All)
            .gap(5., Axis::Horizontal)
            .into(),
            scale,
        );
        assert_eq!(
            bounds(&r, "one"),
            [35. * scale, 10. * scale, 10. * scale, 20. * scale]
        );
        assert_eq!(
            bounds(&r, "fixed"),
            [50. * scale, 10. * scale, 10. * scale, 20. * scale]
        );
        assert_eq!(
            bounds(&r, "two"),
            [65. * scale, 10. * scale, 20. * scale, 20. * scale]
        );
        let batches = r.paint(&mut Recorder, &Theme::dark());
        assert!(batches
            .iter()
            .all(|b| b.shapes.is_empty() && b.glyphs.is_empty()));
    }
}
