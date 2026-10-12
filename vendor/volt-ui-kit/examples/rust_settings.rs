//! Rust-first settings: reusable UI helpers, ordinary conditionals and callbacks.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::{column, prelude::*};

struct Settings {
    notifications: bool,
    keyboard_hints: bool,
    approval_required: bool,
    show_details: bool,
    updates: usize,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            notifications: true,
            keyboard_hints: true,
            approval_required: false,
            show_details: false,
            updates: 0,
        }
    }
}
/// Reuse ordinary Rust functions as components; no registration or UI DSL class.
fn section(title: &str, children: Vec<Element<Settings>>) -> Element<Settings> {
    panel![
        text(title).font_size(18.).font_weight(600),
        column(children).gap(10., Axis::Both),
    ]
    .padding(20., Edge::All)
    .gap(14., Axis::Both)
    .background("#1C2533")
    .border_color("#334154")
    .border_width(1.)
    .rounded(10.)
    .into()
}
fn view(app: &Settings) -> Element<Settings> {
    let detail: Element<Settings> = if app.show_details {
        panel![
            text("How updates work").font_weight(600),
            text("An event changes Rust state. The host calls this view again and installs the new tree with fresh callbacks.")
                .color("#AAB8CA"),
            text("No dependency-tracked signals or background polling are involved.")
                .italic(true).color("#AAB8CA"),
        ].key("explanation").font_family("serif").padding(16., Edge::All).gap(10., Axis::Both).background("#19212D")
            .border_color("#334154").border_width(1.).rounded(8.).into()
    } else {
        text("Enable implementation details to insert another panel.")
            .key("explanation_hint")
            .font_size(12.)
            .color("#97A7BE")
            .into()
    };
    column![
        row![
            column![
                text("Workspace settings").font_size(25.).font_weight(700),
                text("Reusable Rust components, inline closures and conditional content.")
                    .color("#AAB8CA"),
            ]
            .gap(5., Axis::Both)
            .flex(1.),
            badge(format!("{} updates", app.updates))
                .background("#263249")
                .color("#C5D2FC")
                .rounded(6.)
                .font_size(12.),
        ]
        .gap(18., Axis::Both),
        row![
            text("Sans 600")
                .font_family("system")
                .font_weight(600)
                .color("#C5D2FC"),
            text("Serif italic")
                .font_family("serif")
                .italic(true)
                .color("#85C8B3"),
            text("Monospace").font_family("monospace").color("#E4BE87"),
        ]
        .gap(24., Axis::Both),
        separator().background("#334154"),
        section(
            "Preferences",
            vec![
                toggle("Enable notifications", app.notifications)
                    .key("notifications")
                    .accent_color("#A7B8FA")
                    .border_color("#52647E")
                    .on_change(|app: &mut Settings, value| {
                        app.notifications = value;
                        app.updates += 1;
                    })
                    .into(),
                toggle("Show keyboard hints", app.keyboard_hints)
                    .key("keyboard_hints")
                    .accent_color("#A7B8FA")
                    .border_color("#52647E")
                    .on_change(|app: &mut Settings, value| {
                        app.keyboard_hints = value;
                        app.updates += 1;
                    })
                    .into(),
                checkbox("Require approval before applying", app.approval_required)
                    .key("approval")
                    .accent_color("#85C8B3")
                    .border_color("#52647E")
                    .on_change(|app: &mut Settings, value| {
                        app.approval_required = value;
                        app.updates += 1;
                    })
                    .into(),
                checkbox("Show implementation details", app.show_details)
                    .key("show_details")
                    .accent_color("#A7B8FA")
                    .border_color("#52647E")
                    .on_change(|app: &mut Settings, value| {
                        app.show_details = value;
                        app.updates += 1;
                    })
                    .into(),
            ]
        ),
        detail,
        button("Restore defaults")
            .key("restore_defaults")
            .background("#29364A")
            .color("#ECF1F8")
            .border_color("#475872")
            .border_width(1.)
            .accent_color("#A7B8FA")
            .rounded(7.)
            .on_click(|app: &mut Settings| {
                let updates = app.updates + 1;
                *app = Settings::default();
                app.updates = updates;
            }),
        text("This example keeps preferences in memory; it does not write files.")
            .font_size(12.)
            .color("#97A7BE"),
    ]
    .padding(24., Edge::All)
    .gap(18., Axis::Both)
    .font_family("system")
    .font_size(13.)
    .color("#ECF1F8")
    .background("#151B25")
    .into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch("Volt UI Kit — Rust Settings", Settings::default(), view)
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
    fn conditional_panel_follows_inline_rust_callback() {
        let mut app = Settings::default();
        let (document, mut callbacks) = view(&app).build().unwrap().into_parts();
        let bindings = Bindings::default();
        let mut runtime = Runtime::new(document, &callbacks, &bindings).unwrap();
        let viewport = Viewport::new(960., 760., 1.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime.layout(viewport, &bindings, &mut painter).unwrap();
        assert!(!runtime.boxes().iter().any(|b| b.id == "explanation"));
        for _ in 0..4 {
            runtime.focus_next(false);
        }
        assert_eq!(runtime.focused(), Some("show_details"));
        runtime.key_down();
        runtime.key_up(&mut callbacks, &mut app).unwrap();
        assert!(app.show_details);
        assert_eq!(app.updates, 1);
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &callbacks, &bindings)
            .unwrap();
        runtime.layout(viewport, &bindings, &mut painter).unwrap();
        assert!(runtime.boxes().iter().any(|b| b.id == "explanation"));
        assert_eq!(runtime.focused(), Some("show_details"));
    }
}
