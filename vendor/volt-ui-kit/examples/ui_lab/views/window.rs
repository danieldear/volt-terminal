use super::{description, specimen};
use crate::lab::App;
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    vstack![
        description("These are real host requests, not CSS-style blur painted over a panel. Window settings are chosen at startup."),
        specimen("Current launch",vec![
            text(format!("Chrome: {:?}",app.chrome)).into(),
            text(format!("Requested background: {:?}",app.background)).into(),
            description(if cfg!(feature="native-gpu") {"Alpha-capable presenter compiled in; runtime GPU/OS support still determines success."} else {"Software presenter: opaque window. Transparency requests log an explicit opaque fallback."}),
            description("The header is a drag region; its controls remain clickable. Overlay keeps native traffic lights on macOS."),
        ]),
        specimen("Try this same app",vec![
            text("cargo run --features native-gpu --example ui_lab -- --section window --background blur --window-opacity 0.65 --chrome overlay")
                .font_family("monospace").font_size(13.).into(),
            description("Use --background transparent for alpha without blur; opaque is the default. Native blur varies by OS/compositor. There is no portable blur-radius API."),
            description("Use --chrome native | overlay | borderless. Borderless removes the system title bar; close using your OS window/app controls."),
        ]),
        panel![text("Translucent panel, opaque controls").font_weight(600),
            button("Test foreground").key("window-action").primary(true).color(ColorToken::PrimaryContent)
                .on_click(|app:&mut App|app.record("Window foreground action fired")),
            description("A translucent panel is not proof of desktop-through alpha or native blur.")]
            .padding(24.,Edge::All).gap(16.,Axis::Vertical).rounded(12.)
            .background(ColorToken::Base200).background_opacity(0.6),
        specimen("Acceptance checks",vec![
            description("Move the window over contrasting desktop content. Confirm alpha, blur, traffic lights, dragging and DPI changes on your target machine."),
            description("Switch OS light/dark appearance while System is selected. Confirm controls and panels update on supported platforms."),
            description("VOLT_UI_TRACE=1 prints native event/frame diagnostics. --bench measures CPU stages only, not input-to-photon latency."),
            description("Snapshots use a synthetic RGBA clear and cannot validate OS compositing, native appearance events, or trackpad feel."),
        ]),
    ].gap(18.,Axis::Vertical).into()
}
