//! An original desktop-workbench example, not a Codex client or AI integration.
#[path = "desk/mod.rs"]
mod desk;
#[path = "support/rust_ui.rs"]
mod support;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, options) = desk::parse_args(
        ["--size".to_owned(), "1360x900".to_owned()]
            .into_iter()
            .chain(std::env::args().skip(1)),
    )?;
    support::launch_themed_with_args("Volt Desk", app, desk::view, desk::theme, options)
}
