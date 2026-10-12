//! Edit the JSON file and press F5: validate first, retain last valid tree on error.
mod support;
use volt_ui_kit::declarative::Handlers;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut handlers = Handlers::default();
    handlers.on_activate("increment", |count: &mut usize| *count += 1)?;
    handlers.on_activate("reset", |count: &mut usize| *count = 0)?;
    support::launch(
        "Volt UI Kit — Editable Documents",
        include_str!("ui/declarative_ui.json"),
        Some("examples/ui/declarative_ui.json"),
        0,
        handlers,
        |count, bindings| {
            bindings.set_text("status", format!("Rust state: {count}"));
            bindings.set_bool("can_reset", *count > 0);
        },
    )
}
