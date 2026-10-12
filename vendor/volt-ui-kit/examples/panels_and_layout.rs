//! Nested panel/row/column geometry through the shared document runtime.
mod support;
use volt_ui_kit::declarative::Handlers;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut handlers = Handlers::default();
    handlers.on_activate("increment", |count: &mut usize| {
        *count += 1;
        println!("Layout example callback #{count}");
    })?;
    support::launch(
        "Volt UI Kit — Panels & Layout",
        include_str!("ui/panels_and_layout.json"),
        Some("examples/ui/panels_and_layout.json"),
        0,
        handlers,
        |_, _| {},
    )
}
