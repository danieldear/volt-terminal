#![cfg(feature = "native")]

use std::{cell::Cell, rc::Rc, time::Duration};
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Painter, Theme, Viewport,
};

fn setup(element: Element<()>, scale: f32) -> (Runtime, RasterPainter) {
    let viewport = Viewport::new(100. * scale, 80. * scale, scale).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let (document, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    (runtime, painter)
}

fn replace(runtime: &mut Runtime, painter: &mut RasterPainter, element: Element<()>) {
    let (document, handlers) = element.build().unwrap().into_parts();
    runtime
        .replace_document(document, &handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(painter.viewport(), &Bindings::default(), painter)
        .unwrap();
}

fn pixel(runtime: &Runtime, painter: &mut RasterPainter, logical: [f32; 2]) -> [u8; 4] {
    let batches = runtime.paint(painter, &Theme::dark());
    let frame = painter.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
    let scale = painter.viewport().scale;
    let x = (logical[0] * scale) as usize;
    let y = (logical[1] * scale) as usize;
    let color = frame.pixels()[y * frame.width() as usize + x];
    [color.red(), color.green(), color.blue(), color.alpha()]
}

fn close(actual: [u8; 4], expected: [u8; 4], message: &str) {
    assert!(
        actual
            .into_iter()
            .zip(expected)
            .all(|(a, b)| a.abs_diff(b) <= 1),
        "{message}: actual={actual:?}, expected={expected:?}"
    );
}

fn red_child(present: bool, opacity: f32) -> Element<()> {
    let children = if present {
        vec![panel![]
            .key("child")
            .width(60.)
            .height(40.)
            .background("#ff0000")
            .border_width(0.)
            .transition(Transition::fade().duration_ms(100))
            .into()]
    } else {
        vec![]
    };
    vstack(children).key("root").opacity(opacity).into()
}

#[test]
fn detached_child_fade_keeps_surviving_ancestor_opacity_at_each_dpi() {
    for scale in [1., 2.] {
        let (mut runtime, mut painter) = setup(red_child(true, 0.5), scale);
        pixel(&runtime, &mut painter, [30., 20.]);
        runtime.advance_animations(Duration::from_millis(100));
        let before = pixel(&runtime, &mut painter, [30., 20.]);
        close(before, [128, 0, 0, 255], "ancestor opacity before removal");
        replace(&mut runtime, &mut painter, red_child(false, 0.5));
        close(
            pixel(&runtime, &mut painter, [30., 20.]),
            before,
            "detaching child must not brighten it",
        );
        runtime.advance_animations(Duration::from_millis(150));
        close(
            pixel(&runtime, &mut painter, [30., 20.]),
            [64, 0, 0, 255],
            "exit fade retains ancestor attenuation",
        );
        runtime.advance_animations(Duration::from_millis(200));
        assert_eq!(pixel(&runtime, &mut painter, [30., 20.]), [0, 0, 0, 255]);
        assert!(!runtime.is_animating());
    }
}

#[test]
fn removing_entering_parent_preserves_descendant_mount_opacity_at_each_dpi() {
    for scale in [1., 2.] {
        let tree = vstack![panel![]
            .key("nested-child")
            .width(60.)
            .height(40.)
            .background("#ff0000")
            .border_width(0.)
            .transition(Transition::fade().duration_ms(1000))]
        .key("nested-root")
        .transition(Transition::fade().duration_ms(200))
        .into();
        let (mut runtime, mut painter) = setup(tree, scale);
        pixel(&runtime, &mut painter, [30., 20.]);
        runtime.advance_animations(Duration::from_millis(100));
        let before = pixel(&runtime, &mut painter, [30., 20.]);
        close(
            before,
            [13, 0, 0, 255],
            "nested enter groups multiply opacity",
        );
        replace(
            &mut runtime,
            &mut painter,
            vstack![].key("replacement").into(),
        );
        close(
            pixel(&runtime, &mut painter, [30., 20.]),
            before,
            "parent exit must preserve descendant enter opacity",
        );
        runtime.advance_animations(Duration::from_millis(150));
        assert!(
            pixel(&runtime, &mut painter, [30., 20.])[0] <= before[0],
            "detached subtree may fade out, never brighten"
        );
        runtime.advance_animations(Duration::from_millis(300));
        assert_eq!(pixel(&runtime, &mut painter, [30., 20.]), [0, 0, 0, 255]);
        assert!(!runtime.is_animating());
    }
}

#[test]
fn reinserting_same_key_mid_exit_resumes_visible_opacity_at_each_dpi() {
    for scale in [1., 2.] {
        let (mut runtime, mut painter) = setup(red_child(true, 1.), scale);
        pixel(&runtime, &mut painter, [30., 20.]);
        runtime.advance_animations(Duration::from_millis(100));
        pixel(&runtime, &mut painter, [30., 20.]);
        replace(&mut runtime, &mut painter, red_child(false, 1.));
        runtime.advance_animations(Duration::from_millis(150));
        let before = pixel(&runtime, &mut painter, [30., 20.]);
        close(before, [128, 0, 0, 255], "half-completed exit");
        replace(&mut runtime, &mut painter, red_child(true, 1.));
        close(
            pixel(&runtime, &mut painter, [30., 20.]),
            before,
            "reinserted key must not restart at invisible",
        );
        runtime.advance_animations(Duration::from_millis(175));
        assert!(pixel(&runtime, &mut painter, [30., 20.])[0] >= before[0]);
        runtime.advance_animations(Duration::from_millis(250));
        close(
            pixel(&runtime, &mut painter, [30., 20.]),
            [255, 0, 0, 255],
            "reinserted enter completes",
        );
        assert!(!runtime.is_animating());
    }
}

#[test]
fn decorative_header_overlay_never_steals_enabled_or_disabled_button_clicks() {
    for scale in [1., 2.] {
        for enabled in [true, false] {
            let header = zstack![
                button("Button")
                    .key("clickable")
                    .width(80.)
                    .height(40.)
                    .enabled(enabled)
                    .on_click(|_: &mut ()| {}),
                text("Overlay").key("decorative"),
            ]
            .key("header")
            .drag_region(true)
            .into();
            let (mut runtime, _) = setup(header, scale);
            let [x, y] = [50. * scale, 40. * scale];
            runtime.pointer_move(x, y);
            assert_eq!(runtime.button_state("clickable").hovered, enabled);
            assert!(
                !runtime.drag_region_hit(x, y),
                "decorative text must not turn button bounds into a window drag region"
            );
            assert!(
                runtime.drag_region_hit(5. * scale, 5. * scale),
                "exposed header remains draggable"
            );
        }
    }
}

fn overlapping_panels(include_lower: bool) -> Element<()> {
    let mut children = vec![];
    if include_lower {
        children.push(
            panel![]
                .key("lower")
                .width(60.)
                .height(40.)
                .background("#ff0000")
                .border_width(0.)
                .transition(Transition::fade().duration_ms(100))
                .into(),
        );
    }
    children.push(
        panel![]
            .key("upper")
            .width(60.)
            .height(40.)
            .background("#0000ff")
            .border_width(0.)
            .into(),
    );
    zstack(children).key("layers").into()
}

#[test]
fn detached_lower_zstack_layer_stays_below_surviving_upper_layer_at_each_dpi() {
    for scale in [1., 2.] {
        let (mut runtime, mut painter) = setup(overlapping_panels(true), scale);
        pixel(&runtime, &mut painter, [50., 40.]);
        runtime.advance_animations(Duration::from_millis(100));
        let before = pixel(&runtime, &mut painter, [50., 40.]);
        assert_eq!(before, [0, 0, 255, 255]);
        replace(&mut runtime, &mut painter, overlapping_panels(false));
        assert_eq!(
            pixel(&runtime, &mut painter, [50., 40.]),
            before,
            "departing lower layer must not jump above opaque upper sibling"
        );
        runtime.advance_animations(Duration::from_millis(150));
        assert_eq!(pixel(&runtime, &mut painter, [50., 40.]), before);
    }
}

#[test]
fn sequentially_removed_zstack_siblings_keep_their_original_paint_order() {
    fn tree(lower: bool, upper: bool) -> Element<()> {
        let mut children = vec![];
        for (present, key, color) in [(lower, "lower", "#ff0000"), (upper, "upper", "#0000ff")] {
            if present {
                children.push(
                    panel![]
                        .key(key)
                        .width(60.)
                        .height(40.)
                        .background(color)
                        .border_width(0.)
                        .transition(Transition::fade().duration_ms(100))
                        .into(),
                );
            }
        }
        zstack(children).key("layers").into()
    }
    for scale in [1., 2.] {
        let (mut runtime, mut painter) = setup(tree(true, true), scale);
        pixel(&runtime, &mut painter, [50., 40.]);
        runtime.advance_animations(Duration::from_millis(100));
        pixel(&runtime, &mut painter, [50., 40.]);
        replace(&mut runtime, &mut painter, tree(true, false));
        runtime.advance_animations(Duration::from_millis(150));
        let before = pixel(&runtime, &mut painter, [50., 40.]);
        close(
            before,
            [127, 0, 128, 255],
            "upper ghost remains above lower sibling",
        );
        replace(&mut runtime, &mut painter, tree(false, false));
        close(
            pixel(&runtime, &mut painter, [50., 40.]),
            before,
            "removing lower sibling must not place its ghost above the existing upper ghost",
        );
    }
}

#[test]
fn overlapping_children_fade_as_one_group_at_each_dpi() {
    for scale in [1., 2.] {
        let tree = zstack![
            panel![]
                .key("red")
                .width(60.)
                .height(40.)
                .background("#ff0000")
                .border_width(0.),
            panel![]
                .key("green")
                .width(60.)
                .height(40.)
                .background("#00ff00")
                .border_width(0.),
        ]
        .key("group")
        .opacity(0.5)
        .transition(Transition::fade().duration_ms(100))
        .into();
        let (mut runtime, mut painter) = setup(tree, scale);
        assert_eq!(pixel(&runtime, &mut painter, [50., 40.]), [0, 0, 0, 255]);
        runtime.advance_animations(Duration::from_millis(50));
        close(
            pixel(&runtime, &mut painter, [50., 40.]),
            [0, 64, 0, 255],
            "fade multiplies group opacity, not each overlapping child",
        );
        runtime.advance_animations(Duration::from_millis(100));
        close(
            pixel(&runtime, &mut painter, [50., 40.]),
            [0, 128, 0, 255],
            "steady group opacity",
        );
    }
}

#[test]
fn exiting_hovered_button_keeps_its_sampled_appearance_without_explicit_animation() {
    for scale in [1., 2.] {
        let control = button("Action")
            .key("action")
            .width(60.)
            .height(40.)
            .background("#404040")
            .border_width(0.)
            .transition(Transition::fade().duration_ms(100))
            .on_click(|_: &mut ()| {})
            .into();
        let (mut runtime, mut painter) = setup(control, scale);
        pixel(&runtime, &mut painter, [10., 30.]);
        runtime.advance_animations(Duration::from_millis(100));
        runtime.pointer_move(30. * scale, 20. * scale);
        let before = pixel(&runtime, &mut painter, [10., 30.]);
        assert!(before[0] > 64, "default hover feedback must be visible");
        replace(
            &mut runtime,
            &mut painter,
            vstack![].key("replacement").into(),
        );
        close(
            pixel(&runtime, &mut painter, [10., 30.]),
            before,
            "exiting button must retain captured hover appearance after input is cancelled",
        );
    }
}

#[test]
fn exiting_pressed_button_keeps_implicit_press_surface() {
    for scale in [1., 2.] {
        let control = button("Action")
            .key("action")
            .width(60.)
            .height(40.)
            .background("#aaaaaa")
            .border_width(0.)
            .transition(Transition::fade().duration_ms(100))
            .on_click(|_: &mut ()| {})
            .into();
        let (mut runtime, mut painter) = setup(control, scale);
        pixel(&runtime, &mut painter, [10., 30.]);
        runtime.advance_animations(Duration::from_millis(100));
        runtime.pointer_down(30. * scale, 20. * scale);
        let before = pixel(&runtime, &mut painter, [10., 30.]);
        assert!(before[0] < 155, "pressed feedback darkens the surface");
        replace(
            &mut runtime,
            &mut painter,
            vstack![].key("replacement").into(),
        );
        close(
            pixel(&runtime, &mut painter, [10., 30.]),
            before,
            "exit retains sampled press recipe without retaining input capture",
        );
    }
}

#[test]
fn exiting_focused_button_keeps_implicit_focus_ring() {
    for scale in [1., 2.] {
        let control = button("Action")
            .key("action")
            .width(60.)
            .height(40.)
            .background("#404040")
            .border_width(0.)
            .transition(Transition::fade().duration_ms(100))
            .on_click(|_: &mut ()| {})
            .into();
        let (mut runtime, mut painter) = setup(control, scale);
        pixel(&runtime, &mut painter, [1., 20.]);
        runtime.advance_animations(Duration::from_millis(100));
        runtime.focus_next(false);
        let before = pixel(&runtime, &mut painter, [1., 20.]);
        assert_ne!(
            before,
            [64, 64, 64, 255],
            "default focus ring is visible: {before:?}"
        );
        replace(
            &mut runtime,
            &mut painter,
            vstack![].key("replacement").into(),
        );
        close(
            pixel(&runtime, &mut painter, [1., 20.]),
            before,
            "exit retains the sampled focus ring while live focus is cleared",
        );
        assert!(runtime.focused().is_none());
    }
}

#[test]
fn exiting_control_is_paint_only_and_reinsert_uses_new_handler() {
    let old_calls = Rc::new(Cell::new(0));
    let new_calls = Rc::new(Cell::new(0));
    let control = |calls: Rc<Cell<u32>>| -> Element<()> {
        button("Action")
            .key("action")
            .width(80.)
            .height(40.)
            .transition(Transition::fade().duration_ms(100))
            .on_click(move |_: &mut ()| calls.set(calls.get() + 1))
            .into()
    };
    let viewport = Viewport::new(100., 80., 1.).unwrap();
    let mut painter = RasterPainter::new(viewport);
    let (document, old_handlers) = control(old_calls.clone()).build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &old_handlers, &Bindings::default()).unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    pixel(&runtime, &mut painter, [30., 20.]);
    runtime.advance_animations(Duration::from_millis(100));
    pixel(&runtime, &mut painter, [30., 20.]);
    let empty: Element<()> = vstack![].key("empty").into();
    let (empty_document, mut empty_handlers) = empty.build().unwrap().into_parts();
    runtime
        .replace_document(empty_document, &empty_handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    runtime.pointer_down(30., 20.);
    assert!(
        !runtime
            .pointer_up_outcome(30., 20., &mut empty_handlers, &mut ())
            .unwrap()
            .activated
    );
    assert_eq!(old_calls.get(), 0);
    runtime.advance_animations(Duration::from_millis(150));
    let (new_document, mut new_handlers) = control(new_calls.clone()).build().unwrap().into_parts();
    runtime
        .replace_document(new_document, &new_handlers, &Bindings::default())
        .unwrap();
    runtime
        .layout(viewport, &Bindings::default(), &mut painter)
        .unwrap();
    runtime.pointer_down(30., 20.);
    assert!(
        runtime
            .pointer_up_outcome(30., 20., &mut new_handlers, &mut ())
            .unwrap()
            .activated
    );
    assert_eq!(old_calls.get(), 0);
    assert_eq!(new_calls.get(), 1);
}
