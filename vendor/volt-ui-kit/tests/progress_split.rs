#![cfg(feature = "rust-ui")]
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App {
    ratio: f32,
    changes: usize,
}
#[test]
fn split_is_a_controlled_three_part_layout() {
    let ui: Element<App> = split(text("Left"), text("Right"))
        .key("panes")
        .ratio(0.4)
        .min_first(80.)
        .min_second(100.)
        .on_resize(|app: &mut App, ratio| app.ratio = ratio)
        .into();
    let (document, _) = ui.build().unwrap().into_parts();
    assert_eq!(document.root().children().len(), 3);
}
#[test]
fn progress_accepts_valid_and_rejects_invalid_values() {
    assert!(Element::from(progress::<()>(0.4)).build().is_ok());
    for value in [f32::NAN, -0.1, 1.1] {
        assert!(Element::from(progress::<()>(value)).build().is_err());
    }
    assert!(Element::from(spinner::<()>()).build().is_ok());
    assert!(Element::from(progress_indeterminate::<()>())
        .build()
        .is_ok());
}
use std::time::Duration;
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics, ValueKey},
    Painter, Viewport,
};
struct Metrics(f32);
impl TextMetrics for Metrics {
    fn measure(&mut self, s: &str, size: f32, width: f32) -> [f32; 2] {
        [(s.len() as f32 * size * 0.5).min(width), size * 1.2]
    }
}
impl Painter for Metrics {
    type Shape = ([f32; 4], [f32; 4]);
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600. * self.0, 600. * self.0, self.0).unwrap()
    }
    fn draw_rect(&self, out: &mut Vec<Self::Shape>, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        out.push(([x, y, w, h], c));
    }
    fn card_triangle(&self, out: &mut Vec<Self::Shape>, p: [[f32; 2]; 3], c: [f32; 4]) {
        out.push(([p[0][0], p[0][1], 0., 0.], c));
    }
    fn card_round(&self, out: &mut Vec<Self::Shape>, r: [f32; 4], _: f32, c: [f32; 4]) {
        out.push((r, c));
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
impl DocumentPainter for Metrics {
    fn ui_text(&mut self, _: &mut Vec<()>, _: &str, _: [f32; 4], _: f32, _: [f32; 4]) {}
}
fn mount(ui: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    (r, h)
}
fn replace(r: &mut Runtime, h: &mut Handlers<'static, App>, ui: Element<App>, scale: f32) {
    let (d, next) = ui.build().unwrap().into_parts();
    r.replace_document(d, &next, &Bindings::default()).unwrap();
    *h = next;
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
fn panes(ratio: f32, axis: Direction, enabled: bool) -> Element<App> {
    split(text("First").key("first"), text("Second").key("second"))
        .key("panes")
        .axis(axis)
        .ratio(ratio)
        .min_first(80.)
        .min_second(100.)
        .enabled(enabled)
        .on_resize(|app: &mut App, value| {
            app.ratio = value;
            app.changes += 1;
        })
        .into()
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
fn split_drag_has_capture_clamps_to_minima_and_survives_recomposition() {
    for scale in [1., 2.] {
        for axis in [Direction::Horizontal, Direction::Vertical] {
            let mut app = App {
                ratio: 0.4,
                changes: 0,
            };
            let (mut r, mut h) = mount(panes(app.ratio, axis, true), scale);
            let [x, y, w, height] = bounds(&r, "panes-divider");
            let px = x + w / 2.;
            let py = y + height / 2.;
            assert!(
                !r.pointer_down_outcome(px, py, &mut h, &mut app)
                    .unwrap()
                    .activated
            );
            assert_eq!(app.changes, 0);
            let (nx, ny) = if axis == Direction::Horizontal {
                (px + 70. * scale, py)
            } else {
                (px, py + 70. * scale)
            };
            // Moves update application state and request layout only; the
            // host recomposes once when the drag ends.
            let moved = r.pointer_move_outcome(nx, ny, &mut h, &mut app).unwrap();
            assert!(moved.changed && !moved.activated);
            assert!(app.ratio > 0.4);
            replace(&mut r, &mut h, panes(app.ratio, axis, true), scale);
            assert!(
                r.pointer_move_outcome(-1000., -1000., &mut h, &mut app)
                    .unwrap()
                    .changed
            );
            replace(&mut r, &mut h, panes(app.ratio, axis, true), scale);
            let first = bounds(&r, "first");
            let second = bounds(&r, "second");
            let a = usize::from(axis == Direction::Vertical);
            assert!((first[a + 2] / scale - 80.).abs() < 0.01);
            assert!(second[a + 2] / scale >= 99.99);
            assert!(
                r.pointer_up_outcome(-1000., -1000., &mut h, &mut app)
                    .unwrap()
                    .activated,
                "a drag that resized recomposes once on release"
            );
            let changes = app.changes;
            assert!(
                !r.pointer_move_outcome(1000., 1000., &mut h, &mut app)
                    .unwrap()
                    .activated
            );
            assert_eq!(app.changes, changes);
        }
    }
}
#[test]
fn split_keyboard_is_real_controlled_resize_and_disabled_handle_cannot_resize() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(panes(0.4, Direction::Horizontal, true), scale);
        let mut app = App {
            ratio: 0.4,
            changes: 0,
        };
        assert!(r.focus_next(false));
        assert_eq!(r.focused(), Some("panes-divider"));
        assert!(
            r.split_key(ValueKey::Increase, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(app.ratio > 0.4);
        assert!(
            r.split_key(ValueKey::First, &mut h, &mut app)
                .unwrap()
                .activated
        );
        let old = app.changes;
        assert!(
            !r.split_key(ValueKey::First, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.changes, old);
        assert!(
            r.split_key(ValueKey::Last, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(app.ratio < 1.);
        let (mut r, mut h) = mount(panes(0.4, Direction::Horizontal, false), scale);
        assert!(!r.focus_next(false));
        assert!(
            !r.split_key(ValueKey::Increase, &mut h, &mut app)
                .unwrap()
                .activated
        );
    }
}
#[test]
fn split_unsatisfiable_minima_never_overlap_or_produce_negative_geometry() {
    for scale in [1., 2.] {
        let ui: Element<App> = split(text("First").key("first"), text("Second").key("second"))
            .key("panes")
            .width(100.)
            .height(80.)
            .min_first(400.)
            .min_second(400.)
            .on_resize(|_: &mut App, _| {})
            .into();
        let (r, _) = mount(ui, scale);
        let first = bounds(&r, "first");
        let divider = bounds(&r, "panes-divider");
        let second = bounds(&r, "second");
        assert!(first[2] >= 0. && second[2] >= 0.);
        assert!((first[2] + divider[2] + second[2] - 100. * scale).abs() < 0.01);
        assert!((second[0] - (first[0] + first[2] + divider[2])).abs() < 0.01);
    }
}
#[test]
fn split_invalid_ratio_geometry_missing_key_and_callback_fail_before_mount() {
    let make = || {
        split::<App>(text("A"), text("B"))
            .key("split")
            .on_resize(|_, _| {})
    };
    for ratio in [f32::NAN, -0.1, 1.1] {
        assert!(Element::from(make().ratio(ratio)).build().is_err());
    }
    assert!(Element::from(make().handle_size(0.)).build().is_err());
    assert!(
        Element::from(split::<App>(text("A"), text("B")).on_resize(|_, _| {}))
            .build()
            .is_err()
    );
    assert!(
        Element::from(split::<App>(text("A"), text("B")).key("split"))
            .build()
            .is_err()
    );
}
#[test]
fn zero_progress_has_a_visible_track_and_determinate_fill_is_proportional() {
    for scale in [1., 2.] {
        let (r, _) = mount(progress(0.).into(), scale);
        let mut p = Metrics(scale);
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let shapes: Vec<_> = batches.iter().flat_map(|b| &b.shapes).collect();
        assert!(!shapes.is_empty());
        assert!(shapes.iter().any(|(_, color)| color[3] > 0.));
        assert!(!r.is_animating());
        let (r, _) = mount(progress(0.25).into(), scale);
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        let shapes: Vec<_> = batches.iter().flat_map(|b| &b.shapes).collect();
        assert!(shapes
            .iter()
            .any(|(rect, color)| (rect[2] - 45. * scale).abs() < 0.01 && color[3] > 0.));
    }
}
#[test]
fn indeterminate_progress_and_spinner_animate_and_reduced_motion_stops_them() {
    for scale in [1., 2.] {
        for ui in [
            Element::from(progress_indeterminate()),
            Element::from(spinner()),
        ] {
            let (mut r, _) = mount(ui, scale);
            let mut p = Metrics(scale);
            assert!(r.is_animating());
            let before = r.paint_ui(&mut p, &UiTheme::dark());
            assert!(r.advance_animations(Duration::from_millis(250)));
            let after = r.paint_ui(&mut p, &UiTheme::dark());
            assert!(before != after);
            r.set_reduced_motion(true);
            assert!(!r.is_animating());
            let frozen = r.paint_ui(&mut p, &UiTheme::dark());
            assert!(!r.advance_animations(Duration::from_secs(2)));
            let later = r.paint_ui(&mut p, &UiTheme::dark());
            assert!(frozen == later);
        }
    }
}
#[test]
fn disabled_loading_is_static_and_invalid_periods_are_rejected() {
    let (r, _) = mount(spinner().enabled(false).into(), 1.);
    assert!(!r.is_animating());
    for period in [0, 99, 60_001] {
        assert!(Element::from(spinner::<()>().period_ms(period))
            .build()
            .is_err());
    }
    let ui: Element<App> = loading("Preparing records").into();
    assert!(ui.build().is_ok());
}
