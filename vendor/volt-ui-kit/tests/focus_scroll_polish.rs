#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
#[derive(Default)]
struct App {
    clicks: usize,
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, max: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(max), size * 1.2]
    }
}
fn layout(r: &mut Runtime, scale: f32) {
    r.layout(
        Viewport::new(300. * scale, 160. * scale, scale).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
}
fn mount(e: Element<App>, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    layout(&mut r, scale);
    (r, h)
}
fn scroll(hidden: bool) -> Element<App> {
    vstack((0..12).map(|i| {
        Element::from(
            button(format!("Action {i}"))
                .key(format!("action-{i}"))
                .height(30.)
                .enabled(i != 5)
                .on_click(|a: &mut App| a.clicks += 1),
        )
    }))
    .key("scroll")
    .height(100.)
    .width(240.)
    .gap(4., Axis::Vertical)
    .scroll(Scroll::Vertical)
    .scrollbars(if hidden {
        Scrollbars::Hidden
    } else {
        Scrollbars::Auto
    })
    .into()
}
#[test]
fn keyboard_navigation_reveals_every_offscreen_control_and_skips_disabled() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        for i in (0..12).filter(|i| *i != 5) {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("action-{i}").as_str()));
            let b = r
                .boxes()
                .iter()
                .find(|b| b.id == format!("action-{i}"))
                .unwrap();
            let [x, y, w, h] = b.bounds.bounds();
            let [cx, cy, cw, ch] = b.clip.bounds();
            assert!(x >= cx && y >= cy && x + w <= cx + cw + 0.1 && y + h <= cy + ch + 0.1);
        }
        assert!(r.scroll_offset("scroll")[1] > 0.);
        r.key_down();
        r.key_up_outcome(&mut h, &mut a).unwrap();
        assert_eq!(a.clicks, 1);
        r.focus_next(false);
        layout(&mut r, scale);
        assert_eq!(r.focused(), Some("action-0"));
        assert_eq!(r.scroll_offset("scroll")[1], 0.);
    }
}
#[test]
fn visible_scrollbar_drag_has_capture_and_hidden_bars_do_not_intercept() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("scroll")[1] > 100.);
        let old = r.scroll_offset("scroll")[1];
        r.cancel_input();
        r.pointer_move_outcome(235. * scale, 20. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll")[1], old);
        assert_eq!(a.clicks, 0);
        let (mut r, mut h) = mount(scroll(true), scale);
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll")[1], 0.);
    }
}
#[test]
fn keyboard_focus_reveals_nested_scroll_ancestors() {
    for scale in [1., 2.] {
        let e: Element<App> = vstack![
            spacer().height(160.).flex(0.),
            vstack((0..8).map(|i| {
                Element::from(
                    button("Nested")
                        .key(format!("nested-{i}"))
                        .height(30.)
                        .on_click(|a: &mut App| a.clicks += 1),
                )
            }))
            .key("inner")
            .height(90.)
            .scroll(Scroll::Vertical),
            spacer().height(200.).flex(0.)
        ]
        .key("outer")
        .height(130.)
        .scroll(Scroll::Vertical)
        .into();
        let (mut r, _) = mount(e, scale);
        for i in 0..8 {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("nested-{i}").as_str()));
            let b = r
                .boxes()
                .iter()
                .find(|b| b.id == format!("nested-{i}"))
                .unwrap();
            let [x, y, w, h] = b.bounds.bounds();
            let [cx, cy, cw, ch] = b.clip.bounds();
            assert!(x >= cx && y >= cy && x + w <= cx + cw + 0.1 && y + h <= cy + ch + 0.1);
        }
        assert!(r.scroll_offset("outer")[1] > 0.);
        assert!(r.scroll_offset("inner")[1] > 0.);
    }
}
#[test]
fn horizontal_keyboard_reveal_and_scrollbar_drag_work_at_both_scales() {
    for scale in [1., 2.] {
        let e: Element<App> = hstack((0..6).map(|i| {
            Element::from(
                button("Action")
                    .key(format!("h-{i}"))
                    .width(80.)
                    .height(30.)
                    .on_click(|a: &mut App| a.clicks += 1),
            )
        }))
        .key("horizontal")
        .width(240.)
        .height(50.)
        .scroll(Scroll::Horizontal)
        .into();
        let (mut r, mut h) = mount(e, scale);
        let mut a = App::default();
        for i in 0..6 {
            r.focus_next(false);
            layout(&mut r, scale);
            assert_eq!(r.focused(), Some(format!("h-{i}").as_str()));
        }
        assert!(r.scroll_offset("horizontal")[0] > 200.);
        assert!(
            r.pointer_down_outcome(210. * scale, 45. * scale, &mut h, &mut a)
                .unwrap()
                .changed
        );
        r.pointer_move_outcome(35. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("horizontal")[0] < 50.);
        r.pointer_up_outcome(35. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        let old = r.scroll_offset("horizontal");
        r.pointer_move_outcome(210. * scale, 45. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("horizontal"), old);
    }
}
#[test]
fn scrollbar_capture_survives_compatible_recomposition_but_resize_and_tab_disarm_it() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scroll(false), scale);
        let mut a = App::default();
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        let (d, new_h) = scroll(false).build().unwrap().into_parts();
        r.replace_document(d, &new_h, &Bindings::default()).unwrap();
        h = new_h;
        layout(&mut r, scale);
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert!(r.scroll_offset("scroll")[1] > 100.);
        r.focus_next(false);
        layout(&mut r, scale);
        let old = r.scroll_offset("scroll");
        r.pointer_move_outcome(235. * scale, 20. * scale, &mut h, &mut a)
            .unwrap();
        layout(&mut r, scale);
        assert_eq!(r.scroll_offset("scroll"), old);
        r.pointer_down_outcome(235. * scale, 12. * scale, &mut h, &mut a)
            .unwrap();
        r.invalidate_layout();
        r.layout(
            Viewport::new(310. * scale, 160. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        let old = r.scroll_offset("scroll");
        r.pointer_move_outcome(235. * scale, 80. * scale, &mut h, &mut a)
            .unwrap();
        assert_eq!(r.scroll_offset("scroll"), old);
    }
}
#[test]
fn offscreen_controls_do_not_create_invisible_click_targets() {
    let (mut r, mut h) = mount(scroll(false), 1.);
    let mut a = App::default();
    assert!(!r.boxes().iter().any(|b| b.id == "action-10"));
    r.pointer_down_outcome(30., 145., &mut h, &mut a).unwrap();
    assert!(
        !r.pointer_up_outcome(30., 145., &mut h, &mut a)
            .unwrap()
            .activated
    );
    assert_eq!(a.clicks, 0);
}
#[test]
fn keyboard_activation_disarms_a_held_scrollbar_gesture() {
    let (mut r, mut h) = mount(scroll(false), 1.);
    let mut a = App::default();
    r.focus_next(false);
    layout(&mut r, 1.);
    r.pointer_down_outcome(235., 12., &mut h, &mut a).unwrap();
    assert!(r.key_down());
    r.key_up_outcome(&mut h, &mut a).unwrap();
    let old = r.scroll_offset("scroll");
    r.pointer_move_outcome(235., 80., &mut h, &mut a).unwrap();
    layout(&mut r, 1.);
    assert_eq!(r.scroll_offset("scroll"), old);
    assert_eq!(a.clicks, 1);
}
#[test]
fn typing_into_a_mounted_offscreen_focused_editor_reveals_it_and_dispatches() {
    for scale in [1., 2.] {
        let ui: Element<App> = vstack![
            text_field("text")
                .key("field")
                .height(36.)
                .on_change(|a: &mut App, _| a.clicks += 1),
            spacer().height(400.).flex(0.)
        ]
        .key("scroller")
        .height(100.)
        .scroll(Scroll::Vertical)
        .into();
        let (mut r, mut h) = mount(ui, scale);
        let mut app = App::default();
        r.focus_next(false);
        assert_eq!(r.focused(), Some("field"));
        assert!(r.scroll_at(20. * scale, 20. * scale, [0., 200.]));
        layout(&mut r, scale);
        assert!(!r.boxes().iter().any(|b| b.id == "field"));
        assert!(r.editor_candidate_bounds().is_none());
        assert!(r.text_input("!", &mut h, &mut app).unwrap().activated);
        layout(&mut r, scale);
        assert_eq!(app.clicks, 1);
        assert!(r.boxes().iter().any(|b| b.id == "field"));
        assert_eq!(r.scroll_offset("scroller")[1], 0.);
    }
}

fn transcript(lines: usize) -> Element<App> {
    vstack((0..lines).map(|i| {
        Element::from(
            text(format!("Line {i}"))
                .key(format!("line-{i}"))
                .height(20.),
        )
    }))
    .key("log")
    .height(100.)
    .width(240.)
    .scroll(Scroll::Vertical)
    .anchor_end(true)
    .into()
}
fn grow(r: &mut Runtime, lines: usize) {
    let (d, h) = transcript(lines).build().unwrap().into_parts();
    r.replace_document(d, &h, &Bindings::default()).unwrap();
    layout(r, 1.);
}
#[test]
fn end_anchored_scroll_follows_growth_until_the_reader_scrolls_away() {
    let (mut r, _) = mount(transcript(10), 1.);
    // New views start at the end: 10 × 20 − 100.
    assert_eq!(r.scroll_offset("log")[1], 100.);
    grow(&mut r, 12);
    assert_eq!(r.scroll_offset("log")[1], 140.);
    // Scrolling up releases the anchor; growth no longer moves the view.
    assert!(r.scroll_at(20., 20., [0., -60.]));
    layout(&mut r, 1.);
    grow(&mut r, 15);
    assert_eq!(r.scroll_offset("log")[1], 80.);
    // Returning to the end re-engages it.
    assert!(r.scroll_at(20., 20., [0., 1000.]));
    layout(&mut r, 1.);
    assert_eq!(r.scroll_offset("log")[1], 200.);
    grow(&mut r, 20);
    assert_eq!(r.scroll_offset("log")[1], 300.);
}
