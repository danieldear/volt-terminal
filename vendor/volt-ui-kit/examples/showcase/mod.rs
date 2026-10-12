mod activity;
mod explorer;
mod state;
mod studio;
mod style;
#[cfg(test)]
mod tests;
mod welcome;
mod workspace;
use crate::lab::App;
pub use state::{theme, Screen, Showcase};
use style::*;
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ui::prelude::*,
};

/// Shared by both the Lab route and the direct `showcase` example.
pub fn view(app: &App) -> Element<App> {
    let navigation = tab_bar(Screen::ALL.into_iter().map(|screen| {
        tab_item(screen, screen.title()).key(format!("showcase-nav-{}", screen.slug()))
    }))
    .key("atelier-tabs")
    .selected(app.showcase.screen)
    .on_select(|app: &mut App, screen: Screen| {
        app.showcase.screen = screen;
        app.record(format!("Atelier: {}", screen.title()));
    })
    .height(44.)
    .font_size(13.)
    .font_weight(500)
    .gap(5., Axis::Horizontal)
    .scroll(Scroll::Horizontal);
    let page = match app.showcase.screen {
        Screen::Welcome => welcome::view(app),
        Screen::Workspace => workspace::view(app),
        Screen::Studio => studio::view(app),
        Screen::Activity => activity::view(app),
        Screen::Explorer => explorer::view(app),
    };
    let theme_modes = [
        ("Midnight", ThemeMode::Dark),
        ("Paper", ThemeMode::Light),
        ("System", ThemeMode::System),
    ]
    .into_iter()
    .map(|(name, mode)| {
        action(
            name,
            format!("atelier-theme-{name}"),
            app.mode == mode,
            move |app| {
                app.mode = mode;
                app.record(format!("Atelier appearance: {name}"));
            },
        )
        .width(90.)
        .into()
    })
    .collect::<Vec<Element<App>>>();
    let root = vstack![
        toolbar![
            mark(28.),
            text("volt").font_size(21.).font_weight(700),
            label("/ ATELIER"),
            spacer(),
            button("UI Lab")
                .key("section-next")
                .width(80.)
                .height(34.)
                .background(ColorToken::Base300)
                .color(ColorToken::BaseContent)
                .border_width(0.)
                .rounded(8.)
                .on_click(|app: &mut App| app.navigate(app.section.next(false)))
        ]
        .key("atelier-titlebar")
        .height(56.)
        .padding(
            if app.chrome == WindowChrome::Overlay {
                80.
            } else {
                20.
            },
            Edge::Left
        )
        .padding(20., Edge::Right)
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .drag_region(true)
        .background(ColorToken::Base200)
        .background_opacity(if app.background == WindowBackground::Opaque {
            1.
        } else {
            0.72
        }),
        stack![
            navigation,
            button_bar(theme_modes)
                .height(44.)
                .gap(5., Axis::Horizontal)
                .scroll(Scroll::Horizontal)
        ]
        .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
        .gap(12., Axis::Both)
        .padding(responsive(16.).at(Breakpoint::Lg, 36.), Edge::Horizontal),
        vstack![page]
            .key(format!("content-showcase-{}", app.showcase.screen.slug()))
            .flex(1.)
            .padding(responsive(16.).at(Breakpoint::Lg, 36.), Edge::All)
            .scroll(Scroll::Auto),
        hstack![
            label(app.last_action()).flex(1.),
            label(match app.background {
                WindowBackground::Opaque => "OPAQUE / RUST UI",
                WindowBackground::Transparent { .. } => "TRANSLUCENT / RUST UI",
                WindowBackground::Blur { .. } | WindowBackground::CompositorBlur { .. } =>
                    "BLUR REQUEST / RUST UI",
            })
        ]
        .height(30.)
        .padding(16., Edge::Horizontal)
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .background(ColorToken::Base200)
        .background_opacity(if app.background == WindowBackground::Opaque {
            1.
        } else {
            0.72
        }),
    ]
    .font_family(if app.font == "system" {
        "SF"
    } else {
        &app.font
    })
    .font_size(14.)
    .color(ColorToken::BaseContent);
    if app.background == WindowBackground::Opaque {
        root.background(ColorToken::Base100).into()
    } else {
        root.into()
    }
}
