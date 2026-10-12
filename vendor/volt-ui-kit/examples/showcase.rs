//! Direct launch of Volt Atelier. Polished screens and their tests live in showcase/.
//! Legacy UI Lab keeps its existing workbench; both launchers share application state.
#[path = "ui_lab/mod.rs"]
mod lab;
#[path = "support/rust_ui.rs"]
mod support;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let defaults = [
        "--section".to_owned(),
        "showcase".to_owned(),
        "--size".to_owned(),
        "1280x900".to_owned(),
    ]
    .into_iter();
    // Only this GPU-enabled demo defaults to glass; toolkit/Volt defaults stay opaque.
    let surface = if cfg!(feature = "native-gpu") {
        vec![
            "--background".into(),
            "blur".into(),
            "--window-opacity".into(),
            "0.82".into(),
        ]
    } else {
        Vec::<String>::new()
    };
    let args = defaults.chain(surface).chain(std::env::args().skip(1));
    let (app, options) = lab::parse_args(args)?;
    support::launch_themed_with_messages(
        "Volt Atelier",
        app,
        lab::view,
        lab::theme,
        options,
        |app, sender| app.sender = Some(sender),
        lab::jobs::update,
    )
}
