mod appearance;
mod controls;
mod data;
mod layout;
mod runtime;
mod scrolling;
mod surfaces;
mod window;
use super::{state::ACCENTS, App, Section};
use volt_ui_kit::{
    native::{WindowBackground, WindowChrome},
    ui::prelude::*,
};

pub fn description(value: impl Into<String>) -> Element<App> {
    text(value).font_size(13.).color(ColorToken::Muted).into()
}
pub fn heading(value: impl Into<String>) -> Element<App> {
    text(value).font_size(19.).font_weight(600).into()
}
pub fn specimen(title: &str, items: impl IntoIterator<Item = Element<App>>) -> Element<App> {
    let mut children = vec![heading(title)];
    children.extend(items);
    panel(children)
        .padding(18., Edge::All)
        .gap(14., Axis::Vertical)
        .background(ColorToken::Base200)
        .rounded(8.)
        .into()
}
pub fn choice(
    label: impl Into<String>,
    key: impl Into<String>,
    active: bool,
    action: impl FnMut(&mut App) + 'static,
) -> Element<App> {
    button(label)
        .key(key)
        .height(38.)
        .primary(active)
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
        .on_click(action)
        .into()
}
pub fn view(app: &App) -> Element<App> {
    if app.section == Section::Showcase {
        return super::showcase::view(app);
    }
    let nav = Section::ALL
        .into_iter()
        .map(|section| {
            choice(
                section.nav_label(),
                format!("nav-{}", section.slug()),
                section == app.section,
                move |app| app.navigate(section),
            )
            .width(match section {
                Section::Typography => 72.,
                Section::Layout => 96.,
                _ => 104.,
            })
        })
        .collect::<Vec<_>>();
    let modes = [
        ("Light", ThemeMode::Light),
        ("Dark", ThemeMode::Dark),
        ("System", ThemeMode::System),
    ]
    .into_iter()
    .map(|(name, mode)| {
        choice(
            name,
            format!("theme-{}", name.to_lowercase()),
            mode == app.mode,
            move |app| {
                app.mode = mode;
                app.record(format!("Theme: {name}"));
            },
        )
        .width(76.)
    })
    .collect::<Vec<_>>();
    let demo = match app.section {
        Section::Controls => controls::view(app),
        Section::Layout => layout::view(app),
        Section::Typography => appearance::typography(app),
        Section::Effects => appearance::effects(app),
        Section::Surfaces => surfaces::view(app),
        Section::Scrolling => scrolling::view(app),
        Section::Window => window::view(app),
        Section::Runtime => runtime::view(app),
        Section::Data => data::view(app),
        Section::Showcase => unreachable!("showcase owns its shell"),
    };
    let root=vstack![
        hstack![text("Volt UI Lab").font_size(22.).font_weight(700),spacer(),
            text(format!("{} actions",app.actions)).font_size(11.).color(ColorToken::Muted)]
            .key("lab-titlebar").height(48.).padding(12.,Edge::Right)
            .padding(if app.chrome==WindowChrome::Overlay {80.} else {16.},Edge::Left)
            .align(Align::Center).drag_region(true).background(ColorToken::Base200).background_opacity(0.9),
        stack![hstack(modes).gap(8.,Axis::Horizontal),
            button(format!("Accent: {}",ACCENTS[app.accent].0)).key("accent-next").height(38.).width(156.)
                .on_click(|app:&mut App|{app.accent=(app.accent+1)%ACCENTS.len();app.record(format!("Accent: {}",ACCENTS[app.accent].0));}),
        ].direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal))
            .padding(12.,Edge::Horizontal).gap(8.,Axis::Both),
        hstack(nav).key("lab-navigation").height(44.).gap(8.,Axis::Horizontal)
            .padding(12.,Edge::Horizontal).scroll(Scroll::Horizontal),
        vstack![text(app.section.title()).font_size(28.).font_weight(700),demo,
            description("Tab / Shift-Tab: focus. Enter / Space: activate. Wheel: scroll; Shift-wheel: horizontal.")]
            .key(format!("content-{}",app.section.slug())).flex(1.).padding(responsive(12.).at(Breakpoint::Md,24.),Edge::All)
            .gap(18.,Axis::Vertical).scroll(Scroll::Auto),
        vstack![hstack![button("Previous").key("section-previous").width(94.).height(34.)
                .on_click(|app:&mut App|app.navigate(app.section.next(true))),
            button("Next").key("section-next").width(94.).height(34.)
                .on_click(|app:&mut App|app.navigate(app.section.next(false))),spacer(),
            button("Reset").key("reset-demos").width(72.).height(34.).on_click(App::reset_demos)]
            .gap(4.,Axis::Horizontal).align(Align::Center),
            text(app.last_action()).key("last-action").font_size(11.).color(ColorToken::Muted)]
            .padding(12.,Edge::Horizontal).padding(8.,Edge::Vertical).gap(6.,Axis::Vertical)
            .background(ColorToken::Base200).background_opacity(0.9),
    ].gap(8.,Axis::Vertical).font_family(app.font.clone()).font_size(14.).color(ColorToken::BaseContent);
    let root: Element<App> = if app.background == WindowBackground::Opaque {
        root.background(ColorToken::Base100).into()
    } else {
        root.into()
    }; // alpha/blur must not be hidden by an opaque root fill
    zstack![root, controls::overlays(app)]
        .font_family(app.font.clone())
        .color(ColorToken::BaseContent)
        .into()
}
