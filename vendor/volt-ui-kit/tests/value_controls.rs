#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime, ValueKey},
    ui::prelude::*,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    One,
    Two,
    Three,
}
#[derive(Default)]
struct App {
    value: f32,
    selected: Option<Choice>,
    open: bool,
    changes: usize,
}
#[test]
fn slider_rejects_invalid_ranges_steps_and_values() {
    let make = |value| slider::<App>(value).on_change(|app, value| app.value = value);
    for value in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        assert!(Element::from(make(value)).build().is_err());
    }
    for step in [0., -1., f32::NAN, f32::INFINITY, 2.] {
        assert!(Element::from(make(0.5).step(step)).build().is_err());
    }
    assert!(Element::from(make(0.5).range(1.0..=1.0)).build().is_err());
    assert!(Element::from(slider::<App>(0.5)).build().is_err());
}
#[test]
fn unfocused_slider_keys_are_ignored() {
    let ui: Element<App> = slider(0.)
        .key("gain")
        .range(0.0..=1.0)
        .step(0.3)
        .on_change(|app: &mut App, v| {
            app.value = v;
            app.changes += 1;
        })
        .into();
    let (document, mut handlers) = ui.build().unwrap().into_parts();
    let mut runtime = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    // Unmounted/unfocused scalar controls must ignore host navigation keys.
    assert!(
        !runtime
            .slider_key(ValueKey::Increase, &mut handlers, &mut App::default())
            .unwrap()
            .activated
    );
}
#[test]
fn typed_select_rejects_invalid_or_duplicate_options_and_missing_handlers() {
    let make = || {
        select::<App, _>([
            select_item(Choice::One, "One"),
            select_item(Choice::Two, "Two").enabled(false),
        ])
        .key("invalid")
    };
    assert!(Element::from(
        make()
            .selected(Choice::Three)
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
    )
    .build()
    .is_err());
    assert!(Element::from(make().selected(Choice::One)).build().is_err());
    assert!(Element::from(
        select::<App, _>([
            select_item(Choice::One, "One"),
            select_item(Choice::One, "Duplicate")
        ])
        .key("invalid")
        .selected(Choice::One)
        .on_select(|_, _| {})
        .on_open_change(|_, _| {})
    )
    .build()
    .is_err());
}

use volt_ui_kit::{
    declarative::{DocumentPainter, Event, HandlerError, Handlers, TextMetrics},
    Painter, Viewport,
};
struct Metrics(f32);
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size * 0.5).min(width),
            size * 1.2,
        ]
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
    fn card_triangle(&self, out: &mut Vec<Self::Shape>, points: [[f32; 2]; 3], c: [f32; 4]) {
        out.push(([points[0][0], points[0][1], 0., 0.], c));
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
    let (document, handlers) = ui.build().unwrap().into_parts();
    let mut r = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    (r, handlers)
}
fn scalar(value: f32, enabled: bool) -> Element<App> {
    slider(value)
        .key("gain")
        .range(0.0..=1.0)
        .step(0.3)
        .enabled(enabled)
        .on_change(|app: &mut App, v| {
            app.value = v;
            app.changes += 1;
        })
        .into()
}
fn dropdown(open: bool) -> Element<App> {
    select([
        select_item(Choice::One, "One").key("one"),
        select_item(Choice::Two, "Unavailable")
            .key("two")
            .enabled(false),
        select_item(Choice::Three, "Three").key("three"),
    ])
    .key("choice")
    .selected(Choice::One)
    .open(open)
    .on_select(|app: &mut App, value| {
        app.selected = Some(value);
        app.changes += 1;
    })
    .on_open_change(|app: &mut App, open| app.open = open)
    .into()
}
fn click(r: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, id: &str) -> bool {
    let [x, y, w, height] = r
        .boxes()
        .iter()
        .find(|b| b.id == id)
        .unwrap()
        .bounds
        .bounds();
    r.pointer_down_outcome(x + w * 0.5, y + height * 0.5, h, app)
        .unwrap();
    r.pointer_up_outcome(x + w * 0.5, y + height * 0.5, h, app)
        .unwrap()
        .activated
}
#[test]
fn sliders_drag_beyond_bounds_and_hold_capture_until_release() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scalar(0., true), scale);
        let mut app = App::default();
        let [x, y, w, height] = r
            .boxes()
            .iter()
            .find(|b| b.id == "gain")
            .unwrap()
            .bounds
            .bounds();
        let down = r
            .pointer_down_outcome(x + 8. * scale, y + height * 0.5, &mut h, &mut app)
            .unwrap();
        assert!(!down.activated);
        assert!(
            r.pointer_move_outcome(x + w + 100., y + height + 100., &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.value, 1.);
        assert_eq!(app.changes, 1);
        assert!(
            !r.pointer_move_outcome(x + w + 200., y + height + 100., &mut h, &mut app)
                .unwrap()
                .activated
        );
        let up = r
            .pointer_up_outcome(x + w + 200., y + height + 100., &mut h, &mut app)
            .unwrap();
        assert!(up.changed);
        assert!(!up.activated);
        assert!(
            !r.pointer_move_outcome(x - 100., y, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.value, 1.);
    }
}
#[test]
fn slider_keyboard_is_range_safe_disabled_and_noop_at_boundaries() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scalar(0., true), scale);
        let mut app = App::default();
        assert!(r.focus_next(false));
        assert!(
            !r.slider_key(ValueKey::First, &mut h, &mut app)
                .unwrap()
                .activated
        );
        for _ in 0..4 {
            assert!(
                r.slider_key(ValueKey::Increase, &mut h, &mut app)
                    .unwrap()
                    .activated
            );
        }
        assert_eq!(app.value, 1.);
        assert!(
            !r.slider_key(ValueKey::Increase, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(
            r.slider_key(ValueKey::Decrease, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!((app.value - 0.9).abs() < 1e-6);
        assert!(
            r.slider_key(ValueKey::PageDecrease, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.value, 0.);
        assert!(
            r.slider_key(ValueKey::Last, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.value, 1.);
        let (mut r, mut h) = mount(scalar(0.3, false), scale);
        let mut disabled = App::default();
        assert!(!r.focus_next(false));
        assert!(
            !r.slider_key(ValueKey::Last, &mut h, &mut disabled)
                .unwrap()
                .activated
        );
        assert_eq!(disabled.changes, 0);
    }
}
#[test]
fn malformed_numeric_events_never_reach_application_callbacks() {
    let mut h = Handlers::default();
    h.on_value_changed("value", |app: &mut App, value| {
        app.value = value;
        app.changes += 1;
    })
    .unwrap();
    let mut app = App::default();
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            h.dispatch(
                "value",
                Event::ValueChanged {
                    bits: value.to_bits()
                },
                &mut app
            ),
            Err(HandlerError::InvalidValue)
        );
    }
    assert_eq!(app.changes, 0);
}
#[test]
fn sliders_paint_track_thumb_and_disabled_semantics_without_text() {
    for scale in [1., 2.] {
        let (r, _) = mount(scalar(0.3, true), scale);
        let mut p = Metrics(scale);
        let batches = r.paint_ui(&mut p, &UiTheme::dark());
        assert!(batches.iter().any(|b| !b.shapes.is_empty()));
        assert!(batches.iter().all(|b| b.glyphs.is_empty()));
        for (bounds, _) in batches.iter().flat_map(|b| &b.shapes) {
            assert!(bounds.iter().all(|v| v.is_finite()));
        }
        let enabled_alpha = batches
            .iter()
            .flat_map(|b| &b.shapes)
            .map(|(_, color)| color[3])
            .fold(0., f32::max);
        let (disabled, _) = mount(scalar(0.3, false), scale);
        let disabled_batches = disabled.paint_ui(&mut p, &UiTheme::dark());
        let disabled_alpha = disabled_batches
            .iter()
            .flat_map(|b| &b.shapes)
            .map(|(_, color)| color[3])
            .fold(0., f32::max);
        assert!(disabled_alpha > 0. && disabled_alpha < enabled_alpha);
        let (mut focused, _) = mount(scalar(0.3, true), scale);
        assert!(focused.focus_next(false));
        let focused_batches = focused.paint_ui(&mut p, &UiTheme::dark());
        assert!(
            focused_batches
                .iter()
                .map(|b| b.shapes.len())
                .sum::<usize>()
                > batches.iter().map(|b| b.shapes.len()).sum::<usize>()
        );
    }
}
#[test]
fn closed_select_does_not_mount_options_and_open_chooses_and_closes() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(dropdown(false), scale);
        let mut app = App::default();
        assert!(!r.boxes().iter().any(|b| b.id == "three"));
        assert!(click(&mut r, &mut h, &mut app, "choice-trigger"));
        assert!(app.open);
        let (mut r, mut h) = mount(dropdown(true), scale);
        app.open = true;
        assert!(!click(&mut r, &mut h, &mut app, "two"));
        assert!(app.open);
        assert_eq!(app.selected, None);
        assert!(click(&mut r, &mut h, &mut app, "three"));
        assert_eq!(app.selected, Some(Choice::Three));
        assert!(!app.open);
        assert_eq!(app.changes, 1);
    }
}
#[test]
fn selected_option_only_closes_without_spurious_selection_change() {
    let (mut r, mut h) = mount(dropdown(true), 1.);
    let mut app = App {
        open: true,
        ..App::default()
    };
    assert!(click(&mut r, &mut h, &mut app, "one"));
    assert!(!app.open);
    assert_eq!(app.changes, 0);
}

use volt_ui_kit::declarative::SelectionKey;
fn replace(r: &mut Runtime, handlers: &mut Handlers<'static, App>, ui: Element<App>, scale: f32) {
    let (document, next) = ui.build().unwrap().into_parts();
    r.replace_document(document, &next, &Bindings::default())
        .unwrap();
    *handlers = next;
    let mut p = Metrics(scale);
    r.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
#[test]
fn popup_keyboard_focus_skips_disabled_commits_only_on_activation_and_restores_trigger() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(dropdown(false), scale);
        let mut app = App::default();
        assert!(click(&mut r, &mut h, &mut app, "choice-trigger"));
        assert!(app.open);
        replace(&mut r, &mut h, dropdown(app.open), scale);
        assert_eq!(r.focused(), Some("one"));
        assert!(r.overlay_key(SelectionKey::Next));
        assert_eq!(r.focused(), Some("three"));
        assert_eq!(app.selected, None);
        assert_eq!(app.changes, 0);
        assert!(r.overlay_key(SelectionKey::First));
        assert_eq!(r.focused(), Some("one"));
        assert!(r.overlay_key(SelectionKey::Last));
        assert_eq!(r.focused(), Some("three"));
        assert!(r.key_down());
        assert!(r.key_up_outcome(&mut h, &mut app).unwrap().activated);
        assert_eq!(app.selected, Some(Choice::Three));
        assert!(!app.open);
        assert_eq!(app.changes, 1);
        replace(&mut r, &mut h, dropdown(app.open), scale);
        assert_eq!(r.focused(), Some("choice-trigger"));
        assert!(!r.overlay_key(SelectionKey::Next));
    }
}
#[test]
fn popup_escape_and_outside_press_release_dismiss_without_selecting() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(dropdown(true), scale);
        let mut app = App {
            open: true,
            ..App::default()
        };
        assert!(r.dismiss_overlay(&mut h, &mut app).unwrap().activated);
        assert!(!app.open);
        assert_eq!(app.changes, 0);
        assert_eq!(app.selected, None);
        let (mut r, mut h) = mount(dropdown(true), scale);
        app.open = true;
        assert!(
            r.pointer_down_outcome(599. * scale, 599. * scale, &mut h, &mut app)
                .unwrap()
                .changed
        );
        assert!(app.open);
        assert!(
            r.pointer_up_outcome(599. * scale, 599. * scale, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert!(!app.open);
        assert_eq!(app.changes, 0);
        assert_eq!(app.selected, None);
    }
}
fn popup_card(r: &Runtime) -> [f32; 4] {
    r.boxes()
        .iter()
        .find(|b| {
            b.id.starts_with("auto-root")
                && b.kind == volt_ui_kit::declarative::Kind::Panel
                && b.scroll.is_some()
        })
        .unwrap()
        .bounds
        .bounds()
}
#[test]
fn popup_is_anchored_below_trigger_and_flips_above_near_bottom() {
    for scale in [1., 2.] {
        let (r, _) = mount(dropdown(true), scale);
        let [x, y, w, h] = r
            .boxes()
            .iter()
            .find(|b| b.id == "choice-trigger")
            .unwrap()
            .bounds
            .bounds();
        let [px, py, pw, ph] = popup_card(&r);
        assert_eq!(px, x);
        assert_eq!(py, y + h + 4. * scale);
        assert_eq!(pw, w);
        assert!(py + ph <= 600. * scale);
        let ui: Element<App> = zstack![dropdown(true)].flow(Flow::BottomRight).into();
        let (r, _) = mount(ui, scale);
        let [x, y, w, _] = r
            .boxes()
            .iter()
            .find(|b| b.id == "choice-trigger")
            .unwrap()
            .bounds
            .bounds();
        let [px, py, pw, ph] = popup_card(&r);
        assert_eq!(px, x);
        assert_eq!(pw, w);
        assert_eq!(py + ph + 4. * scale, y);
        assert!(px >= 0. && py >= 0. && px + pw <= 600. * scale && py + ph <= 600. * scale);
    }
}
#[test]
fn popup_portal_escapes_a_clipped_parent_and_retains_option_hit_testing() {
    for scale in [1., 2.] {
        let ui: Element<App> = panel![dropdown(true)]
            .key("clip-parent")
            .width(220.)
            .height(40.)
            .padding(0., Edge::All)
            .rounded(2)
            .clip_contents(true)
            .into();
        let (mut r, mut h) = mount(ui, scale);
        let mut app = App {
            open: true,
            ..App::default()
        };
        let parent = r
            .boxes()
            .iter()
            .find(|b| b.id == "clip-parent")
            .unwrap()
            .bounds
            .bounds();
        let option = r
            .boxes()
            .iter()
            .find(|b| b.id == "three")
            .unwrap()
            .bounds
            .bounds();
        assert!(option[1] >= parent[1] + parent[3]);
        assert!(click(&mut r, &mut h, &mut app, "three"));
        assert_eq!(app.selected, Some(Choice::Three));
        assert!(!app.open);
    }
}
fn long_dropdown(selected: usize) -> Element<App> {
    select(
        (0..128).map(|index| {
            select_item(index, format!("Choice {index}")).key(format!("long-{index}"))
        }),
    )
    .key("long")
    .selected(selected)
    .open(true)
    .on_select(|app: &mut App, _| app.changes += 1)
    .on_open_change(|app: &mut App, value| app.open = value)
    .into()
}
#[test]
fn scrolling_popup_keyboard_reaches_and_reveals_last_and_first_options() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(long_dropdown(0), scale);
        let mut app = App {
            open: true,
            ..App::default()
        };
        assert_eq!(r.focused(), Some("long-0"));
        assert!(r.overlay_key(SelectionKey::Last));
        let mut p = Metrics(scale);
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        assert_eq!(r.focused(), Some("long-127"));
        assert!(r.boxes().iter().any(|b| b.id == "long-127"));
        assert!(r.key_down());
        assert!(r.key_up_outcome(&mut h, &mut app).unwrap().activated);
        assert_eq!(app.changes, 1);
        assert!(r.overlay_key(SelectionKey::First));
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        assert_eq!(r.focused(), Some("long-0"));
        assert!(r.boxes().iter().any(|b| b.id == "long-0"));
    }
}

#[test]
fn offscreen_selected_popup_option_is_revealed_on_open() {
    for scale in [1., 2.] {
        let (r, _) = mount(long_dropdown(127), scale);
        assert_eq!(r.focused(), Some("long-127"));
        assert!(r.boxes().iter().any(|b| b.id == "long-127"));
    }
}
#[test]
fn select_validation_covers_identity_callbacks_empty_and_oversized_collections() {
    let make = || select::<App, _>([select_item(0, "Zero")]).key("zero");
    let error = Element::from(make().on_select(|_, _| {}))
        .build()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("on_open_change"));
    let error = Element::from(make().on_open_change(|_, _| {}))
        .build()
        .err()
        .unwrap()
        .to_string();
    assert!(error.contains("on_select"));
    assert!(Element::from(
        select::<App, _>([select_item(0, "Zero")])
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
    )
    .build()
    .is_err());
    assert!(Element::from(
        select::<App, usize>([])
            .key("empty")
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
    )
    .build()
    .is_err());
    assert!(Element::from(
        select::<App, _>((0..129).map(|index| select_item(index, "Many")))
            .key("many")
            .on_select(|_, _| {})
            .on_open_change(|_, _| {})
    )
    .build()
    .is_err());
}
#[test]
fn select_disabled_does_not_mount_a_popup_even_when_requested_open() {
    let ui: Element<App> = select([select_item(0, "Zero").key("zero-option")])
        .key("zero")
        .enabled(false)
        .open(true)
        .on_select(|app: &mut App, _| app.changes += 1)
        .on_open_change(|app: &mut App, value| app.open = value)
        .into();
    let (mut r, mut h) = mount(ui, 1.);
    let mut app = App::default();
    assert!(!r.boxes().iter().any(|b| b.id == "zero-option"));
    assert!(!r.focus_next(false));
    assert!(!click(&mut r, &mut h, &mut app, "zero-trigger"));
    assert_eq!(app.changes, 0);
}
#[test]
fn slider_rejects_unrepresentable_steps_and_nonfinite_pointer_coordinates() {
    let make = || {
        slider::<App>(999_999_900.)
            .range(999_999_800.0..=1_000_000_000.0)
            .step(0.01)
            .on_change(|_, _| {})
    };
    assert!(Element::from(make()).build().is_err());
    let (mut r, mut h) = mount(scalar(0., true), 1.);
    let mut app = App::default();
    assert!(
        !r.pointer_down_outcome(f32::NAN, 16., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.changes, 0);
    r.pointer_down_outcome(8., 16., &mut h, &mut app).unwrap();
    assert!(
        !r.pointer_move_outcome(f32::INFINITY, 16., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.changes, 0);
    r.pointer_up_outcome(f32::NAN, 16., &mut h, &mut app)
        .unwrap();
    assert!(
        !r.pointer_move_outcome(180., 16., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.changes, 0);
}
#[test]
fn slider_capture_survives_its_controlled_recomposition_but_is_cancelled_on_disable() {
    for scale in [1., 2.] {
        let (mut r, mut h) = mount(scalar(0., true), scale);
        let mut app = App::default();
        assert!(
            r.pointer_down_outcome(90. * scale, 16. * scale, &mut h, &mut app)
                .unwrap()
                .activated
        );
        let value = app.value;
        replace(&mut r, &mut h, scalar(value, true), scale);
        assert!(
            r.pointer_move_outcome(180. * scale, 16. * scale, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.value, 1.);
        replace(&mut r, &mut h, scalar(app.value, false), scale);
        let changes = app.changes;
        assert!(
            !r.pointer_move_outcome(0., 16. * scale, &mut h, &mut app)
                .unwrap()
                .activated
        );
        assert_eq!(app.changes, changes);
    }
}
