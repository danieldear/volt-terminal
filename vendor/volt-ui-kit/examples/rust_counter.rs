//! Minimal Rust-first declarative UI: one pure view and inline typed callbacks.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::{column, prelude::*};

#[derive(Default)]
struct Counter {
    count: usize,
}
fn view(app: &Counter) -> Element<Counter> {
    column![
        text("Rust counter").font_size(24.).font_weight(700),
        text("The UI is a function of your Rust state.").color("#AAB8CA"),
        text(format!("Count: {}", app.count))
            .font_family("monospace")
            .font_size(36.)
            .font_weight(600),
        row![
            button("Add one")
                .key("increment")
                .primary(true)
                .on_click(|app: &mut Counter| app.count += 1),
            button("Reset")
                .key("reset")
                .enabled(app.count > 0)
                .on_click(|app: &mut Counter| app.count = 0),
        ]
        .gap(12., Axis::Horizontal),
        text("Tab and Enter work too. Rust source changes require a rebuild.")
            .font_size(12.)
            .color("#AAB8CA"),
    ]
    .padding(28., Edge::Horizontal)
    .padding(20., Edge::Vertical)
    .padding(32., Edge::Top)
    .gap(20., Axis::Vertical)
    .font_family("system")
    .color("#ECF1F8")
    .background("#151B25")
    .into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch("Volt UI Kit — Rust Counter", Counter::default(), view)
}
#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Viewport,
    };
    #[test]
    fn inline_callback_updates_context_and_view_recomposes() {
        let mut app = Counter::default();
        let (document, mut callbacks) = view(&app).build().unwrap().into_parts();
        let bindings = Bindings::default();
        let mut runtime = Runtime::new(document, &callbacks, &bindings).unwrap();
        let viewport = Viewport::new(960., 760., 1.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime.layout(viewport, &bindings, &mut painter).unwrap();
        assert!(
            !runtime
                .boxes()
                .iter()
                .find(|b| b.id == "reset")
                .unwrap()
                .enabled
        );
        runtime.focus_next(false);
        assert_eq!(runtime.focused(), Some("increment"));
        runtime.key_down();
        runtime.key_up(&mut callbacks, &mut app).unwrap();
        assert_eq!(app.count, 1);
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &callbacks, &bindings)
            .unwrap();
        runtime.layout(viewport, &bindings, &mut painter).unwrap();
        assert!(
            runtime
                .boxes()
                .iter()
                .find(|b| b.id == "reset")
                .unwrap()
                .enabled
        );
        assert_eq!(runtime.focused(), Some("increment"));
    }
}
