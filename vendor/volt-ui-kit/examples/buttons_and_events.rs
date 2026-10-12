//! Typed Rust callbacks, pointer capture and keyboard activation.
mod support;
use volt_ui_kit::declarative::Handlers;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fn reset(count: &mut usize) {
        *count = 0;
    }
    let mut handlers = Handlers::default();
    handlers.on_activate("increment", |count: &mut usize| *count += 1)?;
    handlers.on_activate("reset", reset)?;
    support::launch(
        "Volt UI Kit — Buttons & Events",
        include_str!("ui/buttons_and_events.json"),
        Some("examples/ui/buttons_and_events.json"),
        0,
        handlers,
        |count, bindings| {
            bindings.set_text("status", format!("Activations: {count}"));
            bindings.set_bool("can_reset", *count > 0);
        },
    )
}
