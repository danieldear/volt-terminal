//! Optional real window alpha and OS backdrop-blur request; not a shadow simulation.
use volt_ui_kit::{
    native::{run_ui_with_options, NativeOptions, WindowBackground, WindowChrome},
    ui::prelude::*,
};
#[derive(Default)]
struct App {
    count: usize,
}
fn view(app: &App) -> Element<App> {
    vstack![
        hstack![
            text("VOLT UI / TRANSPARENT WINDOW")
                .font_size(13.)
                .font_weight(700),
            spacer()
        ]
        .height(48.)
        .padding(80., Edge::Left)
        .drag_region(true),
        vstack![
            text("A window, not a painted blur.")
                .font_size(30.)
                .font_weight(700),
            text("The desktop comes through the root clear. OS support determines backdrop blur."),
            panel![
                text(format!("Clicked {} times", app.count)),
                button("Increment")
                    .key("increment")
                    .primary(true)
                    .on_click(|app: &mut App| app.count += 1)
            ]
            .padding(24., Edge::All)
            .gap(16., Axis::Vertical)
            .rounded(16.)
            .background(ColorToken::Base200)
            .background_opacity(0.75),
        ]
        .padding(28., Edge::All)
        .gap(24., Axis::Vertical),
    ]
    .color(ColorToken::BaseContent)
    .into() // Do not paint an opaque root background.
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    run_ui_with_options(
        NativeOptions::default()
            .with_chrome(WindowChrome::Overlay)
            .with_theme(ThemeMode::System)
            .with_background(WindowBackground::Blur { opacity: 0.65 }),
        App::default(),
        view,
    )
}
