#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    ui::prelude::*,
};

#[test]
fn navigation_is_controlled_typed_and_group_scoped() {
    let view: Element<u8> = hstack![
        navigation([
            navigation_item(1u8, "Inbox").key("inbox"),
            navigation_item(2, "Sent").key("sent"),
            navigation_item(3, "Archive").key("archive").enabled(false)
        ])
        .key("left")
        .selected(1)
        .on_select(|app: &mut u8, value| *app = value),
        navigation([
            navigation_item(1u8, "Inbox").key("inbox"),
            navigation_item(2, "Sent").key("sent")
        ])
        .key("bottom")
        .placement(NavigationPlacement::Bottom)
        .selected(1)
        .on_select(|app: &mut u8, value| *app = value),
    ]
    .into();
    let (doc, mut handlers) = view.build().unwrap().into_parts();
    let bindings = Bindings::default();
    let mut runtime = Runtime::new(doc, &handlers, &bindings).unwrap();
    struct Metrics;
    impl volt_ui_kit::declarative::TextMetrics for Metrics {
        fn measure(&mut self, text: &str, size: f32, width: f32) -> [f32; 2] {
            [(text.len() as f32 * size * 0.5).min(width), size * 1.2]
        }
    }
    runtime
        .layout(
            volt_ui_kit::Viewport::new(600., 400., 1.).unwrap(),
            &bindings,
            &mut Metrics,
        )
        .unwrap();
    let mut app = 1;
    assert!(
        !runtime
            .accessibility_activate("left-archive", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app, 1);
    runtime
        .accessibility_activate("left-inbox", &mut handlers, &mut app)
        .unwrap();
    assert_eq!(app, 1);
    assert!(
        runtime
            .accessibility_activate("left-sent", &mut handlers, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app, 2);
}

#[test]
fn navigation_rejects_duplicate_values_and_unstable_item_keys() {
    let duplicate: Element<()> = navigation([
        navigation_item(1u8, "A").key("a"),
        navigation_item(1, "B").key("b"),
    ])
    .key("nav")
    .on_select(|_: &mut (), _| {})
    .into();
    assert!(duplicate.build().is_err());
    let missing: Element<()> = navigation([navigation_item(1u8, "A")])
        .key("nav")
        .on_select(|_: &mut (), _| {})
        .into();
    assert!(missing.build().is_err());
}
