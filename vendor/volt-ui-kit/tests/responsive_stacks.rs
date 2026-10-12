#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Theme, Viewport,
};

struct Recorder;
impl TextMetrics for Recorder {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        let natural = text.len() as f32 * size * 0.5;
        [
            natural.min(width),
            (natural / width.max(0.1)).ceil().max(1.) * size * 1.2,
        ]
    }
}
impl Painter for Recorder {
    type Shape = [f32; 4];
    type Glyph = String;
    fn viewport(&self) -> Viewport {
        Viewport::new(1024., 600., 1.).unwrap()
    }
    fn draw_rect(&self, out: &mut Vec<Self::Shape>, x: f32, y: f32, w: f32, h: f32, _: [f32; 4]) {
        out.push([x, y, w, h]);
    }
    fn card_triangle(&self, _: &mut Vec<Self::Shape>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_round(&self, out: &mut Vec<Self::Shape>, bounds: [f32; 4], _: f32, _: [f32; 4]) {
        out.push(bounds);
    }
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
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, out: &mut Vec<String>, text: &str, _: [f32; 4], _: f32, _: [f32; 4]) {
        out.push(text.into());
    }
}
fn build(element: Element<usize>) -> (Runtime, Handlers<'static, usize>) {
    let (doc, h) = element.build().unwrap().into_parts();
    (Runtime::new(doc, &h, &Bindings::default()).unwrap(), h)
}
fn layout(r: &mut Runtime, width: f32, scale: f32) {
    r.layout(
        Viewport::new(width * scale, 300. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Recorder,
    )
    .unwrap();
}
fn bounds(r: &Runtime, id: &str, scale: f32) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds()
        .map(|v| v / scale)
}
fn close(a: [f32; 4], b: [f32; 4]) {
    for (a, b) in a.into_iter().zip(b) {
        assert!((a - b).abs() < 0.01, "{a} != {b}");
    }
}
fn pair() -> Vec<Element<usize>> {
    vec![
        text("A").key("a").width(40.).height(20.).into(),
        text("B").key("b").width(40.).height(20.).into(),
    ]
}

#[test]
fn breakpoints_are_inclusive_logical_width_and_resize_back_and_forth() {
    let (mut r, _) = build(
        stack(pair())
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
            .padding(responsive(10.).at(Breakpoint::Md, 20.), Edge::All)
            .gap(responsive(5.).at(Breakpoint::Md, 12.), Axis::Both)
            .into(),
    );
    for scale in [1., 1.5, 2.] {
        for width in [767., 768., 769., 1024., 767., 768.] {
            layout(&mut r, width, scale);
            let wide = width >= 768.;
            close(
                bounds(&r, "a", scale),
                if wide {
                    [20., 20., 40., 20.]
                } else {
                    [10., 10., 40., 20.]
                },
            );
            close(
                bounds(&r, "b", scale),
                if wide {
                    [72., 20., 40., 20.]
                } else {
                    [10., 35., 40., 20.]
                },
            );
        }
    }
}

#[test]
fn highest_threshold_wins_not_declaration_order_and_duplicate_last_wins() {
    let (mut r, _) = build(
        stack(pair())
            .direction(Direction::Horizontal)
            .padding(
                responsive(1.)
                    .at(Breakpoint::Lg, 30.)
                    .at(Breakpoint::Md, 20.)
                    .at(Breakpoint::MinWidth(768.), 24.),
                Edge::All,
            )
            .gap(
                responsive(2.).at(Breakpoint::MinWidth(800.), 8.),
                Axis::Both,
            )
            .into(),
    );
    for (width, pad, gap) in [
        (767., 1., 2.),
        (768., 24., 2.),
        (800., 24., 8.),
        (1024., 30., 8.),
    ] {
        layout(&mut r, width, 1.);
        close(bounds(&r, "b", 1.), [pad + 40. + gap, pad, 40., 20.]);
    }
}

#[test]
fn responsive_edge_and_axis_setters_keep_existing_last_write_semantics() {
    let (mut r, _) = build(
        stack(pair())
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
            .padding(responsive(10.).at(Breakpoint::Md, 20.), Edge::All)
            .padding(4., Edge::Horizontal)
            .padding(responsive(3.).at(Breakpoint::Md, 7.), Edge::Top)
            .gap(90., Axis::Both)
            .gap(8., Axis::Vertical)
            .gap(responsive(6.).at(Breakpoint::Md, 12.), Axis::Horizontal)
            .into(),
    );
    layout(&mut r, 767., 1.);
    close(bounds(&r, "b", 1.), [4., 31., 40., 20.]);
    layout(&mut r, 768., 1.);
    close(bounds(&r, "b", 1.), [56., 7., 40., 20.]);
    let (mut r, _) = build(
        stack(pair())
            .padding(responsive(99.).at(Breakpoint::Md, 50.), Edge::Top)
            .padding(2., Edge::All)
            .gap(responsive(50.).at(Breakpoint::Md, 80.), Axis::Vertical)
            .gap(3., Axis::Both)
            .into(),
    );
    layout(&mut r, 800., 1.);
    close(bounds(&r, "b", 1.), [2., 25., 40., 20.]);
}

#[test]
fn fixed_stacks_and_scalar_adaptive_stack_have_identical_geometry() {
    for scale in [1., 2.] {
        for (adaptive, fixed) in [
            (
                stack(pair())
                    .direction(Direction::Horizontal)
                    .padding(8., Edge::All)
                    .gap(4., Axis::Both)
                    .into(),
                hstack(pair())
                    .padding(8., Edge::All)
                    .gap(4., Axis::Both)
                    .into(),
            ),
            (
                stack(pair())
                    .padding(8., Edge::All)
                    .gap(4., Axis::Both)
                    .into(),
                vstack(pair())
                    .padding(8., Edge::All)
                    .gap(4., Axis::Both)
                    .into(),
            ),
        ] {
            let (mut a, _) = build(adaptive);
            let (mut b, _) = build(fixed);
            layout(&mut a, 800., scale);
            layout(&mut b, 800., scale);
            for id in ["a", "b"] {
                close(bounds(&a, id, scale), bounds(&b, id, scale));
            }
        }
    }
    let _: Element<usize> = stack![].into();
    let _: Element<usize> = zstack![].into();
}

#[test]
fn resize_preserves_focus_callbacks_and_resolves_nested_intrinsic_sizes() {
    let mut closure_calls = 0;
    let (mut r, mut h) = build(
        vstack![stack![
            button("A")
                .key("a")
                .width(40.)
                .height(20.)
                .on_click(move |n: &mut usize| {
                    closure_calls += 1;
                    *n = closure_calls;
                }),
            text("B").key("b").width(40.).height(20.),
        ]
        .key("inner")
        .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
        .gap(6., Axis::Both)
        .padding(3., Edge::All)]
        .into(),
    );
    let mut count = 0;
    layout(&mut r, 767., 1.);
    r.focus_next(false);
    for width in [768., 767., 769.] {
        // Match the native host's cancellation of in-flight input on resize.
        r.key_down();
        r.cancel_input();
        layout(&mut r, width, 1.);
        assert_eq!(r.focused(), Some("a"));
        assert!(!r.key_up_outcome(&mut h, &mut count).unwrap().activated);
        close(
            bounds(&r, "inner", 1.),
            [0., 0., width, if width >= 768. { 26. } else { 52. }],
        );
        let [x, y, w, hgt] = bounds(&r, "a", 1.);
        r.pointer_down(x + w / 2., y + hgt / 2.);
        assert!(
            r.pointer_up_outcome(x + w / 2., y + hgt / 2., &mut h, &mut count)
                .unwrap()
                .activated
        );
    }
    assert_eq!(count, 3); // Captured callback state was not recreated by layout.
}

#[test]
fn inactive_rules_and_thresholds_are_validated_and_rule_count_is_bounded() {
    for value in [-1., f32::NAN, f32::INFINITY, 16385.] {
        assert!(Element::<usize>::from(
            vstack![].padding(responsive(2.).at(Breakpoint::Lg, value), Edge::Right)
        )
        .build()
        .is_err());
        assert!(Element::<usize>::from(
            stack![].gap(responsive(2.).at(Breakpoint::Lg, value), Axis::Horizontal)
        )
        .build()
        .is_err());
    }
    for threshold in [-1., f32::NAN, f32::INFINITY, 65537.] {
        assert!(Element::<usize>::from(
            stack![].direction(
                responsive(Direction::Vertical)
                    .at(Breakpoint::MinWidth(threshold), Direction::Horizontal)
            )
        )
        .build()
        .is_err());
    }
    let mut rules = responsive(2.);
    for i in 0..33 {
        rules = rules.at(Breakpoint::MinWidth(i as f32), 4.);
    }
    assert!(Element::<usize>::from(vstack![].padding(rules, Edge::All))
        .build()
        .is_err());
}

#[test]
fn zstack_centers_layers_independently_paints_in_order_and_hits_topmost_button() {
    for scale in [1., 1.5, 2.] {
        let (mut r, mut h) = build(
            zstack![
                button("Back")
                    .key("back")
                    .width(100.)
                    .height(60.)
                    .on_click(|n: &mut usize| *n += 1),
                button("Front")
                    .key("front")
                    .width(40.)
                    .height(20.)
                    .on_click(|n: &mut usize| *n += 10),
            ]
            .width(200.)
            .height(100.)
            .padding(10., Edge::All)
            .into(),
        );
        layout(&mut r, 800., scale);
        close(bounds(&r, "back", scale), [50., 20., 100., 60.]);
        close(bounds(&r, "front", scale), [80., 40., 40., 20.]);
        let glyphs: Vec<_> = r
            .paint(&mut Recorder, &Theme::dark())
            .into_iter()
            .flat_map(|b| b.glyphs)
            .filter(|text| !text.is_empty())
            .collect();
        assert_eq!(glyphs, ["Back", "Front"]);
        let mut count = 0;
        r.pointer_down(100. * scale, 50. * scale);
        assert!(
            r.pointer_up_outcome(100. * scale, 50. * scale, &mut h, &mut count)
                .unwrap()
                .activated
        );
        assert_eq!(count, 10);
        r.pointer_down(60. * scale, 50. * scale);
        r.pointer_up_outcome(60. * scale, 50. * scale, &mut h, &mut count)
            .unwrap();
        assert_eq!(count, 11);
    }
}

#[test]
fn zstack_intrinsic_size_is_max_not_sum_and_flow_and_stretch_honor_constraints() {
    let (mut r, _) = build(
        hstack![zstack![
            text("A").key("a").width(40.).height(20.),
            text("B").key("b").width(60.).height(10.),
        ]
        .key("overlay")
        .padding(5., Edge::All)]
        .into(),
    );
    layout(&mut r, 800., 1.);
    close(bounds(&r, "overlay", 1.), [0., 0., 70., 30.]);
    close(bounds(&r, "a", 1.), [15., 5., 40., 20.]);
    close(bounds(&r, "b", 1.), [5., 10., 60., 10.]);
    let (mut r, _) = build(
        zstack![text("A").key("a").width(40.).height(20.)]
            .width(100.)
            .height(60.)
            .padding(5., Edge::All)
            .flow(Flow::BottomRight)
            .into(),
    );
    layout(&mut r, 800., 1.);
    close(bounds(&r, "a", 1.), [55., 35., 40., 20.]);
    let (mut r, _) = build(
        zstack![
            text("Fill").key("fill"),
            text("Fixed").key("fixed").width(20.).height(10.)
        ]
        .width(100.)
        .height(60.)
        .padding(5., Edge::All)
        .align(Align::Stretch)
        .into(),
    );
    layout(&mut r, 800., 1.);
    close(bounds(&r, "fill", 1.), [5., 5., 90., 50.]);
    close(bounds(&r, "fixed", 1.), [40., 25., 20., 10.]);
}

#[test]
fn flow_and_flex_follow_resolved_direction_not_the_stack_kind() {
    let (mut r, _) = build(
        stack(pair())
            .width(200.)
            .height(100.)
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
            .gap(6., Axis::Both)
            .padding(10., Edge::All)
            .flow(Flow::Center)
            .into(),
    );
    layout(&mut r, 767., 1.);
    close(bounds(&r, "a", 1.), [80., 27., 40., 20.]);
    close(bounds(&r, "b", 1.), [80., 53., 40., 20.]);
    layout(&mut r, 768., 1.);
    close(bounds(&r, "a", 1.), [57., 40., 40., 20.]);
    close(bounds(&r, "b", 1.), [103., 40., 40., 20.]);
    let (mut r, _) = build(
        stack![
            text("A").key("a").width(20.).height(10.),
            spacer().key("space"),
            text("B").key("b").width(20.).height(10.)
        ]
        .width(100.)
        .height(80.)
        .gap(5., Axis::Both)
        .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
        .into(),
    );
    layout(&mut r, 767., 1.);
    close(bounds(&r, "space", 1.), [0., 15., 100., 50.]);
    close(bounds(&r, "b", 1.), [0., 70., 20., 10.]);
    layout(&mut r, 768., 1.);
    close(bounds(&r, "space", 1.), [25., 0., 50., 80.]);
    close(bounds(&r, "b", 1.), [80., 0., 20., 10.]);
}

#[test]
fn zstack_padding_clips_hits_cancels_outside_release_and_corners_match_paint() {
    let (mut r, mut h) = build(
        zstack![
            button("Back")
                .key("back")
                .width(200.)
                .height(200.)
                .rounded(0)
                .on_click(|n: &mut usize| *n += 1),
            button("Front")
                .key("front")
                .width(40.)
                .height(40.)
                .rounded(Rounding::Full)
                .on_click(|n: &mut usize| *n += 10),
        ]
        .width(100.)
        .height(80.)
        .padding(10., Edge::All)
        .into(),
    );
    layout(&mut r, 800., 1.);
    close(bounds(&r, "back", 1.), [10., 10., 80., 60.]);
    let mut count = 0;
    r.pointer_down(5., 40.);
    assert!(
        !r.pointer_up_outcome(5., 40., &mut h, &mut count)
            .unwrap()
            .activated
    );
    r.pointer_down(50., 40.);
    assert!(
        !r.pointer_up_outcome(99., 40., &mut h, &mut count)
            .unwrap()
            .activated
    );
    r.pointer_down(30.1, 20.1); // Inside front bounds, outside its circular hit shape.
    r.pointer_up_outcome(30.1, 20.1, &mut h, &mut count)
        .unwrap();
    assert_eq!(count, 1);
    r.pointer_down(50., 40.);
    r.cancel_input();
    assert!(
        !r.pointer_up_outcome(50., 40., &mut h, &mut count)
            .unwrap()
            .activated
    );
    assert_eq!(count, 1);
    let (mut hidden, _) = build(
        zstack![text("Hidden").key("hidden")]
            .width(20.)
            .height(20.)
            .padding(15., Edge::All)
            .into(),
    );
    layout(&mut hidden, 800., 1.);
    assert!(!hidden.boxes().iter().any(|b| b.id == "hidden"));
}

#[test]
fn decorative_and_disabled_layers_pass_through_and_nested_layers_keep_tree_order() {
    let (mut r, mut h) = build(
        zstack![
            button("Back")
                .key("back")
                .width(100.)
                .height(60.)
                .on_click(|n: &mut usize| *n += 1),
            zstack![
                button("Disabled")
                    .key("disabled")
                    .enabled(false)
                    .width(100.)
                    .height(60.)
                    .on_click(|n: &mut usize| *n += 100),
                text("Decoration").width(100.).height(60.),
            ],
        ]
        .width(100.)
        .height(60.)
        .into(),
    );
    layout(&mut r, 800., 1.);
    let mut count = 0;
    r.pointer_down(50., 30.);
    r.pointer_up_outcome(50., 30., &mut h, &mut count).unwrap();
    assert_eq!(count, 1);
    r.focus_next(false);
    assert_eq!(r.focused(), Some("back"));
}

#[test]
fn fractional_custom_thresholds_and_rule_budget_boundary_are_deterministic() {
    let mut rules = responsive(1.);
    for i in 0..32 {
        rules = rules.at(Breakpoint::MinWidth(i as f32), 2.);
    }
    let (mut r, _) = build(
        stack(pair())
            .padding(rules, Edge::All)
            .direction(
                responsive(Direction::Vertical)
                    .at(Breakpoint::MinWidth(487.5), Direction::Horizontal),
            )
            .into(),
    );
    for scale in [0.5, 1.25, 1.5, 2.5] {
        for width in [487.25, 487.5, 487.75] {
            layout(&mut r, width, scale);
            close(
                bounds(&r, "b", scale),
                if width < 487.5 {
                    [2., 22., 40., 20.]
                } else {
                    [42., 2., 40., 20.]
                },
            );
        }
    }
}
