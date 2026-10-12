use super::*;
#[test]
fn local_conversations_are_bounded_and_keyed() {
    let mut app = App::default();
    let initial = app.active;
    for _ in 0..20 {
        app.new_chat();
    }
    assert_eq!(app.threads.len(), 6);
    app.select(initial);
    assert_ne!(app.active, initial);
    for _ in 0..30 {
        app.send();
    }
    assert!(app.current().messages.len() <= 16);
    let mut ids = std::collections::BTreeSet::new();
    for m in &app.current().messages {
        assert!(ids.insert(m.id));
    }
    view(&app).build().unwrap();
}
#[test]
fn cli_compact_and_appearance() {
    let (app, options) =
        parse_args(["--compact", "--theme", "light", "--size", "400x760"].map(str::to_owned))
            .unwrap();
    assert!(!app.sidebar && !app.inspector);
    assert_eq!(
        theme(&app, volt_ui_kit::ThemeMode::Dark).palette.name,
        "Desk light"
    );
    assert!(options.args.iter().any(|a| a == "400x760"));
    assert!(parse_args(["--theme", "invalid"].map(str::to_owned)).is_err());
}

use volt_ui_kit::{
    declarative::{Bindings, Handlers, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Painter, Viewport,
};
struct Harness {
    app: App,
    runtime: Runtime,
    handlers: Handlers<'static, App>,
    painter: RasterPainter,
}
impl Harness {
    fn new(app: App, w: f32, h: f32, scale: f32) -> Self {
        let (doc, handlers) = view(&app).build().unwrap().into_parts();
        let runtime = Runtime::new(doc, &handlers, &Bindings::default()).unwrap();
        let mut this = Self {
            app,
            runtime,
            handlers,
            painter: RasterPainter::new(Viewport::new(w * scale, h * scale, scale).unwrap()),
        };
        this.layout();
        this
    }
    fn layout(&mut self) {
        self.runtime
            .layout(
                self.painter.viewport(),
                &Bindings::default(),
                &mut self.painter,
            )
            .unwrap();
    }
    fn compose(&mut self) {
        let (doc, handlers) = view(&self.app).build().unwrap().into_parts();
        self.runtime
            .replace_document(doc, &handlers, &Bindings::default())
            .unwrap();
        self.handlers = handlers;
        self.layout();
    }
    fn bounds(&self, key: &str) -> [f32; 4] {
        self.runtime
            .boxes()
            .iter()
            .find(|b| b.id == key)
            .unwrap_or_else(|| panic!("missing {key}"))
            .bounds
            .bounds()
    }
    fn click(&mut self, key: &str) {
        let [x, y, w, h] = self.bounds(key);
        self.runtime.pointer_down(x + w / 2., y + h / 2.);
        assert!(
            self.runtime
                .pointer_up_outcome(x + w / 2., y + h / 2., &mut self.handlers, &mut self.app)
                .unwrap()
                .activated,
            "unreachable {key}"
        );
        self.compose();
    }
    fn end(&mut self, key: &str) {
        let b = self.runtime.boxes().iter().find(|b| b.id == key).unwrap();
        let [x, y, w, _] = b.bounds.bounds();
        let d = b.scroll_max[1] - b.scroll_offset[1];
        self.runtime.scroll_at(x + w - 2., y + 2., [0., d]);
        self.layout();
    }
}
#[test]
fn native_panes_and_composer_are_bounded_at_desktop_and_compact_dpi() {
    for (width, compact) in [(1360., false), (960., false), (400., true), (320., true)] {
        for scale in [1., 2.] {
            let app = App {
                sidebar: !compact,
                inspector: !compact,
                ..App::default()
            };
            let mut h = Harness::new(app, width, 850., scale);
            let center = h.bounds("desk-center").map(|v| v / scale);
            assert!(center[2] > 250., "center shrank: {center:?}");
            assert!((center[2] - (width - if compact { 0. } else { 474. })).abs() < 1.);
            let composer = h.bounds("desk-composer").map(|v| v / scale);
            assert!(composer[1] + composer[3] <= 825. && composer[3] > 90.);
            if !compact {
                assert!((h.bounds("desk-sidebar")[2] / scale - 224.).abs() < 1.);
                assert!((h.bounds("desk-inspector")[2] / scale - 248.).abs() < 1.);
            }
            h.click("desk-send");
            assert_eq!(h.app.sent, 1);
            let theme = theme(&h.app, ThemeMode::Dark);
            let batches = h.runtime.paint_ui(&mut h.painter, &theme);
            h.painter
                .rasterize(&batches, theme.palette.base[0])
                .unwrap();
        }
    }
}
#[test]
fn conversation_navigation_and_panel_actions_dispatch_without_losing_messages() {
    let mut h = Harness::new(App::default(), 1360., 960., 1.);
    h.click("desk-send");
    h.click("desk-prompt-next");
    h.click("desk-send");
    assert_eq!(h.app.current().messages.len(), 7);
    h.click("desk-thread-2");
    assert_eq!(h.app.active, 2);
    h.click("desk-thread-1");
    assert_eq!(h.app.current().messages.len(), 7);
    h.click("desk-inspector-toggle");
    assert!(!h.app.inspector);
    h.click("desk-sidebar-toggle");
    assert!(!h.app.sidebar);
    assert!((h.bounds("desk-center")[2] - 1360.).abs() < 1.);
    h.click("desk-theme");
    assert_eq!(h.app.theme, ThemeMode::Light);
    h.click("desk-sidebar-toggle");
    h.click("desk-inspector-toggle");
    h.click("desk-save");
    assert!(h.app.saved);
    h.click("desk-new");
    assert!(h.app.current().messages.is_empty());
    h.click("desk-send");
    assert_eq!(h.app.current().messages.len(), 2);
}
#[test]
fn conversation_scroll_reaches_new_messages_and_code_can_collapse() {
    let mut h = Harness::new(App::default(), 1360., 850., 2.);
    for _ in 0..10 {
        h.click("desk-send");
    }
    h.end("desk-conversation-1");
    let last = h.app.current().messages.last().unwrap().id;
    assert!(h
        .runtime
        .boxes()
        .iter()
        .any(|b| b.id == format!("desk-message-{last}")));
    h.click("desk-hide-code");
    assert!(!h.app.show_code);
    h.end("desk-conversation-1");
    h.click("desk-show-code");
    assert!(h.app.show_code);
}

#[test]
fn composer_sends_edited_unicode_text_and_blank_drafts_do_not_send() {
    for scale in [1., 2.] {
        let mut h = Harness::new(
            App {
                sidebar: false,
                inspector: false,
                ..App::default()
            },
            400.,
            850.,
            scale,
        );
        let [x, y, w, height] = h.bounds("desk-prompt");
        h.runtime.pointer_down(x + w / 2., y + height / 2.);
        h.runtime.editor_select_all();
        h.runtime
            .text_input("Review this 👩‍💻\nSecond line", &mut h.handlers, &mut h.app)
            .unwrap();
        h.compose();
        h.click("desk-send");
        let message = &h.app.current().messages[h.app.current().messages.len() - 2];
        assert_eq!(message.body, "Review this 👩‍💻\nSecond line");
        let sent = h.app.sent;
        h.app.draft = "   \n".into();
        h.compose();
        h.app.send();
        assert_eq!(h.app.sent, sent);
        let [x, y, w, height] = h.bounds("desk-send");
        assert!(!h.runtime.pointer_down(x + w / 2., y + height / 2.));
        assert!(
            !h.runtime
                .pointer_up_outcome(x + w / 2., y + height / 2., &mut h.handlers, &mut h.app)
                .unwrap()
                .activated
        );
    }
}
