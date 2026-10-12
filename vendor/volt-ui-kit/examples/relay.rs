//! Relay: a coding-agent desktop client built with Volt UI — frosted thread
//! sidebar, streaming transcript, composer menus and an inspector card. The
//! agent is a local scripted worker, so everything runs offline.
//!
//! cargo run --release --features native-gpu --example relay
//! cargo run --release --features native --example relay -- --theme light
#[path = "relay/mod.rs"]
mod relay;
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ThemeMode,
};

fn parse_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(relay::App, support::RustLaunchOptions), Box<dyn std::error::Error>> {
    let mut app = relay::App::new();
    let mut forwarded = vec!["--size".to_owned(), "1320x860".to_owned()];
    // Frosted where the window can be translucent (GPU); opaque otherwise.
    let mut glass = cfg!(feature = "native-gpu");
    let mut opacity = 0.72f32;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--theme" => {
                let value = args.next().ok_or("missing --theme value")?;
                app.mode = match value.as_str() {
                    "dark" => ThemeMode::Dark,
                    "light" => ThemeMode::Light,
                    "system" => ThemeMode::System,
                    _ => return Err("--theme must be light, dark or system".into()),
                };
                forwarded.extend(["--theme".to_owned(), value]);
            }
            "--thread" => {
                let id: u32 = args.next().ok_or("missing --thread id")?.parse()?;
                if !app.threads.iter().any(|t| t.id == id) {
                    return Err(format!("no thread {id}").into());
                }
                app.open(id);
            }
            "--new" => app.new_chat(),
            "--settings" => app.settings_open = true,
            "--actions" => app.thread_menu_open = true,
            "--no-inspector" => app.inspector = false,
            "--no-sidebar" => app.sidebar = false,
            "--draft" => app.draft = args.next().ok_or("missing --draft text")?,
            "--send" => {
                let prompt = args.next().ok_or("missing --send prompt")?;
                app.send(Some(prompt));
            }
            "--background" => {
                glass = match args.next().ok_or("missing --background mode")?.as_str() {
                    "blur" => true,
                    "opaque" => false,
                    _ => return Err("--background must be blur or opaque".into()),
                }
            }
            "--window-opacity" => {
                opacity = args
                    .next()
                    .ok_or("missing --window-opacity value")?
                    .parse()?;
                if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                    return Err("--window-opacity must be between 0 and 1".into());
                }
            }
            _ => forwarded.push(arg),
        }
    }
    app.glass = glass;
    // The window clears fully transparent; the view paints the tint so it can
    // change live. `--window-opacity` sets where it starts.
    app.tint = ((opacity * 100.).round() as u8).max(relay::TINT_MIN);
    Ok((
        app,
        support::RustLaunchOptions {
            chrome: if cfg!(target_os = "macos") {
                WindowChrome::Overlay
            } else {
                WindowChrome::Native
            },
            background: if glass {
                WindowBackground::Blur { opacity: 0. }
            } else {
                WindowBackground::Opaque
            },
            args: forwarded,
        },
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, options) = parse_args(std::env::args().skip(1))?;
    if !cfg!(feature = "native-gpu") {
        eprintln!(
            "Volt UI: the frosted sidebar needs a translucent window; run with \
             --features native-gpu (software drawing is always opaque)"
        );
    }
    support::launch_themed_with_messages(
        "Relay",
        app,
        relay::view,
        relay::theme,
        options,
        |app, sender| app.sender = Some(sender),
        relay::update,
    )
}
