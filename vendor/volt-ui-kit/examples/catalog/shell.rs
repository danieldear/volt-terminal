//! Site chrome: header, sidebar, page frame, landing page, command palette, toasts.
use super::{
    art::{icon, mark, Icon},
    pages::{dialogs, doc, Page, PAGES},
    App, THEMES,
};
use volt_ui_kit::ui::{prelude::*, Element as El};

/// Fully transparent pixels let clicks fall through to the window behind on
/// macOS, so the tint never goes below this.
pub const OPACITY_MIN: u8 = 10;

pub fn view(app: &App) -> El<App> {
    let landing = app.page == Page::Introduction;
    let page = if landing {
        super::landing::landing(app)
    } else {
        article(app)
    };
    // Desktop-app layout: the sidebar sits on the (optionally frosted) window
    // material; the content is a solid sheet so text never reads over blur.
    let content = vstack![
        header(app),
        divider().background(ColorToken::Border),
        // Vertical breathing room lives inside the scrolled content: scroll
        // padding would also shrink the clip, cutting content off early.
        {
            // A readable measure on wide windows: the page column stops
            // growing and centres instead of stretching edge to edge.
            let column = vstack![page]
                .key("page-column")
                .width_fraction(1.)
                .max_width(if landing { 1200. } else { 1040. });
            let padded = vstack![
                spacer().height(if landing { 56. } else { 32. }).flex(0.),
                column,
                spacer().height(64.).flex(0.),
            ]
            .align(Align::Center)
            .padding(
                responsive(16.)
                    .at(Breakpoint::Md, 24.)
                    .at(Breakpoint::Xl, 56.),
                Edge::Horizontal,
            );
            // The landing hero runs edge to edge, outside the page padding.
            let mut scrolled: Vec<El<App>> = vec![];
            if landing {
                scrolled.push(super::landing::landing_hero(app));
            }
            scrolled.push(padded.into());
            vstack(scrolled)
                .key(format!("page-{}", app.page.slug()))
                .transition(Transition::fade().duration_ms(160))
                .flex(1.)
                .align(Align::Stretch)
                .scroll(Scroll::Vertical)
        },
    ]
    .key("content")
    .flex(1.)
    .align(Align::Stretch)
    .background(ColorToken::Base100)
    .border_color(if app.glass {
        ColorToken::BaseContent.alpha(9)
    } else {
        ColorToken::Border.into()
    })
    .border_width(1.);
    let content: El<App> = if app.glass {
        // A sheet floating inside the material. Inset on every free edge, it
        // never meets the window's own rounded corners, whatever their radius.
        // No rounded clip: the page is padded clear of the corners, and a
        // full-window clip layer would push page fades past the GPU's layer
        // budget into software frames.
        vstack![
            vstack![].key("sheet-drag").height(8.).drag_region(true),
            content
                .rounded(12.)
                .shadow(Shadow::new().color("#0000002E").blur(18.).offset(0., 2.)),
        ]
        .key("sheet-frame")
        .flex(1.)
        .align(Align::Stretch)
        .padding(8., Edge::Right)
        .padding(8., Edge::Bottom)
        .into()
    } else {
        content.into()
    };
    // A pixel-sized split: dragging the divider resizes the sidebar, which
    // then keeps its width as the window resizes. Below Lg the sidebar is
    // hidden and the split collapses to the content alone.
    let body = split(sidebar(app), content)
        .key("body-split")
        .first_size(app.sidebar_width)
        .min_first(200.)
        .max_first(420.)
        .min_second(480.)
        // A wide invisible grab area; a pill appears on hover or drag.
        .handle_size(10.)
        .handle_color("#00000000")
        .handle_style(SplitHandle::Reveal)
        .fill_width()
        .fill_height()
        .flex(1.)
        .background(if app.glass {
            String::from("#00000000")
        } else {
            ColorToken::Base200.into()
        })
        .on_resize(|app: &mut App, width| app.sidebar_width = width.round());
    let mut layers: Vec<El<App>> = vec![body.into()];
    layers.extend(dialogs(app));
    layers.push(nav_drawer(app));
    layers.push(palette(app));
    layers.push(toast(app));
    layers.extend(super::lifecycle::overlays(app));
    layers.push(super::documents::overlay(app));
    zstack(layers)
        .key("catalog")
        .align(Align::Stretch)
        .font_family("SF")
        .font_size(14.)
        .color(ColorToken::BaseContent)
        // Background RGB/tint and opacity are owned by the native root clear.
        // Do not paint a second alpha overlay or fade the foreground subtree.
        .background(if app.glass {
            String::from("#00000000")
        } else {
            ColorToken::Base200.into()
        })
        .into()
}

fn mode_button(app: &App) -> El<App> {
    let dark = app.mode == ThemeMode::Dark;
    zstack![
        button("")
            .key("mode-toggle")
            .accessible_label(if dark {
                "Use light appearance"
            } else {
                "Use dark appearance"
            })
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .on_click(move |app: &mut App| {
                app.mode = if dark {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
            }),
        icon(
            if dark { Icon::Sun } else { Icon::Moon },
            18.,
            ColorToken::BaseContent
        ),
    ]
    .width(36.)
    .height(36.)
    .into()
}

fn header(app: &App) -> El<App> {
    let nav = |label: &'static str, target: Page| -> El<App> {
        let active = match target {
            Page::Introduction => app.page == Page::Introduction,
            Page::Theming => app.page == Page::Theming,
            Page::SignIn => app.page.group() == "Patterns",
            _ => app.page.group() == "Components",
        };
        // Letter-spaced mono navigation; the active section is an inverted chip.
        let b = button(label.to_uppercase())
            .key(format!("nav-{}", target.slug()))
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .size(ControlSize::Small)
            .font_family("SF Mono")
            .font_size(11.)
            .letter_spacing(0.16)
            .rounded(2.)
            .font_weight(if active { 600 } else { 500 })
            .on_click(move |app: &mut App| app.go(target));
        if active {
            b.background(ColorToken::BaseContent)
                .color(ColorToken::Base100)
                .hover_style(|s| s.background(ColorToken::BaseContent))
                .into()
        } else {
            b.color(ColorToken::Muted).into()
        }
    };
    let search = zstack![
        button("Search components…")
            .key("open-palette")
            .variant(ButtonVariant::Outline)
            .label_align(Align::Start)
            .color(ColorToken::Muted)
            .width(240.)
            .height(36.)
            .on_click(|app: &mut App| app.palette.open = true),
        hstack![
            spacer(),
            icon(Icon::Search, 15., ColorToken::Muted),
            kbd("⌘K"),
        ]
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .padding(8., Edge::Right)
        .scroll_passthrough(true),
    ]
    .width(240.)
    .height(36.)
    .align(Align::Stretch);
    // Below Md the search box becomes an icon button.
    let search_compact = zstack![
        button("")
            .key("open-palette-compact")
            .accessible_label("Search components")
            .variant(ButtonVariant::Ghost)
            .width(36.)
            .height(36.)
            .on_click(|app: &mut App| app.palette.open = true),
        icon(Icon::Search, 17., ColorToken::BaseContent),
    ]
    .key("search-compact")
    .width(36.)
    .height(36.)
    .flow(Flow::Center)
    .visible(responsive(true).at(Breakpoint::Md, false));
    let search = search.visible(responsive(false).at(Breakpoint::Md, true));
    // Below Lg the sidebar collapses; this opens it as a drawer. On macOS it
    // clears the traffic lights, which then sit over the header.
    let menu_button = hstack![
        spacer()
            .width(if cfg!(target_os = "macos") { 64. } else { 0. })
            .flex(0.),
        zstack![
            button("")
                .key("open-nav")
                .accessible_label("Show pages")
                .variant(ButtonVariant::Ghost)
                .width(36.)
                .height(36.)
                .on_click(|app: &mut App| app.nav_open = true),
            volt_ui_kit::ui::icon(volt_ui_kit::ui::Icon::Menu).size(18.),
        ]
        .width(36.)
        .height(36.)
        .flow(Flow::Center),
    ]
    .key("nav-toggle")
    .align(Align::Center)
    .visible(responsive(true).at(Breakpoint::Lg, false));
    let themes = menu(
        if app.custom_palettes.is_some() {
            "Custom"
        } else {
            THEMES[app.theme].0
        },
        THEMES.iter().enumerate().map(|(i, (name, ..))| {
            menu_item(i, *name).checked(app.custom_palettes.is_none() && i == app.theme)
        }),
    )
    .key("theme-menu")
    .accessible_label("Accent color")
    .open(app.theme_menu_open)
    .width(116.)
    .height(36.)
    .background(ColorToken::Base100)
    .border_color(ColorToken::Border)
    .border_width(1.)
    .on_action(|app: &mut App, i| {
        app.theme = i;
        app.custom_palettes = None;
    })
    .on_open_change(|app: &mut App, open| app.theme_menu_open = open);
    toolbar![
        menu_button,
        hstack![
            nav("Docs", Page::Introduction),
            nav("Components", Page::Button),
            nav("Themes", Page::Theming),
            nav("Patterns", Page::SignIn),
        ]
        .key("header-navigation")
        .accessible_label("Documentation navigation")
        .height(36.)
        .gap(6., Axis::Horizontal)
        .align(Align::Center)
        .scroll(Scroll::Horizontal)
        .scrollbars(Scrollbars::Hidden)
        .flex(1.),
        search,
        search_compact,
        themes,
        mode_button(app),
    ]
    .key("site-header")
    .height(56.)
    .gap(6., Axis::Horizontal)
    .padding(16., Edge::Horizontal)
    .drag_region(true)
    .into()
}

/// The filter field and grouped page links. `prefix` keeps the sidebar's
/// and the drawer's keys distinct ("side" / "drawer").
fn page_links(app: &App, prefix: &str) -> Vec<El<App>> {
    let query = app.sidebar_query.trim().to_lowercase();
    let filter_key = if prefix == "side" {
        "sidebar-filter".to_owned()
    } else {
        format!("{prefix}-filter")
    };
    let mut filter = text_field(&app.sidebar_query)
        .key(filter_key)
        .accessible_label("Filter component pages")
        .placeholder("Filter…")
        .leading_icon(volt_ui_kit::ui::Icon::Search)
        .height(32.)
        .font_size(13.)
        .background(ColorToken::BaseContent.alpha(5))
        .border_color(ColorToken::BaseContent.alpha(10))
        .on_change(|app: &mut App, v| app.sidebar_query = v);
    // The clear button appears only when there is something to clear.
    if !app.sidebar_query.is_empty() {
        filter = filter.trailing_action(
            format!("{prefix}-filter-clear"),
            volt_ui_kit::ui::Icon::Close,
            "Clear filter",
            |app: &mut App| app.sidebar_query.clear(),
        );
    }
    let mut items: Vec<El<App>> = vec![filter.into()];
    let mut group = "";
    let mut shown = 0;
    for page in PAGES {
        if !query.is_empty() && !page.title().to_lowercase().contains(&query) {
            continue;
        }
        if page.group() != group {
            group = page.group();
            items.push(
                vstack![text(group)
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::BaseContent)]
                .key(format!(
                    "{}group-{}",
                    if prefix == "side" {
                        String::new()
                    } else {
                        format!("{prefix}-")
                    },
                    group.replace(' ', "-").to_lowercase()
                ))
                .padding(16., Edge::Top)
                .padding(4., Edge::Bottom)
                .padding(10., Edge::Left)
                .into(),
            );
        }
        shown += 1;
        let active = app.page == page;
        let mut link = button(page.title())
            .key(format!("{prefix}-{}", page.slug()))
            .variant(ButtonVariant::Ghost)
            .label_align(Align::Start)
            .height(30.)
            .font_size(13.)
            .flex(1.)
            .on_click(move |app: &mut App| app.go(page));
        link = if active {
            link.background(ColorToken::BaseContent.alpha(if app.glass { 10 } else { 7 }))
                .font_weight(600)
        } else {
            link.color(ColorToken::Muted)
        };
        let row: El<App> = if page.is_new() {
            hstack![
                link,
                badge("New")
                    .font_size(10.)
                    .rounded(Rounding::Full)
                    .background(ColorToken::Primary.alpha(14))
                    .color(ColorToken::Primary),
            ]
            .gap(6., Axis::Horizontal)
            .align(Align::Center)
            .into()
        } else {
            link.into()
        };
        items.push(row);
    }
    if shown == 0 {
        items.push(
            text("No matching pages.")
                .font_size(13.)
                .color(ColorToken::Muted)
                .into(),
        );
    }
    items
}

/// Narrow windows: the same page list in a left drawer.
fn nav_drawer(app: &App) -> El<App> {
    drawer(
        vstack(page_links(app, "drawer"))
            .gap(2., Axis::Vertical)
            .align(Align::Stretch),
    )
    .key("nav-drawer")
    .title("Volt UI")
    .side(DrawerSide::Left)
    .size(280.)
    .open(app.nav_open)
    .on_dismiss(|app: &mut App| app.nav_open = false)
    .into()
}

fn sidebar(app: &App) -> El<App> {
    let items = page_links(app, "side");
    // The traffic lights sit over the sidebar on macOS; elsewhere the brand
    // row starts at the top.
    let mac = cfg!(target_os = "macos");
    let brand = hstack![
        mark(24.),
        text("Volt UI").font_size(15.).font_weight(700),
        badge(concat!(
            env!("CARGO_PKG_VERSION_MAJOR"),
            ".",
            env!("CARGO_PKG_VERSION_MINOR")
        ))
        .font_size(10.)
        .rounded(Rounding::Full)
        .background(ColorToken::BaseContent.alpha(8))
        .color(ColorToken::Muted),
    ]
    .key("brand")
    .height(40.)
    .gap(8., Axis::Horizontal)
    .align(Align::Center)
    .padding(18., Edge::Horizontal);
    vstack![
        vstack![]
            .key("titlebar-space")
            .height(if mac { 48. } else { 8. })
            .drag_region(true),
        brand,
        vstack(items)
            .key("sidebar")
            .accessible_label("Component navigation")
            .flex(1.)
            .padding(12., Edge::Horizontal)
            .padding(8., Edge::Top)
            .padding(16., Edge::Bottom)
            .gap(2., Axis::Vertical)
            .scroll(Scroll::Vertical)
            .scrollbars(Scrollbars::Hidden),
        glass_controls(app),
    ]
    .key("sidebar-column")
    .visible(responsive(false).at(Breakpoint::Lg, true))
    .align(Align::Stretch)
    .into()
}

/// Separate background opacity, tint and native blur source.
fn glass_controls(app: &App) -> El<App> {
    if !app.glass {
        return spacer().height(0.).flex(0.).into();
    }
    let control = |label: &'static str,
                   key: &'static str,
                   value: u8,
                   min: u8,
                   change: fn(&mut App, f32)|
     -> El<App> {
        vstack![
            hstack![
                text(label)
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::Muted),
                spacer(),
                text(format!("{value}%"))
                    .font_size(12.)
                    .color(ColorToken::Muted)
            ]
            .align(Align::Center),
            slider(value as f32)
                .key(key)
                .accessible_label(label)
                .range(min as f32..=100.)
                .step(1.)
                .height(20.)
                .on_change(change),
        ]
        .gap(4., Axis::Vertical)
        .into()
    };
    let source: El<App> = if cfg!(target_os = "macos") {
        segmented_control([(0u8, "Material"), (1, "Compositor")])
            .selected(if app.compositor_blur { 1 } else { 0 })
            .key("glass-source")
            .accessible_label("Blur source")
            .on_select(|app: &mut App, value| app.compositor_blur = value == 1)
            .into()
    } else {
        spacer().height(0.).flex(0.).into()
    };
    vstack![
        control(
            "Background opacity",
            "background-opacity",
            app.opacity,
            OPACITY_MIN,
            |app, value| app.opacity = value.round() as u8
        ),
        control(
            "Tint strength",
            "sidebar-tint",
            app.tint,
            0,
            |app, value| app.tint = value.round() as u8
        ),
        toggle("Backdrop blur", app.blur)
            .key("window-blur")
            .on_change(|app: &mut App, value| app.blur = value),
        source,
        text(if app.compositor_blur {
            "Compositor: private macOS API"
        } else {
            "Material adds its own system tint"
        })
        .font_size(10.)
        .color(ColorToken::Muted),
    ]
    .key("glass-controls")
    .gap(8., Axis::Vertical)
    .padding(18., Edge::Horizontal)
    .padding(12., Edge::Vertical)
    .into()
}

fn article(app: &App) -> El<App> {
    let page = app.page;
    let content = doc(page, app);
    let mut parts: Vec<El<App>> = vec![
        {
            // Docs → the group's first page → this page.
            let first = PAGES
                .into_iter()
                .find(|p| p.group() == page.group())
                .unwrap_or(page);
            breadcrumb([
                (Page::Introduction, "Docs"),
                (first, page.group()),
                (page, page.title()),
            ])
            .key("breadcrumb")
            .on_select(|app: &mut App, target| app.go(target))
            .into()
        },
        hstack![
            text(page.title())
                .font_family(super::landing::HEADING)
                .font_size(44.)
                .font_weight(500),
            if page.is_new() {
                El::from(
                    badge("New")
                        .rounded(Rounding::Full)
                        .background(ColorToken::Primary.alpha(14))
                        .color(ColorToken::Primary),
                )
            } else {
                spacer().width(0.).flex(0.).into()
            },
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        text(page.summary())
            .font_size(17.)
            .color(ColorToken::Muted)
            .into(),
    ];
    for section in content.sections {
        let showing_code = app.code.contains(section.id);
        let id = section.id;
        let lines = section.code.lines().count() as f32;
        let switch = tab_bar([tab_item(false, "Preview"), tab_item(true, "Code")])
            .key(format!("{id}-view"))
            .selected(showing_code)
            .on_select(move |app: &mut App, code| {
                if code {
                    app.code.insert(id.to_string());
                } else {
                    app.code.remove(id);
                }
            })
            .height(30.)
            .font_size(12.);
        let body: El<App> = if showing_code {
            // Selectable, read-only source; Cmd/Ctrl+C copies a selection.
            text_area(section.code)
                .key(format!("{id}-code"))
                .read_only(true)
                .on_change(|_: &mut App, _| {})
                .fill_width()
                .height(lines * 19. + 34.)
                .font_family("SF Mono")
                .font_size(13.)
                .color("#E4E4E7")
                .background("#09090B")
                .border_color("#27272A")
                .rounded(10.)
                .into()
        } else {
            vstack![section.demo]
                .key(format!("{id}-preview"))
                .align(Align::Center)
                .padding(40., Edge::Vertical)
                .padding(24., Edge::Horizontal)
                .border_color(ColorToken::Border)
                .border_width(1.)
                .rounded(10.)
                .into()
        };
        parts.push(
            vstack![
                text(section.title).font_size(20.).font_weight(600),
                text(section.note).color(ColorToken::Muted),
                hstack![switch, spacer()],
                body,
            ]
            .key(format!("section-{id}"))
            .gap(10., Axis::Vertical)
            .padding(18., Edge::Top)
            .into(),
        );
    }
    if !content.api.is_empty() {
        let rows = content
            .api
            .iter()
            .enumerate()
            .map(|(i, (name, about))| table_row(i, [*name, *about]).key(format!("api-{i}")));
        parts.push(text("API reference").font_size(20.).font_weight(600).into());
        parts.push(
            table(
                [
                    table_column(0u8, "Method").width(340.).sortable(false),
                    table_column(1, "Description").width(400.).sortable(false),
                ],
                rows,
            )
            .key("api")
            .on_select(|_: &mut App, _| {})
            .font_size(13.)
            .into(),
        );
    }
    let index = PAGES.iter().position(|p| *p == page).unwrap_or(0);
    let prev = index.checked_sub(1).map(|i| PAGES[i]);
    let next = PAGES.get(index + 1).copied();
    let link = |target: Option<Page>, forward: bool| -> El<App> {
        match target {
            Some(target) => button(if forward {
                format!("{} ›", target.title())
            } else {
                format!("‹ {}", target.title())
            })
            .key(if forward { "page-next" } else { "page-prev" })
            .variant(ButtonVariant::Outline)
            .on_click(move |app: &mut App| app.go(target))
            .into(),
            None => spacer().width(0.).flex(0.).into(),
        }
    };
    parts.push(divider().background(ColorToken::Border).into());
    parts.push(hstack![link(prev, false), spacer(), link(next, true)].into());
    vstack(parts).gap(14., Axis::Vertical).into()
}

/// Site search (⌘K): every page, grouped like the sidebar; summaries are
/// searchable keywords. Enter opens the best match.
fn palette(app: &App) -> El<App> {
    use volt_ui_kit::ui::Icon as Glyph;
    let commands = PAGES.into_iter().map(|page| {
        let glyph = match page.group() {
            "Getting started" => Glyph::Home,
            "Patterns" => Glyph::Grid,
            _ => Glyph::File,
        };
        command(page, page.title())
            .group(page.group())
            .icon(glyph)
            .keywords(page.summary().split_whitespace())
    });
    command_palette(commands, &app.palette)
        .key("palette")
        .placeholder("Search components and guides…")
        .state(|app: &mut App| &mut app.palette)
        .on_run(|app: &mut App, page| app.go(page))
        .into()
}

/// Confirmations ("Copied", "Saved") through the toolkit's toaster.
fn toast(app: &App) -> El<App> {
    let toasts: Vec<Toast> = app
        .toast
        .iter()
        .map(|(id, message)| Toast::new(u64::from(*id), message.clone()).tone(Severity::Success))
        .collect();
    toaster(&toasts)
        .key("toast-layer")
        .on_dismiss(|app: &mut App, _| app.toast = None)
        .into()
}
