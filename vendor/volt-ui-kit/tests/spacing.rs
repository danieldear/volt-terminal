#![cfg(feature = "declarative")]
use volt_ui_kit::{
    declarative::{Bindings, Document, Handlers, Runtime, TextMetrics},
    Viewport,
};
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        let natural = text.len() as f32 * size * 0.5;
        [
            natural.min(width),
            (natural / width.max(0.1)).ceil().max(1.) * size * 1.2,
        ]
    }
}
fn layout(doc: Document, handlers: &Handlers<'_, usize>, scale: f32) -> Runtime {
    let mut runtime = Runtime::new(doc, handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(600. * scale, 400. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    runtime
}
fn bounds(runtime: &Runtime, id: &str) -> [f32; 4] {
    runtime
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds()
}
fn close(actual: [f32; 4], expected: [f32; 4]) {
    for (a, e) in actual.into_iter().zip(expected) {
        assert!((a - e).abs() < 0.01, "{actual:?} != {expected:?}");
    }
}
#[cfg(feature = "rust-ui")]
mod rust {
    use super::*;
    use volt_ui_kit::ui::{column, prelude::*};
    fn ui(element: Element<usize>, scale: f32) -> Runtime {
        let (doc, handlers) = element.build().unwrap().into_parts();
        layout(doc, &handlers, scale)
    }
    #[test]
    fn asymmetric_edges_and_axes_scale_and_reserve_all_four_sides() {
        for scale in [1., 1.5, 2.] {
            let runtime = ui(
                column![text("Fill").key("fill").flex(1.)]
                    .width(100.)
                    .height(80.)
                    .padding(10., Edge::All)
                    .padding(20., Edge::Horizontal)
                    .padding(12., Edge::Vertical)
                    .padding(4., Edge::Top)
                    .padding(32., Edge::Right)
                    .padding(16., Edge::Bottom)
                    .padding(24., Edge::Left)
                    .into(),
                scale,
            );
            close(
                bounds(&runtime, "fill"),
                [24. * scale, 4. * scale, 44. * scale, 60. * scale],
            );
        }
    }
    #[test]
    fn modifier_order_overwrites_only_affected_edges_and_all_padding_resets_them() {
        let runtime = ui(
            column![text("Fill").key("fill").flex(1.)]
                .width(100.)
                .height(80.)
                .padding(10., Edge::All)
                .padding(30., Edge::Top)
                .padding(4., Edge::Vertical)
                .padding(30., Edge::Left)
                .padding(8., Edge::Horizontal)
                .padding(12., Edge::Right)
                .into(),
            1.,
        );
        close(bounds(&runtime, "fill"), [8., 4., 80., 72.]);
        let runtime = ui(
            column![text("Fill").key("fill").flex(1.)]
                .width(100.)
                .height(80.)
                .padding(30., Edge::Left)
                .padding(12., Edge::Vertical)
                .padding(6., Edge::All)
                .into(),
            1.,
        );
        close(bounds(&runtime, "fill"), [6., 6., 88., 68.]);
    }
    #[test]
    fn gaps_select_container_axis_with_fallback_and_never_add_outer_edge_space() {
        for scale in [1., 2.] {
            for kind in ["row", "column", "panel"] {
                let children = vec![
                    text("A").key("a").width(40.).height(20.).into(),
                    text("B").key("b").width(40.).height(20.).into(),
                ];
                let container = match kind {
                    "row" => row(children),
                    "panel" => panel(children),
                    _ => column(children),
                };
                let runtime = ui(
                    container
                        .width(200.)
                        .height(100.)
                        .gap(30., Axis::Both)
                        .gap(12., Axis::Horizontal)
                        .gap(8., Axis::Vertical)
                        .into(),
                    scale,
                );
                // Panels are padded surfaces (12px); gaps still add no outer space.
                let inset = if kind == "panel" { 12. * scale } else { 0. };
                close(
                    bounds(&runtime, "a"),
                    [inset, inset, 40. * scale, 20. * scale],
                );
                close(
                    bounds(&runtime, "b"),
                    if kind == "row" {
                        [52. * scale, 0., 40. * scale, 20. * scale]
                    } else {
                        [inset, 28. * scale + inset, 40. * scale, 20. * scale]
                    },
                );
            }
        }
        let runtime = ui(
            row![
                text("A").key("a").width(40.).height(20.),
                text("B").key("b").width(40.).height(20.)
            ]
            .gap(99., Axis::Horizontal)
            .gap(88., Axis::Vertical)
            .gap(5., Axis::Both)
            .into(),
            1.,
        );
        close(bounds(&runtime, "b"), [45., 0., 40., 20.]);
        let runtime = ui(
            row![
                text("A").key("a").width(40.).height(20.),
                text("B").key("b").width(40.).height(20.)
            ]
            .gap(5., Axis::Both)
            .gap(88., Axis::Vertical)
            .into(),
            1.,
        );
        close(bounds(&runtime, "b"), [45., 0., 40., 20.]);
    }
    #[test]
    fn intrinsic_container_size_uses_asymmetric_sums_and_nested_wrap_width() {
        let runtime = ui(
            column![row![
                text("A").key("a").width(40.).height(20.),
                text("B").key("b").width(40.).height(20.)
            ]
            .key("inner")
            .padding(7., Edge::Left)
            .padding(11., Edge::Right)
            .padding(3., Edge::Top)
            .padding(5., Edge::Bottom)
            .gap(6., Axis::Horizontal)]
            .into(),
            1.,
        );
        // Parent column stretches width, but measured height includes top + bottom.
        close(bounds(&runtime, "inner"), [0., 0., 600., 28.]);
        close(bounds(&runtime, "a"), [7., 3., 40., 20.]);
        close(bounds(&runtime, "b"), [53., 3., 40., 20.]);
        let runtime = ui(
            row![column![text("x".repeat(20)).key("wrapped").font_size(10.)]
                .key("inner")
                .width(80.)
                .padding(10., Edge::Left)
                .padding(20., Edge::Right)
                .padding(3., Edge::Top)
                .padding(5., Edge::Bottom)]
            .into(),
            1.,
        );
        close(bounds(&runtime, "inner"), [0., 0., 80., 32.]);
        close(bounds(&runtime, "wrapped"), [10., 3., 50., 24.]);
    }
    #[test]
    fn padding_and_gap_are_not_click_targets_and_overlarge_padding_hides_children() {
        let element: Element<usize> = column![
            button("Hit")
                .key("hit")
                .on_click(|count: &mut usize| *count += 1)
                .width(50.)
                .height(30.),
            text("Below").height(20.)
        ]
        .padding(20., Edge::Left)
        .padding(10., Edge::Top)
        .gap(15., Axis::Vertical)
        .into();
        let (doc, mut handlers) = element.build().unwrap().into_parts();
        let mut runtime = layout(doc, &handlers, 2.);
        let mut count = 0;
        for (x, y) in [(10., 30.), (70., 90.)] {
            runtime.pointer_down(x, y);
            assert!(
                !runtime
                    .pointer_up_outcome(x, y, &mut handlers, &mut count)
                    .unwrap()
                    .activated
            );
        }
        runtime.pointer_down(70., 50.);
        assert!(
            runtime
                .pointer_up_outcome(70., 50., &mut handlers, &mut count)
                .unwrap()
                .activated
        );
        assert_eq!(count, 1);
        let runtime = ui(
            column![text("Hidden").key("hidden")]
                .width(20.)
                .height(20.)
                .padding(16., Edge::Left)
                .padding(16., Edge::Right)
                .into(),
            1.,
        );
        assert!(!runtime.boxes().iter().any(|b| b.id == "hidden"));
    }
    #[test]
    fn every_directional_value_is_validated_including_inactive_axes() {
        for value in [-1., f32::NAN, f32::INFINITY, 16385.] {
            for element in [
                column::<usize>(vec![]).padding(value, Edge::Top),
                column(vec![]).padding(value, Edge::Right),
                column(vec![]).padding(value, Edge::Bottom),
                column(vec![]).padding(value, Edge::Left),
                column(vec![]).padding(value, Edge::Horizontal),
                column(vec![]).padding(value, Edge::Vertical),
                column(vec![]).gap(value, Axis::Horizontal),
                row(vec![]).gap(value, Axis::Vertical),
            ] {
                assert!(Element::from(element).build().is_err());
            }
        }
    }
}
#[test]
fn json_directional_fields_validate_and_scalar_documents_still_match() {
    let source = r#"{"schema_version":1,"root":{"type":"column","id":"root","width":100,"height":80,"padding":10,"padding_horizontal":20,"padding_vertical":12,"padding_top":4,"padding_right":32,"padding_bottom":16,"padding_left":24,"gap_horizontal":7,"gap_vertical":5,"children":[{"type":"text","id":"fill","text":"Fill","flex":1}]}}"#;
    let doc = Document::parse(source).unwrap();
    let runtime = layout(doc, &Handlers::default(), 1.);
    close(bounds(&runtime, "fill"), [24., 4., 44., 60.]);
    let source = r#"{"schema_version":1,"root":{"type":"column","id":"root","width":100,"height":80,"padding":10,"children":[{"type":"text","id":"fill","text":"Fill","flex":1}]}}"#;
    close(
        bounds(
            &layout(Document::parse(source).unwrap(), &Handlers::default(), 1.),
            "fill",
        ),
        [10., 10., 80., 60.],
    );
    for field in [
        "padding_horizontal",
        "padding_vertical",
        "padding_top",
        "padding_right",
        "padding_bottom",
        "padding_left",
        "gap_horizontal",
        "gap_vertical",
    ] {
        for value in [
            serde_json::json!(-1),
            serde_json::json!(16385),
            serde_json::json!("8"),
            serde_json::Value::Null,
        ] {
            let mut node = serde_json::json!({"type":"column","id":"root"});
            node[field] = value;
            let document = serde_json::json!({"schema_version":1,"root":node});
            assert!(Document::parse(&document.to_string()).is_err());
        }
        let mut node =
            serde_json::json!({"type":"text","id":"text","text":"No container modifiers"});
        node[field] = serde_json::json!(8);
        assert!(
            Document::parse(&serde_json::json!({"schema_version":1,"root":node}).to_string())
                .is_err()
        );
    }
}

#[test]
fn json_flow_full_radius_and_spacer_defaults_are_additive_and_strict() {
    let source = r#"{"schema_version":1,"root":{"type":"row","id":"root","width":100,"height":40,"flow":"center","children":[{"type":"text","id":"a","text":"A","width":20,"height":10},{"type":"spacer","id":"space"},{"type":"text","id":"b","text":"B","width":20,"height":10}]}}"#;
    let handlers = Handlers::default();
    let r = layout(Document::parse(source).unwrap(), &handlers, 1.);
    close(bounds(&r, "space"), [20., 0., 60., 40.]);
    close(bounds(&r, "a"), [0., 15., 20., 10.]);
    close(bounds(&r, "b"), [80., 15., 20., 10.]);
    let fixed = source.replace(
        "\"type\":\"spacer\",\"id\":\"space\"",
        "\"type\":\"spacer\",\"id\":\"space\",\"flex\":0,\"width\":10",
    );
    let r = layout(Document::parse(&fixed).unwrap(), &handlers, 1.);
    close(bounds(&r, "space"), [45., 0., 10., 40.]);
    close(bounds(&r, "a"), [25., 15., 20., 10.]);
    close(bounds(&r, "b"), [55., 15., 20., 10.]);
    let source = r#"{"schema_version":1,"root":{"type":"button","id":"pill","label":"Pill","width":100,"height":40,"on_activate":"click","style":{"radius":"full"}}}"#;
    let mut handlers = Handlers::default();
    handlers
        .on_activate("click", |n: &mut usize| *n += 1)
        .unwrap();
    let mut r = layout(Document::parse(source).unwrap(), &handlers, 1.);
    let mut calls = 0;
    r.pointer_down(0.1, 0.1);
    assert!(
        !r.pointer_up_outcome(0.1, 0.1, &mut handlers, &mut calls)
            .unwrap()
            .activated
    );
    r.pointer_down(50., 20.);
    assert!(
        r.pointer_up_outcome(50., 20., &mut handlers, &mut calls)
            .unwrap()
            .activated
    );
    assert_eq!(calls, 1);
    for invalid in [
        source.replace("\"radius\":\"full\"", "\"radius\":\"other\""),
        source.replace("\"radius\":\"full\"", "\"radius\":true"),
        source.replace("\"radius\":\"full\"", "\"radius\":-1"),
        source.replace("\"style\":", "\"flow\":\"center\",\"style\":"),
    ] {
        assert!(Document::parse(&invalid).is_err());
    }
    for value in ["4", "4.5", "0", "256"] {
        assert!(Document::parse(&source.replace("\"full\"", value)).is_ok());
    }
    assert!(Document::parse(
        r#"{"schema_version":1,"root":{"type":"panel","id":"p","flow":"vertical"}}"#
    )
    .is_err());
}

#[test]
fn json_stack_direction_and_zstack_are_additive_but_responsive_objects_are_rust_only() {
    let source = r#"{"schema_version":1,"root":{"type":"stack","id":"root","direction":"horizontal","gap":6,"children":[{"type":"text","id":"a","text":"A","width":40,"height":20},{"type":"text","id":"b","text":"B","width":60,"height":10}]}}"#;
    let r = layout(Document::parse(source).unwrap(), &Handlers::default(), 1.);
    close(bounds(&r, "b"), [46., 0., 60., 10.]);
    let overlay = source
        .replace("\"stack\"", "\"zstack\"")
        .replace("\"direction\":\"horizontal\",", "");
    let r = layout(Document::parse(&overlay).unwrap(), &Handlers::default(), 1.);
    close(bounds(&r, "a"), [280., 190., 40., 20.]);
    close(bounds(&r, "b"), [270., 195., 60., 10.]);
    for invalid in [
        source.replace("\"horizontal\"", "\"random\""),
        source.replace("\"stack\"", "\"column\""),
        source.replace("\"gap\":6", "\"gap\":{\"base\":6}"),
        source.replace("\"gap\":6", "\"responsive\":{}"),
    ] {
        assert!(Document::parse(&invalid).is_err());
    }
}
