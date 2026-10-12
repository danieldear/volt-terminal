use super::App;
use volt_ui_kit::ui::{prelude::*, Badge, Button, Container, Stack, Text};

pub fn label(value: impl Into<String>) -> Text<App> {
    text(value)
        .font_family("SF Mono")
        .font_size(10.)
        .font_weight(500)
        .color(ColorToken::Muted)
}
pub fn muted(value: impl Into<String>) -> Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}
pub fn title(value: impl Into<String>) -> Text<App> {
    text(value).font_size(19.).font_weight(600)
}
pub fn pill(value: impl Into<String>, bg: ColorToken, fg: ColorToken) -> Badge<App> {
    badge(value)
        .font_size(10.)
        .font_weight(600)
        .background(bg)
        .color(fg)
        .rounded(Rounding::Full)
}
pub fn card(children: impl IntoIterator<Item = Element<App>>) -> Container<App> {
    panel(children)
        .padding(22., Edge::All)
        .gap(16., Axis::Vertical)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(20.)
}
/// Tint the surface, never the children: type and focus rings remain fully opaque.
pub fn glass_card(app: &App, children: impl IntoIterator<Item = Element<App>>) -> Container<App> {
    card(children).background_opacity(
        if app.background == volt_ui_kit::native::WindowBackground::Opaque {
            1.
        } else {
            0.88
        },
    )
}
pub fn action(
    label: impl Into<String>,
    key: impl Into<String>,
    active: bool,
    callback: impl FnMut(&mut App) + 'static,
) -> Button<App> {
    button(label)
        .key(key)
        .height(40.)
        .font_size(12.)
        .font_weight(600)
        .background(if active {
            ColorToken::Primary
        } else {
            ColorToken::Base300
        })
        .color(if active {
            ColorToken::PrimaryContent
        } else {
            ColorToken::BaseContent
        })
        .border_width(0.)
        .rounded(10.)
        .hover_style(|s| s.opacity(0.82))
        .pressed_style(|s| s.opacity(0.65))
        .focus_style(|s| s.glow(Glow::new().color(ColorToken::Primary).blur(4.).spread(2.)))
        .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(100))
        .on_click(callback)
}
pub fn route(screen: super::Screen, key: &str, caption: &str) -> Button<App> {
    action(caption, key, true, move |app| {
        app.showcase.screen = screen;
        app.record(format!("Atelier: {}", screen.title()));
    })
}
/// Hand-drawn vector mark; not an SF Symbol or a copied daisyUI asset.
pub fn mark(size: f32) -> Element<App> {
    canvas(move |c| {
        let s = size / 32.;
        let path = VectorPath::new()
            .move_to(5., 7.)
            .line_to(14., 7.)
            .line_to(14., 18.)
            .line_to(27., 6.)
            .line_to(27., 15.)
            .line_to(14., 27.)
            .line_to(5., 18.)
            .close();
        c.fill(
            path.transformed([s, 0., 0., s, 0., 0.]),
            ColorToken::Primary,
        );
    })
    .width(size)
    .height(size)
}
pub fn avatar(initials: &str, bg: &str, size: f32) -> Element<App> {
    panel![text(initials)
        .font_size(size * 0.32)
        .font_weight(700)
        .color("#252435")]
    .width(size)
    .height(size)
    .flow(Flow::Center)
    .rounded(Rounding::Full)
    .background(bg)
    .into()
}
pub fn adaptive(children: impl IntoIterator<Item = Element<App>>) -> Stack<App> {
    stack(children)
        .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
        .gap(20., Axis::Both)
}
