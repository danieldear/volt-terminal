//! Northbound: an app-scale showcase of every Rust UI component, themed around
//! a two-week trip along Norway's Arctic coast. Photographs are generated in
//! Rust at startup; no assets are bundled or downloaded.
//!
//! cargo run --features native --example northbound
//! cargo run --features native --example northbound -- --screen days --theme light
#[path = "northbound/mod.rs"]
mod northbound;
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::native::{WindowBackground, WindowChrome};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = northbound::App::new();
    let mut forwarded = vec!["--size".to_owned(), "1320x900".to_owned()];
    let mut args = std::env::args().skip(1);
    let mut chrome = if cfg!(target_os = "macos") {
        WindowChrome::Overlay
    } else {
        WindowChrome::Native
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screen" => {
                let value = args
                    .next()
                    .ok_or("missing --screen route|days|journal|settings")?;
                app.screen = northbound::Screen::parse(&value)
                    .ok_or("--screen must be route, days, journal or settings")?;
            }
            "--accent" => {
                let value = args.next().ok_or("missing --accent name")?;
                app.accent = northbound::ACCENTS
                    .iter()
                    .position(|(name, _)| name.eq_ignore_ascii_case(&value))
                    .ok_or("--accent must be aurora, ember, glacier or heather")?;
            }
            "--reserve" => app.reserve_open = true,
            "--native-chrome" => chrome = WindowChrome::Native,
            "--theme" => {
                let value = args.next().ok_or("missing --theme value")?;
                app.mode = match value.as_str() {
                    "light" => volt_ui_kit::ThemeMode::Light,
                    "system" => volt_ui_kit::ThemeMode::System,
                    _ => volt_ui_kit::ThemeMode::Dark,
                };
                forwarded.extend(["--theme".to_owned(), value]);
            }
            _ => forwarded.push(arg),
        }
    }
    support::launch_themed_with_args(
        "Northbound",
        app,
        northbound::view,
        northbound::theme,
        support::RustLaunchOptions {
            chrome,
            background: WindowBackground::Opaque,
            args: forwarded,
        },
    )
}
