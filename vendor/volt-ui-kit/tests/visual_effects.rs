#![cfg(feature = "rust-ui")]
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Theme, Viewport,
};
#[derive(Debug, PartialEq)]
enum Draw {
    Corners([f32; 4]),
    Layer(f32),
    End,
    Fill([f32; 4]),
    Outline([f32; 4], f32),
}
struct Recorder;
impl Painter for Recorder {
    type Shape = Draw;
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(300., 200., 1.).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<Draw>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<Draw>, _: [[f32; 2]; 3], _: [f32; 4]) {}
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
    fn card_round(&self, o: &mut Vec<Draw>, _: [f32; 4], r: f32, color: [f32; 4]) {
        o.push(Draw::Corners([r; 4]));
        o.push(Draw::Fill(color));
    }
    fn card_outline(&self, o: &mut Vec<Draw>, _: [f32; 4], r: [f32; 4], w: f32, _: [f32; 4]) {
        o.push(Draw::Outline(r, w));
    }
    fn card_corners(&self, o: &mut Vec<Draw>, _: [f32; 4], r: [f32; 4], _: [f32; 4]) {
        o.push(Draw::Corners(r));
    }
}
impl TextMetrics for Recorder {
    fn measure(&mut self, _: &str, _: f32, _: f32) -> [f32; 2] {
        [40., 20.]
    }
}
impl DocumentPainter for Recorder {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
    fn ui_layer_begin(&mut self, o: &mut Vec<Draw>, v: f32) {
        o.push(Draw::Layer(v));
    }
    fn ui_layer_end(&mut self, o: &mut Vec<Draw>) {
        o.push(Draw::End);
    }
}
fn runtime(e: Element<usize>) -> Runtime {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    r
}
fn layers(r: &Runtime) -> Vec<f32> {
    r.paint(&mut Recorder, &Theme::dark())
        .into_iter()
        .flat_map(|b| b.shapes)
        .filter_map(|s| {
            if let Draw::Layer(v) = s {
                Some(v)
            } else {
                None
            }
        })
        .collect()
}
#[test]
fn arrays_and_selected_corner_hit_geometry() {
    let mut r = runtime(
        vstack![button("Save")
            .key("save")
            .width(100.)
            .height(40.)
            .rounded_on(20., Corners::Left)
            .on_click(|n: &mut usize| *n += 1)]
        .padding(10., [Edge::Top, Edge::Left, Edge::Top])
        .gap(4., [Axis::Vertical, Axis::Horizontal])
        .into(),
    );
    assert_eq!(r.boxes()[1].bounds.bounds(), [10., 10., 100., 40.]);
    assert!(!r.pointer_move(10., 10.));
    assert!(r.pointer_move(109., 10.));
    let draws: Vec<_> = r
        .paint(&mut Recorder, &Theme::dark())
        .into_iter()
        .flat_map(|b| b.shapes)
        .collect();
    assert!(draws.contains(&Draw::Corners([20., 0., 0., 20.])));
}
#[test]
fn hover_animation_interrupts_and_reduced_motion_settles() {
    let mut r = runtime(
        button("Save")
            .key("save")
            .width(100.)
            .height(40.)
            .hover_style(|s| s.opacity(0.2))
            .animate_on(Interaction::Hover, Animation::linear().duration_ms(100))
            .on_click(|_: &mut usize| {})
            .into(),
    );
    layers(&r);
    r.pointer_move(50., 20.);
    layers(&r);
    r.advance_animations(Duration::from_millis(50));
    assert!((layers(&r)[0] - 0.6).abs() < 0.01);
    assert!(r.is_animating());
    r.pointer_leave();
    layers(&r);
    r.advance_animations(Duration::from_millis(100));
    assert!((layers(&r)[0] - 0.8).abs() < 0.01);
    r.set_reduced_motion(true);
    assert!(!r.is_animating());
    assert!(layers(&r).is_empty());
}
#[test]
fn invalid_effects_are_rejected() {
    for e in [
        text::<()>("X").opacity(f32::NAN).into(),
        text("X").shadow(Shadow::new().blur(999.)).into(),
        text("X").glow(Glow::new().color("invalid")).into(),
    ] {
        assert!(Element::build(e).is_err());
    }
    // Rust-only metadata must not silently widen the existing strict JSON schema.
    for source in [
        r#"{"schema_version":1,"root":{"type":"text","id":"x","text":"x","effects":{}}}"#,
        r#"{"schema_version":1,"root":{"type":"text","id":"x","text":"x","style":{"corners":[2,2,2,2]}}}"#,
        r#"{"schema_version":1,"root":{"type":"text","id":"x","text":"x","style":{"opacity":0.5}}}"#,
    ] {
        assert!(volt_ui_kit::declarative::Document::parse(source).is_err());
    }
}
#[test]
fn fade_enter_is_visual_only_and_idle_after_completion() {
    let mut r = runtime(
        text("Entering")
            .key("enter")
            .transition(Transition::fade().duration_ms(100))
            .into(),
    );
    assert_eq!(layers(&r), vec![0.]);
    r.advance_animations(Duration::from_millis(50));
    assert_eq!(layers(&r), vec![0.5]);
    r.advance_animations(Duration::from_millis(100));
    assert!(layers(&r).is_empty());
    assert!(!r.is_animating());
}

fn draws(r: &Runtime) -> Vec<Draw> {
    r.paint(&mut Recorder, &Theme::dark())
        .into_iter()
        .flat_map(|b| b.shapes)
        .collect()
}
fn replace(r: &mut Runtime, e: Element<usize>) {
    let (d, h) = e.build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
}
#[test]
fn small_rust_default_does_not_change_json_button_default() {
    let r = runtime(button("New").on_click(|_: &mut usize| {}).into());
    assert!(draws(&r).contains(&Draw::Corners([6.; 4])));
    let mut h = volt_ui_kit::declarative::Handlers::<usize>::default();
    h.on_activate("legacy", |_: &mut usize| {}).unwrap();
    let d = volt_ui_kit::declarative::Document::parse(
        r#"{"schema_version":1,"root":{"type":"button","id":"legacy","label":"Legacy","on_activate":"legacy"}}"#,
    )
    .unwrap();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    r.layout(Recorder.viewport(), &Bindings::default(), &mut Recorder)
        .unwrap();
    assert!(draws(&r).contains(&Draw::Corners([7.; 4])));
}
#[test]
fn selected_corner_setters_union_reset_full_and_scale() {
    for scale in [1., 1.5, 2., 3.] {
        let e: Element<usize> = button("X")
            .width(100.)
            .height(40.)
            .rounded_on(12., [Corners::Top, Corners::Left, Corners::Left])
            .rounded_on(Rounding::Full, Corner::BottomRight)
            .border_color("#ffffff")
            .border_width(2.)
            .on_click(|_: &mut usize| {})
            .into();
        let (doc, h) = e.build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        r.layout(
            Viewport::new(300. * scale, 200. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Recorder,
        )
        .unwrap();
        let v = [12. * scale, 12. * scale, 20. * scale, 12. * scale];
        let d = draws(&r);
        assert!(d.contains(&Draw::Corners(v)));
        assert!(d.contains(&Draw::Outline(v, 2. * scale)));
    }
    let r = runtime(
        button("X")
            .rounded_on(15., Corners::Left)
            .rounded(4.)
            .on_click(|_: &mut usize| {})
            .into(),
    );
    assert!(draws(&r).contains(&Draw::Corners([4.; 4])));
}
#[test]
fn array_spacing_responsive_empty_and_last_selected_setter() {
    let r = runtime(
        vstack![
            text("A").key("a").height(20.),
            text("B").key("b").height(20.)
        ]
        .padding(
            responsive(8.).at(Breakpoint::MinWidth(300.), 12.),
            [Edge::Horizontal, Edge::Top],
        )
        .padding(4., &[Edge::Left, Edge::Bottom][..])
        .padding(99., [] as [Edge; 0])
        .gap(6., vec![Axis::Both, Axis::Vertical])
        .gap(99., [] as [Axis; 0])
        .into(),
    );
    assert_eq!(r.boxes()[1].bounds.bounds(), [4., 12., 284., 20.]);
    assert_eq!(r.boxes()[2].bounds.bounds(), [4., 38., 284., 20.]);
}
#[test]
fn removing_during_enter_fade_does_not_jump_and_exits_never_dispatch() {
    let mut r = runtime(
        button("Leaving")
            .key("old")
            .width(100.)
            .height(40.)
            .transition(Transition::fade().duration_ms(100))
            .on_click(|n: &mut usize| *n += 1)
            .into(),
    );
    layers(&r);
    r.advance_animations(Duration::from_millis(50));
    assert_eq!(layers(&r), vec![0.5]);
    r.pointer_down(50., 20.);
    replace(&mut r, text("Replacement").key("new").into());
    assert_eq!(layers(&r), vec![0.5]);
    let mut h = volt_ui_kit::declarative::Handlers::default();
    let mut calls = 0;
    assert!(
        !r.pointer_up_outcome(50., 20., &mut h, &mut calls)
            .unwrap()
            .activated
    );
    assert_eq!(calls, 0);
    r.advance_animations(Duration::from_millis(75));
    assert_eq!(layers(&r), vec![0.375]);
    r.advance_animations(Duration::from_millis(150));
    assert!(layers(&r).is_empty());
    assert!(!r.is_animating());
}
#[test]
fn appearance_animation_survives_same_key_recomposition_and_clock_is_monotonic() {
    let make = |color| {
        button("X")
            .key("x")
            .background(color)
            .animation(Animation::linear().duration_ms(100))
            .on_click(|_: &mut usize| {})
            .into()
    };
    let mut r = runtime(make("#000000"));
    draws(&r);
    replace(&mut r, make("#ffffff"));
    draws(&r);
    r.advance_animations(Duration::from_millis(50));
    assert!(draws(&r).contains(&Draw::Fill([0.5, 0.5, 0.5, 1.])));
    r.advance_animations(Duration::from_millis(20));
    assert!(draws(&r).contains(&Draw::Fill([0.5, 0.5, 0.5, 1.])));
    r.advance_animations(Duration::from_millis(100));
    assert!(draws(&r).contains(&Draw::Fill([1.; 4])));
    assert!(!r.is_animating());
}
#[test]
fn opacity_is_nested_group_state_not_a_hit_test_policy() {
    let mut r = runtime(
        vstack![
            button("Transparent")
                .key("x")
                .opacity(0.)
                .on_click(|_: &mut usize| {}),
            text("Sibling").opacity(0.5)
        ]
        .opacity(0.5)
        .into(),
    );
    let d = draws(&r);
    assert_eq!(d.iter().filter(|s| matches!(s, Draw::Layer(_))).count(), 3);
    assert_eq!(d.iter().filter(|s| matches!(s, Draw::End)).count(), 3);
    let [x, y, w, h] = r.boxes()[1].bounds.bounds();
    assert!(r.pointer_move(x + w / 2., y + h / 2.));
}
#[test]
fn inactive_patches_and_animation_limits_are_validated() {
    let bad: Vec<Element<()>> = vec![
        text("X").hover_style(|s| s.opacity(-1.)).into(),
        text("X")
            .animate_on(Interaction::Focus, Animation::linear().duration_ms(10_001))
            .into(),
        text("X")
            .transition(Transition::fade().duration_ms(10_001))
            .into(),
        text("X").rounded_on(f32::NAN, Corner::TopLeft).into(),
        text("X")
            .shadow(Shadow::new().offset(f32::INFINITY, 0.))
            .into(),
    ];
    for b in bad {
        assert!(b.build().is_err());
    }
    let mut r = runtime(
        text("Instant")
            .transition(Transition::fade().duration_ms(0))
            .into(),
    );
    assert!(layers(&r).is_empty());
    assert!(!r.is_animating());
    r.set_reduced_motion(true);
    assert!(layers(&r).is_empty());
}

#[test]
fn panel_hover_tracks_topmost_visual_ancestry_without_recomposition() {
    let mut r = runtime(
        panel![text("Child").height(20.)]
            .key("panel")
            .hover_style(|s| s.opacity(0.5))
            .animate_on(Interaction::Hover, Animation::linear().duration_ms(100))
            .into(),
    );
    draws(&r);
    assert!(r.pointer_move(100., 100.));
    draws(&r);
    r.advance_animations(Duration::from_millis(50));
    assert_eq!(layers(&r), vec![0.75]);
    assert!(r.pointer_leave());
    draws(&r);
    r.advance_animations(Duration::from_millis(150));
    assert!(layers(&r).is_empty());
}
#[test]
fn reinserted_fade_key_resumes_visible_opacity() {
    let make = || {
        text("A")
            .key("a")
            .transition(Transition::fade().duration_ms(100))
            .into()
    };
    let mut r = runtime(make());
    draws(&r);
    r.advance_animations(Duration::from_millis(100));
    draws(&r);
    replace(&mut r, text("B").key("b").into());
    r.advance_animations(Duration::from_millis(150));
    assert_eq!(layers(&r), vec![0.5]);
    replace(&mut r, make());
    assert_eq!(layers(&r), vec![0.5]);
    r.advance_animations(Duration::from_millis(200));
    assert_eq!(layers(&r), vec![0.75]);
}

#[test]
fn primary_labels_keep_their_paired_colour_under_an_inherited_text_colour() {
    use volt_ui_kit::ui::prelude::*;
    let ui: Element<usize> = vstack![button("Save")
        .key("save")
        .primary(true)
        .on_click(|_: &mut usize| {})]
    .color(ColorToken::BaseContent)
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    let json = format!("{:?}", document.root());
    assert!(
        json.contains("@primary-content"),
        "primary label colour must be explicit"
    );
    // Turning primary off again restores inheritance.
    let plain: Element<usize> = button("Save")
        .primary(true)
        .primary(false)
        .on_click(|_: &mut usize| {})
        .into();
    let (document, _) = plain.build().unwrap().into_parts();
    assert!(!format!("{:?}", document.root()).contains("@primary-content"));
}

#[test]
fn primary_variant_uses_the_paired_content_token() {
    use volt_ui_kit::ui::prelude::*;
    let ui: Element<usize> = vstack![button("Save")
        .variant(ButtonVariant::Primary)
        .on_click(|_: &mut usize| {})]
    .color(ColorToken::BaseContent)
    .into();
    let (document, _) = ui.build().unwrap().into_parts();
    assert!(format!("{:?}", document.root()).contains("@primary-content"));
}
