#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, DocumentPainter, Handlers, Runtime, TextMetrics},
    ui::prelude::*,
    Painter, Viewport,
};

#[derive(Default)]
struct App {
    open: bool,
    nested: bool,
    clicks: usize,
    dismissed: usize,
}
fn screen(app: &App) -> Element<App> {
    zstack![
        vstack![
            button("Open")
                .key("opener")
                .on_click(|app: &mut App| app.open = true),
            button("Behind")
                .key("behind")
                .on_click(|app: &mut App| app.clicks += 100),
            text("Very tall content").height(1800.),
        ]
        .scroll(Scroll::Vertical)
        .key("background-scroll")
        .flex(1.),
        dialog(vstack![
            button("Unavailable")
                .key("disabled")
                .enabled(false)
                .on_click(|_: &mut App| {}),
            button("Accept")
                .key("accept")
                .on_click(|app: &mut App| app.clicks += 1),
            button("Close")
                .key("close")
                .on_click(|app: &mut App| app.open = false),
            dialog(
                button("Nested")
                    .key("nested-action")
                    .on_click(|app: &mut App| app.clicks += 10)
            )
            .key("nested-dialog")
            .open(app.nested)
            .on_dismiss(|app: &mut App| app.nested = false),
        ])
        .key("confirmation")
        .open(app.open)
        .title("Confirm changes")
        .on_dismiss(|app: &mut App| {
            app.open = false;
            app.dismissed += 1;
        }),
    ]
    .align(Align::Stretch)
    .into()
}
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
    type Shape = ();
    type Glyph = ();
    fn viewport(&self) -> Viewport {
        Viewport::new(600. * self.0, 400. * self.0, self.0).unwrap()
    }
    fn draw_rect(&self, _: &mut Vec<()>, _: f32, _: f32, _: f32, _: f32, _: [f32; 4]) {}
    fn card_triangle(&self, _: &mut Vec<()>, _: [[f32; 2]; 3], _: [f32; 4]) {}
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
fn mount(app: &App, scale: f32) -> (Runtime, Handlers<'static, App>) {
    let (document, handlers) = screen(app).build().unwrap().into_parts();
    let mut rt = Runtime::new(document, &handlers, &Bindings::default()).unwrap();
    let mut p = Metrics(scale);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    (rt, handlers)
}
fn recompose(rt: &mut Runtime, h: &mut Handlers<'static, App>, app: &App, scale: f32) {
    let (doc, next) = screen(app).build().unwrap().into_parts();
    rt.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    *h = next;
    let mut p = Metrics(scale);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
}
fn point(rt: &Runtime, key: &str) -> [f32; 2] {
    let [x, y, w, h] = rt
        .boxes()
        .iter()
        .find(|b| b.id == key)
        .unwrap()
        .bounds
        .bounds();
    [x + w / 2., y + h / 2.]
}
fn click(rt: &mut Runtime, h: &mut Handlers<'static, App>, app: &mut App, key: &str) -> bool {
    let [x, y] = point(rt, key);
    rt.pointer_down(x, y);
    rt.pointer_up_outcome(x, y, h, app).unwrap().activated
}
#[test]
fn modal_initial_focus_traps_forward_and_backward_navigation() {
    for scale in [1., 2.] {
        let app = App {
            open: true,
            ..App::default()
        };
        let (mut rt, _) = mount(&app, scale);
        assert_eq!(rt.focused(), Some("accept"));
        rt.focus_next(false);
        assert_eq!(rt.focused(), Some("close"));
        rt.focus_next(false);
        assert_eq!(rt.focused(), Some("accept"));
        rt.focus_next(true);
        assert_eq!(rt.focused(), Some("close"));
    }
}
#[test]
fn modal_blocks_underlying_pointer_and_wheel() {
    let mut app = App {
        open: true,
        ..App::default()
    };
    let (mut rt, mut h) = mount(&app, 1.);
    assert!(!rt.scroll_at(5., 5., [0., 120.]));
    assert_eq!(rt.scroll_offset("background-scroll"), [0., 0.]);
    let [x, y] = point(&rt, "behind");
    rt.pointer_down(x, y);
    rt.pointer_up_outcome(x, y, &mut h, &mut app).unwrap();
    assert_eq!(app.clicks, 0);
}
#[test]
fn dismissal_restores_the_opener_and_cancels_stale_press() {
    let mut app = App::default();
    let (mut rt, mut h) = mount(&app, 1.);
    assert!(click(&mut rt, &mut h, &mut app, "opener"));
    recompose(&mut rt, &mut h, &app, 1.);
    assert_eq!(rt.focused(), Some("accept"));
    rt.key_down();
    assert!(rt.dismiss_overlay(&mut h, &mut app).unwrap().activated);
    recompose(&mut rt, &mut h, &app, 1.);
    assert_eq!(rt.focused(), Some("opener"));
    assert!(!rt.key_up_outcome(&mut h, &mut app).unwrap().activated);
    assert_eq!(app.clicks, 0);
    assert_eq!(app.dismissed, 1);
}
#[test]
fn outside_click_requires_matching_press_and_release() {
    let mut app = App {
        open: true,
        ..App::default()
    };
    let (mut rt, mut h) = mount(&app, 1.);
    let [x, y] = point(&rt, "accept");
    rt.pointer_down(x, y);
    assert!(
        !rt.pointer_up_outcome(2., 2., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(app.open);
    rt.pointer_down(2., 2.);
    assert!(
        !rt.pointer_up_outcome(x, y, &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(app.open);
    rt.pointer_down(2., 2.);
    assert!(
        rt.pointer_up_outcome(3., 3., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(!app.open);
}
#[test]
fn nested_modal_dismisses_only_topmost_and_restores_parent_focus() {
    let mut app = App {
        open: true,
        ..App::default()
    };
    let (mut rt, mut h) = mount(&app, 1.);
    rt.focus_next(false);
    assert_eq!(rt.focused(), Some("close"));
    app.nested = true;
    recompose(&mut rt, &mut h, &app, 1.);
    assert_eq!(rt.focused(), Some("nested-action"));
    rt.focus_next(false);
    assert_eq!(rt.focused(), Some("nested-action"));
    assert!(rt.dismiss_overlay(&mut h, &mut app).unwrap().activated);
    assert!(!app.nested);
    assert!(app.open);
    recompose(&mut rt, &mut h, &app, 1.);
    assert_eq!(rt.focused(), Some("close"));
}
#[test]
fn closed_dialog_installs_neither_hidden_controls_nor_callbacks() {
    let app = App::default();
    let (mut rt, _) = mount(&app, 1.);
    assert!(!rt.boxes().iter().any(|b| b.id == "accept"));
    rt.focus_next(false);
    assert_eq!(rt.focused(), Some("opener"));
    rt.focus_next(false);
    assert_eq!(rt.focused(), Some("behind"));
}
#[test]
fn dialog_validation_rejects_missing_dismiss_duplicate_keys_and_invalid_geometry() {
    let e: Element<App> = dialog(text("Missing")).key("missing").open(true).into();
    assert!(e.build().is_err());
    let e: Element<App> = dialog(text("Geometry"))
        .key("bad")
        .open(true)
        .width(f32::NAN)
        .on_dismiss(|_| {})
        .into();
    assert!(e.build().is_err());
    let e: Element<App> = zstack![
        dialog(text("One"))
            .key("same")
            .open(true)
            .on_dismiss(|_| {}),
        dialog(text("Two"))
            .key("same")
            .open(true)
            .on_dismiss(|_| {})
    ]
    .into();
    assert!(e.build().is_err());
}
#[test]
fn locked_dismissal_flags_block_without_leaking_events() {
    let ui: Element<App> = zstack![
        button("Behind")
            .key("behind")
            .on_click(|a: &mut App| a.clicks += 100),
        dialog(
            button("Only")
                .key("only")
                .on_click(|a: &mut App| a.clicks += 1)
        )
        .key("locked")
        .open(true)
        .backdrop_dismiss(false)
        .escape_dismiss(false)
        .on_dismiss(|a: &mut App| a.open = false)
    ]
    .align(Align::Stretch)
    .into();
    let (doc, mut h) = ui.build().unwrap().into_parts();
    let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(1.);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let mut app = App {
        open: true,
        ..App::default()
    };
    rt.pointer_down(2., 2.);
    assert!(
        !rt.pointer_up_outcome(2., 2., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(!rt.dismiss_overlay(&mut h, &mut app).unwrap().activated);
    assert!(app.open);
    assert_eq!(app.clicks, 0);
    assert_eq!(rt.focused(), Some("only"));
}

#[test]
fn portal_escapes_narrow_clipped_ancestor_and_resets_paint_depth() {
    let ui: Element<App> = vstack![
        button("Open").key("opener").on_click(|_: &mut App| {}),
        dialog(button("Accept").key("accept").on_click(|_: &mut App| {}))
            .key("portal")
            .open(true)
            .on_dismiss(|_: &mut App| {}),
    ]
    .width(80.)
    .height(100.)
    .opacity(0.25)
    .clip_contents(true)
    .into();
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(1.);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let overlay = rt.boxes().iter().find(|b| b.id == "portal").unwrap();
    assert_eq!(overlay.bounds.bounds(), [0., 0., 600., 400.]);
    let card = rt
        .boxes()
        .iter()
        .find(|b| b.id == "portal-content")
        .unwrap();
    assert_eq!(card.bounds.bounds()[2], 420.);
    assert!(card.clip.bounds()[2] > 80.);
    assert_eq!(rt.focused(), Some("accept"));
}

#[test]
fn modal_without_enabled_controls_has_no_underlying_keyboard_target() {
    let ui: Element<App> = zstack![
        button("Behind")
            .key("behind")
            .on_click(|a: &mut App| a.clicks += 100),
        dialog(
            button("Unavailable")
                .enabled(false)
                .on_click(|_: &mut App| {})
        )
        .key("no-focus")
        .open(true)
        .on_dismiss(|_: &mut App| {}),
    ]
    .align(Align::Stretch)
    .into();
    let (doc, mut h) = ui.build().unwrap().into_parts();
    let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(1.);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let mut app = App::default();
    assert_eq!(rt.focused(), None);
    assert!(!rt.focus_next(false));
    assert!(!rt.key_down());
    assert!(!rt.key_up_outcome(&mut h, &mut app).unwrap().activated);
    assert_eq!(app.clicks, 0);
}

#[test]
fn dialog_scroll_does_not_bubble_into_underlying_view() {
    let ui: Element<App> = zstack![
        vstack![text("Behind").height(2000.)]
            .scroll(Scroll::Vertical)
            .key("behind-scroll"),
        dialog(vstack![
            text("Inside").height(1200.),
            button("Last").key("last").on_click(|_: &mut App| {})
        ])
        .key("scroll-dialog")
        .open(true)
        .on_dismiss(|_: &mut App| {}),
    ]
    .align(Align::Stretch)
    .into();
    let (doc, h) = ui.build().unwrap().into_parts();
    let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    let mut p = Metrics(1.);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    let [x, y] = point(&rt, "scroll-dialog-content");
    assert!(rt.scroll_at(x, y, [0., 180.]));
    assert!(rt.scroll_offset("scroll-dialog-content")[1] > 0.);
    assert_eq!(rt.scroll_offset("behind-scroll"), [0., 0.]);
}

#[test]
fn cancelling_input_or_relayout_disarms_a_backdrop_press() {
    let mut app = App {
        open: true,
        ..App::default()
    };
    let (mut rt, mut h) = mount(&app, 1.);
    rt.pointer_down(2., 2.);
    assert!(rt.cancel_input());
    assert!(
        !rt.pointer_up_outcome(2., 2., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(app.open);
    rt.pointer_down(2., 2.);
    rt.focus_next(false);
    assert!(
        !rt.pointer_up_outcome(2., 2., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(app.open);
    rt.pointer_down(2., 2.);
    let mut metrics = Metrics(1.);
    rt.layout(
        Viewport::new(680., 440., 1.).unwrap(),
        &Bindings::default(),
        &mut metrics,
    )
    .unwrap();
    assert!(
        !rt.pointer_up_outcome(2., 2., &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert!(app.open);
    assert_eq!(app.dismissed, 0);
}

#[test]
fn closing_before_release_never_activates_a_departing_dialog_button() {
    let mut app = App {
        open: true,
        ..App::default()
    };
    let (mut rt, mut h) = mount(&app, 1.);
    let [x, y] = point(&rt, "accept");
    rt.pointer_down(x, y);
    app.open = false;
    recompose(&mut rt, &mut h, &app, 1.);
    assert!(
        !rt.pointer_up_outcome(x, y, &mut h, &mut app)
            .unwrap()
            .activated
    );
    assert_eq!(app.clicks, 0);
}

#[test]
fn removing_an_opener_while_open_does_not_restore_its_focus() {
    let mut app = App::default();
    let (mut rt, mut h) = mount(&app, 1.);
    click(&mut rt, &mut h, &mut app, "opener");
    recompose(&mut rt, &mut h, &app, 1.);
    let ui: Element<App> = zstack![
        text("The original opener has been removed"),
        dialog(button("Inside").key("inside").on_click(|_: &mut App| {}))
            .key("confirmation")
            .open(true)
            .on_dismiss(|a: &mut App| a.open = false),
    ]
    .into();
    let (doc, next) = ui.build().unwrap().into_parts();
    rt.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    h = next;
    let mut p = Metrics(1.);
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    rt.dismiss_overlay(&mut h, &mut app).unwrap();
    let (doc, next) = Element::from(text::<App>("No controls left"))
        .build()
        .unwrap()
        .into_parts();
    rt.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    h = next;
    rt.layout(p.viewport(), &Bindings::default(), &mut p)
        .unwrap();
    assert_eq!(rt.focused(), None);
    assert!(!rt.key_down());
    assert!(!rt.key_up_outcome(&mut h, &mut app).unwrap().activated);
}

#[cfg(feature = "native")]
#[test]
fn portal_paint_is_not_clipped_or_faded_by_its_original_parent() {
    use volt_ui_kit::native::RasterPainter;
    for scale in [1., 2.] {
        let ui: Element<App> = vstack![
            text("Original parent"),
            dialog(text("Modal"))
                .key("paint-portal")
                .open(true)
                .background("#FFFFFF")
                .color("#000000")
                .border_width(0.)
                .rounded(0)
                .on_dismiss(|_: &mut App| {}),
        ]
        .width(80.)
        .height(100.)
        .opacity(0.25)
        .clip_contents(true)
        .into();
        let (doc, h) = ui.build().unwrap().into_parts();
        let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        let viewport = Viewport::new(600. * scale, 400. * scale, scale).unwrap();
        let mut p = RasterPainter::new(viewport);
        rt.layout(viewport, &Bindings::default(), &mut p).unwrap();
        let card = rt
            .boxes()
            .iter()
            .find(|b| b.id == "paint-portal-content")
            .unwrap();
        let [x, y, w, height] = card.bounds.bounds();
        let sample = [(x + w - 12. * scale) as u32, (y + height / 2.) as u32];
        let batches = rt.paint_ui(&mut p, &UiTheme::dark());
        let pixels = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
        let pixel = pixels.pixel(sample[0], sample[1]).unwrap();
        assert!(pixel.red()>240 && pixel.green()>240 && pixel.blue()>240,
            "portal inherited its original parent's clipping or quarter-opacity at {scale}x: {pixel:?}");
    }
}
