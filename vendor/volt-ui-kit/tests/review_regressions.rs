#![cfg(feature = "rust-ui")]
//! Edge cases found while reviewing the component-parity delivery.
use volt_ui_kit::{
    declarative::{Bindings, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};

struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, text: &str, size: f32, max: f32) -> [f32; 2] {
        [
            (text.chars().count() as f32 * size / 2.).min(max),
            size * 1.2,
        ]
    }
}

#[test]
fn pagination_accepts_the_largest_page_without_overflow() {
    let element: Element<()> = pagination(usize::MAX, usize::MAX)
        .key("pages")
        .on_change(|_: &mut (), _| {})
        .into();
    assert!(element.build().is_ok());
}

#[test]
fn form_labels_name_controls_without_overriding_explicit_accessibility() {
    let element: Element<()> = vstack![
        form_field(
            "Email",
            text_field("ada@")
                .key("email")
                .on_change(|_: &mut (), _| {})
        )
        .key("email-field"),
        form_field(
            "Name",
            text_field("Ada")
                .key("name")
                .accessible_label("Account owner")
                .on_change(|_: &mut (), _| {})
        )
        .key("name-field"),
    ]
    .into();
    let (doc, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(400., 400., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    let nodes = runtime.accessibility_nodes();
    assert_eq!(
        nodes.iter().find(|n| n.id == "email").unwrap().label,
        "Email"
    );
    assert_eq!(
        nodes.iter().find(|n| n.id == "name").unwrap().label,
        "Account owner"
    );
}

#[cfg(feature = "native")]
#[test]
fn centered_editable_text_and_caret_stay_inside_the_field() {
    use volt_ui_kit::native::RasterPainter;
    let element: Element<()> = text_field("123")
        .key("value")
        .width(200.)
        .height(48.)
        .font_size(20.)
        .text_align(TextAlign::Center)
        .color("#ffffff")
        .background("#00000000")
        .border_width(0.)
        .on_change(|_: &mut (), _| {})
        .into();
    let (doc, handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    let vp = Viewport::new(240., 80., 1.).unwrap();
    let mut painter = RasterPainter::new(vp);
    runtime
        .layout(vp, &Bindings::default(), &mut painter)
        .unwrap();
    runtime.settle_animations();
    let batches = runtime.paint_ui(&mut painter, &UiTheme::dark());
    let pixels = painter.rasterize(&batches, [0.; 4]).unwrap();
    let ink: Vec<_> = (0..240)
        .filter(|&x| (0..80).any(|y| pixels.pixel(x, y).unwrap().alpha() > 0))
        .collect();
    assert!(!ink.is_empty(), "centered editable text must not disappear");
    assert!(
        ink[0] > 50 && *ink.last().unwrap() < 150,
        "ink is centered inside the field: {ink:?}"
    );
    assert!(runtime.accessibility_focus("value"));
    let caret = runtime.editor_candidate_bounds().unwrap();
    assert!(
        (100. ..150.).contains(&caret[0]),
        "caret follows the same aligned glyphs: {caret:?}"
    );
}

#[test]
fn disabled_offsets_are_validated_like_other_visual_states() {
    let element: Element<()> = button("Off")
        .enabled(false)
        .disabled_style(|s| s.offset(f32::NAN, 0.))
        .on_click(|_: &mut ()| {})
        .into();
    assert!(element.build().is_err());
}

#[test]
fn number_inputs_refuse_nonfinite_ranges_and_emit_finite_extreme_steps() {
    for (min, max) in [
        (f64::NEG_INFINITY, 10.),
        (0., f64::INFINITY),
        (0., f64::NAN),
    ] {
        let element: Element<f64> = number_input(0.)
            .key("n")
            .range(min, max)
            .on_change(|value: &mut f64, next| *value = next)
            .into();
        assert!(element.build().is_err());
    }
    let element: Element<f64> = number_input(f64::MAX / 2.)
        .key("n")
        .decimals(6)
        .step(f64::MAX)
        .on_change(|value: &mut f64, next| *value = next)
        .into();
    let (doc, mut handlers) = element.build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(
            Viewport::new(400., 200., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
    let [x, y, w, h] = runtime
        .boxes()
        .iter()
        .find(|b| b.id == "n-inc")
        .unwrap()
        .bounds
        .bounds();
    let mut value = 0.;
    runtime.pointer_down(x + w / 2., y + h / 2.);
    assert!(
        runtime
            .pointer_up_outcome(x + w / 2., y + h / 2., &mut handlers, &mut value)
            .unwrap()
            .activated
    );
    assert_eq!(value, f64::MAX);
}

#[test]
fn responsive_extensions_validate_inactive_values_and_thresholds() {
    let cases: Vec<Element<()>> = vec![
        text("x")
            .responsive_width(responsive(100.).at(Breakpoint::Xxl, f32::NAN))
            .into(),
        text("x")
            .responsive_font_size(responsive(14.).at(Breakpoint::Xxl, 0.))
            .into(),
        text("x")
            .visible(responsive(true).at(Breakpoint::MinWidth(f32::NAN), false))
            .into(),
        text("x")
            .visible((0..33).fold(responsive(true), |r, i| {
                r.at(Breakpoint::MinWidth(i as f32), false)
            }))
            .into(),
    ];
    for (i, element) in cases.into_iter().enumerate() {
        assert!(
            element.build().is_err(),
            "invalid responsive extension {i} accepted"
        );
    }
}

#[test]
fn date_arithmetic_rejects_extreme_offsets_without_overflow() {
    let date = CalendarDate::new(2026, 10, 12).unwrap();
    for days in [i64::MIN, i64::MAX] {
        assert!(CalendarDate::from_days_since_epoch(days).is_none());
        assert!(date.add_days(days).is_none());
    }
    for months in [i32::MIN, i32::MAX] {
        assert!(date.add_months(months).is_none());
    }
    let time = TimeOfDay::new(23, 59).unwrap();
    for minutes in [i32::MIN, i32::MAX] {
        let total = (1439_i64 + i64::from(minutes)).rem_euclid(1440);
        assert_eq!(
            time.add_minutes(minutes),
            TimeOfDay::new((total / 60) as u8, (total % 60) as u8).unwrap()
        );
    }
}

#[test]
fn file_drops_respect_modal_scope_changes() {
    let view = |modal| -> Element<usize> {
        zstack![
            vstack![drop_zone().key("drop").on_drop(|n: &mut usize, _| *n += 1)].key("parent"),
            dialog(text("Modal"))
                .key("modal")
                .open(modal)
                .on_dismiss(|_: &mut usize| {}),
        ]
        .key("root")
        .into()
    };
    let vp = Viewport::new(400., 400., 1.).unwrap();
    let (doc, handlers) = view(false).build().unwrap().into_parts();
    let mut runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
    runtime
        .layout(vp, &Bindings::default(), &mut Metrics)
        .unwrap();
    let mut calls = 0;
    assert!(runtime.file_hover(None));
    let (doc, h) = view(true).build().unwrap().into_parts();
    runtime
        .replace_document(doc, &h, &Bindings::default())
        .unwrap();
    let mut handlers = h;
    runtime
        .layout(vp, &Bindings::default(), &mut Metrics)
        .unwrap();
    let outcome = runtime
        .drop_files(None, vec!["private.txt".into()], &mut handlers, &mut calls)
        .unwrap();
    assert!(
        !outcome.activated,
        "a stale hovered zone cannot bypass the modal"
    );
    assert_eq!(calls, 0);
}
