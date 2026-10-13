//! Volt UI catalog: every component, documented with live previews, code,
//! variations and API tables, in a shadcn/ui-style documentation shell.
//!
//! cargo run --release --features native --example catalog
//! cargo run --release --features native --example catalog -- --page table --theme dark --accent violet
//! cargo run --release --features native-gpu --example catalog   # frosted sidebar
//! cargo run --release --features native-gpu --example catalog -- --background opaque
#[path = "catalog/mod.rs"]
mod catalog;
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::{native::WindowChrome, ThemeMode};

fn parse_args(
    args: impl IntoIterator<Item = String>,
) -> Result<(catalog::App, support::RustLaunchOptions), Box<dyn std::error::Error>> {
    let mut app = catalog::App::new();
    let mut forwarded = vec!["--size".to_owned(), "1440x920".to_owned()];
    let mut chrome = if cfg!(target_os = "macos") {
        WindowChrome::Overlay
    } else {
        WindowChrome::Native
    };
    // Frosted sidebar by default where the window can be translucent (GPU);
    // the software renderer always presents an opaque window.
    let mut background = if cfg!(feature = "native-gpu") {
        "blur"
    } else {
        "opaque"
    };
    let mut opacity = 0.72f32;
    let mut tint = 0u8;
    let mut show_code = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--page" => {
                let slug = args.next().ok_or("missing --page slug")?;
                app.page = catalog::PAGES
                    .into_iter()
                    .find(|p| p.slug() == slug)
                    .ok_or_else(|| {
                        let all: Vec<String> = catalog::PAGES.iter().map(|p| p.slug()).collect();
                        format!("unknown page {slug}; one of: {}", all.join(", "))
                    })?;
            }
            "--accent" => {
                let name = args.next().ok_or("missing --accent name")?;
                app.theme = catalog::THEMES
                    .iter()
                    .position(|(n, ..)| n.eq_ignore_ascii_case(&name))
                    .ok_or("--accent must be zinc, blue, violet, rose, green or orange")?;
            }
            "--theme" => {
                let value = args.next().ok_or("missing --theme value")?;
                app.mode = match value.as_str() {
                    "dark" => ThemeMode::Dark,
                    "system" => ThemeMode::System,
                    "light" => ThemeMode::Light,
                    _ => return Err("--theme must be light, dark or system".into()),
                };
                forwarded.extend(["--theme".to_owned(), value]);
            }
            "--code" => {
                // Show the first section's code (handy for screenshots).
                show_code = true;
            }
            "--palette" => app.palette.open = true,
            "--dialog" => app.dialog_open = true,
            "--native-chrome" => chrome = WindowChrome::Native,
            "--compact-chrome" => chrome = WindowChrome::CompactOverlay,
            "--background" => {
                background =
                    match args.next().ok_or("missing --background mode")?.as_str() {
                        "opaque" => "opaque",
                        "transparent" => "transparent",
                        "blur" | "material" => "blur",
                        "compositor" => "compositor",
                        _ => return Err(
                            "--background must be opaque, transparent, blur/material or compositor"
                                .into(),
                        ),
                    };
            }
            "--tint" => {
                tint = args.next().ok_or("missing --tint value")?.parse()?;
                if tint > 100 {
                    return Err("--tint must be between 0 and 100".into());
                }
            }
            "--window-opacity" => {
                opacity = args
                    .next()
                    .ok_or("missing --window-opacity value")?
                    .parse()?;
                if !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
                    return Err("--window-opacity must be finite and between 0 and 1".into());
                }
            }
            _ => forwarded.push(arg),
        }
    }
    if show_code {
        if let Some(id) = catalog::first_section_id(&app) {
            app.code.insert(id.to_owned());
        }
    }
    app.glass = background != "opaque";
    app.opacity = ((opacity * 100.).round() as u8).max(catalog::OPACITY_MIN);
    app.tint = tint;
    app.blur = matches!(background, "blur" | "compositor");
    app.compositor_blur = background == "compositor";
    let background = catalog::window_background(&app);
    Ok((
        app,
        support::RustLaunchOptions {
            chrome,
            background,
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
    let renderer = match std::env::var("VOLT_UI_RENDERER").ok().as_deref() {
        None | Some("auto") => volt_ui_kit::native::RendererPreference::Auto,
        Some("gpu") => volt_ui_kit::native::RendererPreference::GpuRequired,
        Some("software") => volt_ui_kit::native::RendererPreference::Software,
        Some(_) => return Err("VOLT_UI_RENDERER must be auto, gpu or software".into()),
    };
    support::launch_themed_with_application(
        "Volt UI",
        app,
        catalog::view,
        catalog::theme,
        options,
        move |application| {
            let application = application
                .background_from_state(catalog::window_background)
                .renderer(renderer)
                .commands(catalog::lifecycle::commands())
                .requests(|app, sender| app.requests = Some(sender))
                .on_close_request(catalog::lifecycle::close_request)
                .messages(|app, sender| app.sender = Some(sender), catalog::update)
                // Under `cargo volt dev`, code changes restart on the same page.
                .dev_state(catalog::dev_save, catalog::dev_load);
            #[cfg(all(feature = "native-desktop", target_os = "macos"))]
            let application = application.desktop(catalog::lifecycle::desktop(), |_, _| {});
            application
        },
    )
}
