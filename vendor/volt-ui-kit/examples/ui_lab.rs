//! The canonical interactive workbench. Add future feature demos to its sections.
#[path = "ui_lab/mod.rs"]
mod lab;
#[path = "support/rust_ui.rs"]
mod support;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, options) = lab::parse_args(std::env::args().skip(1))?;
    support::launch_themed_with_messages(
        "Volt UI Lab",
        app,
        lab::view,
        lab::theme,
        options,
        |app, sender| app.sender = Some(sender),
        lab::jobs::update,
    )
}
