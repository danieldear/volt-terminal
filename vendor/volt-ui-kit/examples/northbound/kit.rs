//! Northbound's visual vocabulary: SF throughout, with hierarchy carried by
//! size and weight; everything else is semantic color.
use super::{
    icons::{icon, Icon},
    App,
};
use volt_ui_kit::ui::{prelude::*, Badge, Button, Container, Text};

/// The host resolves "SF" to SF Pro Text below 24 px and SF Pro Display at
/// 24 px and above (installed Apple faces), falling back to sans-serif.
pub const SANS: &str = "SF";

pub fn display(value: impl Into<String>, size: f32) -> Text<App> {
    text(value).font_size(size).font_weight(700)
}
pub fn heading(value: impl Into<String>) -> Text<App> {
    text(value).font_size(17.).font_weight(600)
}
pub fn body(value: impl Into<String>) -> Text<App> {
    text(value).font_size(14.).color(ColorToken::BaseContent)
}
pub fn muted(value: impl Into<String>) -> Text<App> {
    text(value).font_size(13.).color(ColorToken::Muted)
}
pub fn small(value: impl Into<String>) -> Text<App> {
    text(value)
        .font_size(12.)
        .font_weight(500)
        .color(ColorToken::Muted)
}

/// Surfaces carry hierarchy through radius: pictures 18, cards 16, controls 10.
pub fn card(children: impl IntoIterator<Item = Element<App>>) -> Container<App> {
    vstack(children)
        .padding(20., Edge::All)
        .gap(14., Axis::Vertical)
        .background(ColorToken::Base200)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .rounded(16.)
}

pub fn chip(value: impl Into<String>, bg: ColorToken, fg: ColorToken) -> Badge<App> {
    badge(value)
        .font_size(12.)
        .font_weight(600)
        .background(bg)
        .color(fg)
        .border_width(0.)
        .rounded(Rounding::Full)
}
/// A frosted chip for text over photographs.
pub fn photo_chip(value: impl Into<String>) -> Badge<App> {
    badge(value)
        .font_size(12.)
        .font_weight(600)
        .background("#0B141C99")
        .color("#F3F7F6")
        .border_color("#FFFFFF2E")
        .border_width(1.)
        .rounded(Rounding::Full)
}

fn interactive(b: Button<App>) -> Button<App> {
    b.pressed_style(|s| s.opacity(0.7))
        .focus_style(|s| s.glow(Glow::new().color(ColorToken::Primary).blur(6.).spread(2.)))
        .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(140))
        .animate_on(Interaction::Pressed, Animation::ease_out().duration_ms(80))
}
pub fn primary(
    label: impl Into<String>,
    key: impl Into<String>,
    f: impl FnMut(&mut App) + 'static,
) -> Button<App> {
    interactive(
        button(label)
            .key(key)
            .height(40.)
            .font_size(14.)
            .font_weight(600)
            .background(ColorToken::Primary)
            .color(ColorToken::PrimaryContent)
            .border_width(0.)
            .rounded(10.)
            .on_click(f),
    )
    .hover_style(|s| s.glow(Glow::new().color(ColorToken::Primary).blur(14.)))
}
pub fn quiet(
    label: impl Into<String>,
    key: impl Into<String>,
    f: impl FnMut(&mut App) + 'static,
) -> Button<App> {
    interactive(
        button(label)
            .key(key)
            .height(40.)
            .font_size(14.)
            .font_weight(500)
            .background(ColorToken::Base300)
            .color(ColorToken::BaseContent)
            .border_width(0.)
            .rounded(10.)
            .on_click(f),
    )
    .hover_style(|s| s.background(ColorToken::Border))
}
pub fn danger(
    label: impl Into<String>,
    key: impl Into<String>,
    f: impl FnMut(&mut App) + 'static,
) -> Button<App> {
    interactive(
        button(label)
            .key(key)
            .height(40.)
            .font_size(14.)
            .font_weight(600)
            .background(ColorToken::Error)
            .color(ColorToken::ErrorContent)
            .border_width(0.)
            .rounded(10.)
            .on_click(f),
    )
    .hover_style(|s| s.opacity(0.85))
}

/// A square icon button: an invisible hit target with a canvas glyph layered on top.
pub fn icon_button(
    glyph: Icon,
    key: impl Into<String>,
    size: f32,
    active: bool,
    f: impl FnMut(&mut App) + 'static,
) -> Element<App> {
    let (bg, fg) = if active {
        (ColorToken::Primary, ColorToken::PrimaryContent)
    } else {
        (ColorToken::Base300, ColorToken::BaseContent)
    };
    let key = key.into();
    zstack![
        interactive(
            button("")
                .key(key.clone())
                .width(size)
                .height(size)
                .background(if active { bg } else { ColorToken::Base200 })
                .border_color(ColorToken::Border)
                .border_width(if active { 0. } else { 1. })
                .rounded(12.)
                .on_click(f)
        )
        .hover_style(|s| if active {
            s.opacity(0.9)
        } else {
            s.background(ColorToken::Base300)
        }),
        icon(glyph, size * 0.46, fg),
    ]
    .key(format!("{key}-layer"))
    .width(size)
    .height(size)
    .into()
}

/// Horizontal progress drawn as two layered bars.
pub fn meter(fraction: f32, width: f32, color: ColorToken) -> Element<App> {
    let fraction = fraction.clamp(0., 1.);
    zstack![
        panel![]
            .width(width)
            .height(6.)
            .padding(0., Edge::All)
            .background(ColorToken::Base300)
            .border_width(0.)
            .rounded(Rounding::Full),
        panel![]
            .width((width * fraction).max(6.))
            .height(6.)
            .padding(0., Edge::All)
            .background(color)
            .border_width(0.)
            .rounded(Rounding::Full),
    ]
    .flow(Flow::Left)
    .width(width)
    .height(6.)
    .into()
}

pub fn stat(value: impl Into<String>, label: impl Into<String>) -> Element<App> {
    vstack![text(value).font_size(22.).font_weight(600), small(label),]
        .gap(2., Axis::Vertical)
        .into()
}

pub fn section_header(title: impl Into<String>, trailing: Element<App>) -> Element<App> {
    hstack![heading(title).font_size(19.), spacer(), trailing]
        .align(Align::Center)
        .gap(12., Axis::Horizontal)
        .into()
}

pub fn row_with_icon(glyph: Icon, color: ColorToken, content: Element<App>) -> Element<App> {
    hstack![
        panel![icon(glyph, 18., color)]
            .width(36.)
            .height(36.)
            .padding(0., Edge::All)
            .flow(Flow::Center)
            .background(ColorToken::Base300)
            .border_width(0.)
            .rounded(10.),
        content,
    ]
    .gap(12., Axis::Horizontal)
    .align(Align::Center)
    .into()
}
