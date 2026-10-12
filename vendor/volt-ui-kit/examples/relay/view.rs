//! Layout: a frosted sidebar of threads, and a solid sheet holding the
//! transcript, the composer and an inspector card.
use super::{
    icons::{icon, icon_button, mark, Icon},
    Access, App, Effort, Message, Model, Role, Thread,
};
use volt_ui_kit::ui::{prelude::*, Element as El, Text};

const SIDEBAR: f32 = 272.;
const COLUMN: f32 = 760.;
const MAC: bool = cfg!(target_os = "macos");

pub fn view(app: &App) -> El<App> {
    let mut row: Vec<El<App>> = vec![];
    if app.sidebar {
        row.push(sidebar(app));
    }
    row.push(sheet(app));
    zstack![hstack(row).align(Align::Stretch), settings(app), toast(app),]
        .key("relay")
        .align(Align::Stretch)
        .font_family("SF")
        .font_size(14.)
        .color(ColorToken::BaseContent)
        // With glass the window clears fully transparent and this fill is
        // the material's tint, adjustable live from Settings.
        .background(if app.glass {
            ColorToken::Base100.alpha(app.tint)
        } else {
            ColorToken::Base200.into()
        })
        .into()
}

/// Shorten to `max` characters with an ellipsis; sidebar rows are one line.
fn fit(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }
    let cut: String = value.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

fn muted(value: impl Into<String>) -> Text<App> {
    text(value.into()).color(ColorToken::Muted)
}

/// Left padding that keeps content clear of the traffic lights when the
/// sidebar is hidden.
fn traffic_lights(app: &App) -> f32 {
    if MAC && !app.sidebar {
        84.
    } else {
        16.
    }
}

// ───────────────────────────── Sidebar ─────────────────────────────

fn sidebar(app: &App) -> El<App> {
    let titlebar = hstack![
        spacer(),
        icon_button(
            "hide-sidebar",
            "Hide sidebar",
            Icon::Sidebar,
            30.,
            ColorToken::Muted,
            |app: &mut App| app.sidebar = false,
        ),
    ]
    .key("sidebar-titlebar")
    .height(if MAC { 48. } else { 40. })
    .align(Align::Center)
    .padding(10., Edge::Horizontal)
    .drag_region(true);
    let brand = hstack![
        mark(22.),
        text("Relay").font_size(16.).font_weight(700),
        spacer(),
        icon_button(
            "notifications",
            "Notifications",
            Icon::Bell,
            30.,
            ColorToken::Muted,
            |app: &mut App| app.notify("No new notifications"),
        ),
        icon_button(
            "search-threads",
            "Search threads",
            Icon::Search,
            30.,
            if app.search_open {
                ColorToken::BaseContent
            } else {
                ColorToken::Muted
            },
            |app: &mut App| {
                app.search_open = !app.search_open;
                app.query.clear();
            },
        ),
    ]
    .key("brand")
    .height(36.)
    .gap(8., Axis::Horizontal)
    .align(Align::Center)
    .padding(16., Edge::Left)
    .padding(10., Edge::Right);

    let mut items: Vec<El<App>> = vec![];
    items.push(row_button(
        "new-chat",
        "New chat",
        Icon::Compose,
        false,
        |app: &mut App| app.new_chat(),
    ));
    if app.search_open {
        items.push(
            text_field(&app.query)
                .key("thread-query")
                .accessible_label("Search threads")
                .placeholder("Search threads…")
                .height(32.)
                .font_size(13.)
                .background(ColorToken::BaseContent.alpha(6))
                .border_color(ColorToken::BaseContent.alpha(10))
                .on_change(|app: &mut App, v| app.query = v)
                .into(),
        );
    }
    let query = app.query.trim().to_lowercase();
    let visible: Vec<&Thread> = app
        .threads
        .iter()
        .filter(|t| query.is_empty() || t.title.to_lowercase().contains(&query))
        .collect();
    if query.is_empty() {
        // Pinned threads, grouped under their project.
        let pinned: Vec<&Thread> = visible.iter().copied().filter(|t| t.pinned).collect();
        if !pinned.is_empty() {
            items.push(section("Pinned"));
            let mut projects: Vec<&str> = pinned.iter().map(|t| t.project.as_str()).collect();
            projects.dedup();
            projects.sort_unstable();
            projects.dedup();
            for project in projects {
                items.push(project_row(app, project, "pinned"));
                for t in pinned.iter().filter(|t| t.project == project) {
                    items.push(thread_row(app, t, "pin", true));
                }
            }
        }
        items.push(section("Projects"));
        let mut projects: Vec<&str> = app.threads.iter().map(|t| t.project.as_str()).collect();
        projects.sort_unstable();
        projects.dedup();
        for project in projects {
            items.push(project_row(app, project, "projects"));
        }
    }
    items.push(section(if query.is_empty() {
        "Recents"
    } else {
        "Results"
    }));
    if visible.is_empty() {
        items.push(
            muted("No matching threads.")
                .font_size(13.)
                .key("no-results")
                .into(),
        );
    }
    for t in &visible {
        items.push(thread_row(app, t, "recent", false));
    }

    let footer = hstack![
        zstack![text("NH").font_size(11.).font_weight(700).color("#FFFFFF")]
            .width(28.)
            .height(28.)
            .flow(Flow::Center)
            .rounded(Rounding::Full)
            .background("#E8833A"),
        vstack![
            text("You").font_size(13.).font_weight(600),
            muted("Offline demo").font_size(11.),
        ],
        spacer(),
        icon_button(
            "open-settings",
            "Settings",
            Icon::Gear,
            30.,
            ColorToken::Muted,
            |app: &mut App| app.settings_open = true,
        ),
    ]
    .key("account")
    .height(52.)
    .gap(10., Axis::Horizontal)
    .align(Align::Center)
    .padding(16., Edge::Left)
    .padding(10., Edge::Right);

    vstack![
        titlebar,
        brand,
        vstack(items)
            .key("threads")
            .accessible_label("Threads")
            .flex(1.)
            .gap(2., Axis::Vertical)
            .padding(10., Edge::Horizontal)
            .padding(10., Edge::Top)
            .padding(12., Edge::Bottom)
            .scroll(Scroll::Vertical)
            .scrollbars(Scrollbars::Hidden),
        divider().background(ColorToken::BaseContent.alpha(8)),
        footer,
    ]
    .key("sidebar")
    .width(SIDEBAR)
    .align(Align::Stretch)
    .background(if app.glass {
        "#00000000".to_owned()
    } else {
        ColorToken::Base200.into()
    })
    .into()
}

fn section(label: &'static str) -> El<App> {
    vstack![muted(label).font_size(12.).font_weight(600)]
        .key(format!("section-{}", label.to_lowercase()))
        .padding(14., Edge::Top)
        .padding(4., Edge::Bottom)
        .padding(10., Edge::Left)
        .into()
}

/// A full-width sidebar row: an accessible ghost button underneath, with the
/// visible icon, label and status layered on top.
#[allow(clippy::too_many_arguments)]
fn nav_row(
    key: String,
    label: String,
    max: usize,
    leading: Option<El<App>>,
    trailing: Option<El<App>>,
    active: bool,
    strong: bool,
    height: f32,
    on_click: impl FnMut(&mut App) + 'static,
) -> El<App> {
    // Visible labels may be shortened; the accessible name never is.
    let visible = fit(&label, max);
    let mut b = button("")
        .key(key.clone())
        .accessible_label(label)
        .variant(ButtonVariant::Ghost)
        .height(height)
        .on_click(on_click);
    if active {
        b = b.background(ColorToken::BaseContent.alpha(10));
    }
    let mut row: Vec<El<App>> = vec![];
    row.extend(leading);
    row.push(
        text(visible)
            .font_size(13.)
            .font_weight(if strong { 600 } else { 400 })
            .color(if active || strong {
                ColorToken::BaseContent
            } else {
                ColorToken::Muted
            })
            .flex(1.)
            .height(18.)
            .clip_contents(true)
            .into(),
    );
    row.extend(trailing);
    zstack![
        b,
        hstack(row)
            .gap(10., Axis::Horizontal)
            .align(Align::Center)
            .padding(10., Edge::Left)
            .padding(10., Edge::Right),
    ]
    .key(format!("{key}-row"))
    .height(height)
    .align(Align::Stretch)
    .into()
}

fn row_button(
    key: &str,
    label: &str,
    glyph: Icon,
    active: bool,
    on_click: impl FnMut(&mut App) + 'static,
) -> El<App> {
    nav_row(
        key.into(),
        label.into(),
        30,
        Some(icon(glyph, 16., ColorToken::BaseContent)),
        None,
        active,
        true,
        32.,
        on_click,
    )
}

fn project_row(app: &App, project: &str, group: &str) -> El<App> {
    let target = app
        .threads
        .iter()
        .find(|t| t.project == project)
        .map(|t| t.id);
    let current = group == "projects" && app.thread().project == project;
    nav_row(
        format!("{group}-project-{project}"),
        project.into(),
        30,
        Some(icon(Icon::Folder, 16., ColorToken::Muted)),
        None,
        false,
        current,
        30.,
        move |app: &mut App| {
            if let Some(id) = target {
                app.open(id);
            }
        },
    )
}

fn thread_row(app: &App, t: &Thread, group: &str, indent: bool) -> El<App> {
    let id = t.id;
    let status: Option<El<App>> = if app.running_thread() == Some(id) {
        Some(spinner().width(14.).height(14.).into())
    } else if t.unread {
        Some(
            vstack![]
                .width(7.)
                .height(7.)
                .rounded(Rounding::Full)
                .background(ColorToken::Primary)
                .into(),
        )
    } else {
        None
    };
    nav_row(
        format!("{group}-thread-{id}"),
        t.title.clone(),
        if indent { 26 } else { 30 },
        // Pinned threads sit under their project, aligned with its label.
        indent.then(|| spacer().width(16.).flex(0.).into()),
        status,
        id == app.active,
        false,
        30.,
        move |app: &mut App| app.open(id),
    )
}

// ───────────────────────────── Sheet ─────────────────────────────

fn sheet(app: &App) -> El<App> {
    let mut body: Vec<El<App>> = vec![vstack![transcript(app), composer(app)]
        .key("conversation")
        .flex(1.)
        .align(Align::Stretch)
        .into()];
    if app.inspector {
        body.push(inspector(app));
    }
    let sheet = vstack![
        header(app),
        hstack(body)
            .key("sheet-body")
            .flex(1.)
            .align(Align::Stretch),
    ]
    .key("sheet")
    .flex(1.)
    .align(Align::Stretch)
    .background(ColorToken::Base100);
    if !app.glass {
        return sheet.into();
    }
    // Float the sheet inside the material: inset on every free edge, it never
    // meets the window's rounded corners whatever their radius. No rounded
    // clip layer: content is padded clear of the corners.
    let mut frame = vstack![
        vstack![].key("sheet-drag").height(8.).drag_region(true),
        sheet
            .rounded(12.)
            .border_color(ColorToken::BaseContent.alpha(9))
            .border_width(1.)
            .shadow(Shadow::new().color("#00000030").blur(16.).offset(0., 2.)),
    ]
    .key("sheet-frame")
    .flex(1.)
    .align(Align::Stretch)
    .padding(8., Edge::Right)
    .padding(8., Edge::Bottom);
    if !app.sidebar {
        frame = frame.padding(8., Edge::Left);
    }
    frame.into()
}

fn header(app: &App) -> El<App> {
    let t = app.thread();
    let mut left: Vec<El<App>> = vec![];
    if !app.sidebar {
        left.push(icon_button(
            "show-sidebar",
            "Show sidebar",
            Icon::Sidebar,
            30.,
            ColorToken::Muted,
            |app: &mut App| app.sidebar = true,
        ));
    }
    left.push(icon(Icon::Folder, 16., ColorToken::Muted));
    left.push(muted(t.project.clone()).font_size(13.).into());
    left.push(muted("/").font_size(13.).into());
    left.push(text(t.title.clone()).font_size(13.).font_weight(600).into());
    if t.pinned {
        left.push(icon(Icon::Pin, 14., ColorToken::Muted));
    }
    let pinned = t.pinned;
    let item = |key: &'static str, label: &'static str, danger: bool, f: fn(&mut App)| -> El<App> {
        button(label)
            .key(key)
            .variant(ButtonVariant::Ghost)
            .label_align(Align::Start)
            .height(32.)
            .font_size(13.)
            .fill_width()
            .color(if danger {
                ColorToken::Error
            } else {
                ColorToken::BaseContent
            })
            .on_click(f)
            .into()
    };
    let actions = popover(
        vstack![
            item(
                "action-pin",
                if pinned { "Unpin thread" } else { "Pin thread" },
                false,
                App::toggle_pin
            ),
            item("action-new", "New chat in project", false, |app| {
                app.thread_menu_open = false;
                app.new_chat();
            }),
            divider().background(ColorToken::Border),
            item("action-delete", "Delete thread", true, App::delete_thread),
        ]
        .gap(2., Axis::Vertical),
    )
    .key("thread-actions")
    .anchor("thread-more")
    .open(app.thread_menu_open)
    .width(212.)
    .on_dismiss(|app: &mut App| app.thread_menu_open = false);
    let (added, removed) = t.changes();
    let mut right: Vec<El<App>> = vec![];
    if added != 0 || removed != 0 {
        right.push(
            hstack![
                text(format!("+{added}"))
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::Success),
                text(format!("−{removed}"))
                    .font_size(12.)
                    .font_weight(600)
                    .color(ColorToken::Error),
            ]
            .key("header-changes")
            .gap(6., Axis::Horizontal)
            .align(Align::Center)
            .into(),
        );
    }
    right.push(icon_button(
        "thread-more",
        "Thread actions",
        Icon::More,
        30.,
        ColorToken::Muted,
        |app: &mut App| app.thread_menu_open = !app.thread_menu_open,
    ));
    right.push(actions.into());
    right.push(icon_button(
        "toggle-inspector",
        if app.inspector {
            "Hide inspector"
        } else {
            "Show inspector"
        },
        Icon::Inspector,
        30.,
        if app.inspector {
            ColorToken::BaseContent
        } else {
            ColorToken::Muted
        },
        |app: &mut App| app.inspector = !app.inspector,
    ));
    hstack![
        hstack(left).gap(8., Axis::Horizontal).align(Align::Center),
        spacer(),
        hstack(right).gap(6., Axis::Horizontal).align(Align::Center),
    ]
    .key("header")
    .height(if MAC { 48. } else { 44. })
    .align(Align::Center)
    .padding(traffic_lights(app), Edge::Left)
    .padding(12., Edge::Right)
    .drag_region(true)
    .into()
}

/// A centred column no wider than `COLUMN`.
fn column(key: &str, child: El<App>) -> El<App> {
    hstack![spacer(), vstack![child].width(COLUMN), spacer()]
        .key(key)
        .padding(28., Edge::Horizontal)
        .into()
}

fn transcript(app: &App) -> El<App> {
    let t = app.thread();
    let content: El<App> = if t.messages.is_empty() {
        empty_state(app)
    } else {
        let last = t.messages.len() - 1;
        let messages: Vec<El<App>> = t
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| match m.role {
                Role::User => user_message(t.id, i, m),
                Role::Agent => agent_message(app, t.id, i, m, i == last),
            })
            .collect();
        vstack(messages).gap(26., Axis::Vertical).into()
    };
    vstack![column("transcript-column", content)]
        // One scroll view per thread, so each keeps its own position.
        .key(format!("transcript-{}", t.id))
        .accessible_label("Conversation")
        .flex(1.)
        .padding(28., Edge::Top)
        .padding(24., Edge::Bottom)
        .scroll(Scroll::Vertical)
        .anchor_end(true)
        .into()
}

fn empty_state(app: &App) -> El<App> {
    let project = app.thread().project.clone();
    let suggestions = [
        (
            "Fix the GPU border bug",
            "fix the grey band on the GPU renderer",
        ),
        ("Run the test suite", "run the tests with every feature"),
        (
            "Polish the catalog design",
            "design a frosted glass sidebar",
        ),
        ("Explain the architecture", "explain how this project works"),
    ];
    let cards: Vec<El<App>> = suggestions
        .iter()
        .enumerate()
        .map(|(i, (title, prompt))| {
            let send = prompt.to_string();
            zstack![
                button("")
                    .key(format!("suggestion-{i}"))
                    .accessible_label(*title)
                    .variant(ButtonVariant::Outline)
                    .rounded(14.)
                    .hover_style(|s| s.background(ColorToken::BaseContent.alpha(5)))
                    .on_click(move |app: &mut App| app.send(Some(send.clone()))),
                vstack![
                    text(*title).font_size(14.).font_weight(600),
                    muted(*prompt).font_size(12.),
                ]
                .gap(4., Axis::Vertical)
                .padding(14., Edge::All),
            ]
            .key(format!("suggestion-{i}-layer"))
            .flow(Flow::TopLeft)
            .align(Align::Stretch)
            .height(72.)
            .width((COLUMN - 12.) / 2.)
            .into()
        })
        .collect();
    let mut rows = cards.into_iter();
    let mut grid: Vec<El<App>> = vec![];
    while let (Some(a), b) = (rows.next(), rows.next()) {
        let row: Vec<El<App>> = match b {
            Some(b) => vec![a, b],
            None => vec![a],
        };
        grid.push(hstack(row).gap(12., Axis::Horizontal).into());
    }
    vstack![
        spacer().height(80.).flex(0.),
        mark(44.),
        text(format!("What should we build in {project}?"))
            .font_size(26.)
            .font_weight(700),
        muted("Relay reads, edits and tests code in your workspace. Replies here are scripted locally.")
            .font_size(14.),
        spacer().height(12.).flex(0.),
        vstack(grid).gap(12., Axis::Vertical),
    ]
    .key("empty")
    .gap(12., Axis::Vertical)
    .into()
}

fn user_message(thread: u32, index: usize, m: &Message) -> El<App> {
    hstack![
        spacer().width(120.).flex(0.),
        spacer(),
        vstack![text(m.text.clone()).font_size(14.)]
            .key(format!("bubble-{thread}-{index}"))
            .padding(10., Edge::Vertical)
            .padding(14., Edge::Horizontal)
            .rounded(18.)
            .background(ColorToken::BaseContent.alpha(7)),
    ]
    .key(format!("message-{thread}-{index}"))
    .into()
}

fn agent_message(app: &App, thread: u32, index: usize, m: &Message, last: bool) -> El<App> {
    let key = format!("{thread}-{index}");
    let mut parts: Vec<El<App>> = vec![];
    // Work summary: live progress while streaming, a disclosure afterwards.
    if m.streaming {
        let label = m
            .steps
            .iter()
            .rev()
            .find(|s| !s.done)
            .map(|s| s.label.clone())
            .unwrap_or_else(|| {
                if m.text.is_empty() {
                    "Thinking…".into()
                } else {
                    "Writing…".into()
                }
            });
        parts.push(
            hstack![
                spinner().width(14.).height(14.),
                muted(label).font_size(13.)
            ]
            .key(format!("progress-{key}"))
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .into(),
        );
    } else if let Some(secs) = m.worked {
        let expanded = m.expanded;
        parts.push(
            hstack![zstack![
                button("")
                    .key(format!("worked-{key}"))
                    .accessible_label(format!("Worked for {secs} seconds"))
                    .variant(ButtonVariant::Ghost)
                    .height(26.)
                    .rounded(6.)
                    .on_click(move |app: &mut App| {
                        let id = app.active;
                        if let Some(m) = app
                            .threads
                            .iter_mut()
                            .find(|t| t.id == id)
                            .and_then(|t| t.messages.get_mut(index))
                        {
                            m.expanded = !m.expanded;
                        }
                    }),
                hstack![
                    muted(format!("Worked for {secs}s")).font_size(13.),
                    icon(
                        if expanded {
                            Icon::ChevronDown
                        } else {
                            Icon::ChevronRight
                        },
                        14.,
                        ColorToken::Muted
                    ),
                ]
                .gap(4., Axis::Horizontal)
                .align(Align::Center)
                .padding(6., Edge::Horizontal),
            ]
            .key(format!("worked-{key}-layer"))
            .width(150.)
            .height(26.)
            .align(Align::Stretch)]
            .into(),
        );
    }
    if (m.streaming || m.expanded) && !m.steps.is_empty() {
        let steps: Vec<El<App>> = m
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let glyph = if s.label.starts_with("Ran") {
                    Icon::Terminal
                } else if s.label.starts_with("Edited") {
                    Icon::Diff
                } else if s.label.starts_with("Read") {
                    Icon::File
                } else {
                    Icon::Search
                };
                let status: El<App> = if s.done {
                    icon(Icon::Check, 14., ColorToken::Success)
                } else {
                    spinner().width(14.).height(14.).into()
                };
                hstack![
                    icon(glyph, 15., ColorToken::Muted),
                    text(s.label.clone()).font_size(13.),
                    muted(s.detail.clone()).font_size(12.).flex(1.),
                    status,
                ]
                .key(format!("step-{key}-{i}"))
                .gap(10., Axis::Horizontal)
                .align(Align::Center)
                .height(26.)
                .into()
            })
            .collect();
        parts.push(
            vstack(steps)
                .key(format!("steps-{key}"))
                .gap(2., Axis::Vertical)
                .padding(12., Edge::Left)
                .padding(4., Edge::Vertical)
                .border_color(ColorToken::BaseContent.alpha(10))
                .border_width(0.)
                .into(),
        );
    }
    parts.extend(rich_text(&key, &m.text));
    if !m.files.is_empty() {
        parts.push(files_card(&key, m));
    }
    if !m.streaming && !m.text.is_empty() {
        let liked = m.liked;
        let mut actions: Vec<El<App>> = vec![icon_button(
            format!("like-{key}"),
            if liked {
                "Remove like"
            } else {
                "Good response"
            },
            Icon::Heart,
            28.,
            if liked {
                ColorToken::Error
            } else {
                ColorToken::Muted
            },
            move |app: &mut App| {
                let id = app.active;
                if let Some(m) = app
                    .threads
                    .iter_mut()
                    .find(|t| t.id == id)
                    .and_then(|t| t.messages.get_mut(index))
                {
                    m.liked = !m.liked;
                }
            },
        )];
        if last && !app.running() {
            actions.push(icon_button(
                format!("retry-{key}"),
                "Retry",
                Icon::Retry,
                28.,
                ColorToken::Muted,
                |app: &mut App| app.retry(),
            ));
        }
        parts.push(
            hstack(actions)
                .key(format!("actions-{key}"))
                .gap(2., Axis::Horizontal)
                .into(),
        );
    }
    vstack(parts)
        .key(format!("message-{key}"))
        .gap(10., Axis::Vertical)
        .into()
}

/// Paragraphs, "- " bullets and fenced code blocks.
fn rich_text(key: &str, source: &str) -> Vec<El<App>> {
    let mut out: Vec<El<App>> = vec![];
    let mut code: Option<Vec<&str>> = None;
    let mut paragraph: Vec<&str> = vec![];
    let flush = |paragraph: &mut Vec<&str>, out: &mut Vec<El<App>>| {
        if paragraph.is_empty() {
            return;
        }
        let n = out.len();
        out.push(
            text(paragraph.join(" "))
                .key(format!("p-{key}-{n}"))
                .font_size(14.)
                .into(),
        );
        paragraph.clear();
    };
    for line in source.lines() {
        if out.len() >= 32 {
            break;
        }
        if line.trim_start().starts_with("```") {
            flush(&mut paragraph, &mut out);
            match code.take() {
                Some(lines) => {
                    let n = out.len();
                    out.push(
                        vstack![text(lines.join("\n"))
                            .font_family("SF Mono")
                            .font_size(12.5)]
                        .key(format!("code-{key}-{n}"))
                        .padding(14., Edge::All)
                        .rounded(10.)
                        .background(ColorToken::BaseContent.alpha(5))
                        .border_color(ColorToken::Border)
                        .border_width(1.)
                        .into(),
                    );
                }
                None => code = Some(vec![]),
            }
            continue;
        }
        if let Some(lines) = code.as_mut() {
            lines.push(line);
            continue;
        }
        if let Some(item) = line.strip_prefix("- ") {
            flush(&mut paragraph, &mut out);
            let n = out.len();
            out.push(
                hstack![
                    muted("•").width(14.),
                    text(item.to_owned()).font_size(14.).flex(1.)
                ]
                .key(format!("li-{key}-{n}"))
                .into(),
            );
        } else if line.trim().is_empty() {
            flush(&mut paragraph, &mut out);
        } else {
            paragraph.push(line);
        }
    }
    // An unterminated fence while streaming renders as plain text so far.
    if let Some(lines) = code {
        paragraph.extend(lines);
    }
    if out.len() < 32 {
        flush(&mut paragraph, &mut out);
    }
    out
}

fn files_card(key: &str, m: &Message) -> El<App> {
    let (added, removed) = m.files.iter().fold((0u32, 0u32), |(a, r), f| {
        (a.saturating_add(f.added), r.saturating_add(f.removed))
    });
    let mut rows: Vec<El<App>> = vec![hstack![
        icon(Icon::Diff, 15., ColorToken::Muted),
        text(format!(
            "{} file{} changed",
            m.files.len(),
            if m.files.len() == 1 { "" } else { "s" }
        ))
        .font_size(13.)
        .font_weight(600),
        spacer(),
        text(format!("+{added}"))
            .font_size(12.)
            .color(ColorToken::Success),
        text(format!("−{removed}"))
            .font_size(12.)
            .color(ColorToken::Error),
    ]
    .key(format!("files-head-{key}"))
    .gap(8., Axis::Horizontal)
    .align(Align::Center)
    .height(36.)
    .padding(12., Edge::Horizontal)
    .into()];
    for (i, f) in m.files.iter().enumerate() {
        rows.push(divider().background(ColorToken::Border).into());
        rows.push(
            hstack![
                text(f.path.clone()).font_family("SF Mono").font_size(12.),
                spacer(),
                text(format!("+{}", f.added))
                    .font_size(12.)
                    .color(ColorToken::Success),
                text(format!("−{}", f.removed))
                    .font_size(12.)
                    .color(ColorToken::Error),
            ]
            .key(format!("file-{key}-{i}"))
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .height(32.)
            .padding(12., Edge::Horizontal)
            .into(),
        );
    }
    vstack(rows)
        .key(format!("files-{key}"))
        .rounded(12.)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .clip(Clip::Rounded)
        .into()
}

// ───────────────────────────── Composer ─────────────────────────────

fn composer(app: &App) -> El<App> {
    let running = app.running();
    let busy_elsewhere = app.running_thread().is_some() && !running;
    let menu_style = |m: Menu<App, u8>| {
        m.height(30.)
            .font_size(12.)
            .background("#00000000")
            .border_width(0.)
            .color(ColorToken::Muted)
    };
    let model = menu_style(
        menu(
            app.model.label(),
            Model::ALL
                .iter()
                .enumerate()
                .map(|(i, m)| menu_item(i as u8, m.label())),
        )
        .key("model")
        .accessible_label("Model")
        .width(118.)
        .open(app.model_open)
        .on_action(|app: &mut App, i| app.model = Model::ALL[i as usize])
        .on_open_change(|app: &mut App, open| app.model_open = open),
    );
    let effort = menu_style(
        menu(
            app.effort.label(),
            Effort::ALL
                .iter()
                .enumerate()
                .map(|(i, e)| menu_item(i as u8, e.label())),
        )
        .key("effort")
        .accessible_label("Reasoning effort")
        .width(88.)
        .open(app.effort_open)
        .on_action(|app: &mut App, i| app.effort = Effort::ALL[i as usize])
        .on_open_change(|app: &mut App, open| app.effort_open = open),
    );
    let full = app.access == Access::Full;
    let access = menu(
        app.access.label(),
        Access::ALL
            .iter()
            .enumerate()
            .map(|(i, a)| menu_item(i as u8, a.label())),
    )
    .key("access")
    .accessible_label("Permissions")
    .width(112.)
    .height(30.)
    .font_size(12.)
    .background("#00000000")
    .border_width(0.)
    .color(if full {
        ColorToken::Warning
    } else {
        ColorToken::Muted
    })
    .open(app.access_open)
    .on_action(|app: &mut App, i| app.access = Access::ALL[i as usize])
    .on_open_change(|app: &mut App, open| app.access_open = open);
    let can_send = !app.draft.trim().is_empty() && app.running_thread().is_none();
    let send: El<App> = if running {
        circle_button("stop", "Stop", Icon::Stop, true, |app: &mut App| app.stop())
    } else {
        circle_button("send", "Send", Icon::ArrowUp, can_send, |app: &mut App| {
            app.send(None)
        })
    };
    let field = text_area(&app.draft)
        .key("composer-input")
        .accessible_label("Message")
        .placeholder(if busy_elsewhere {
            "Another thread is running…"
        } else if app.enter_sends {
            "Ask Relay anything. Shift+Enter for a new line"
        } else {
            "Ask Relay anything. ⌘↩ to send"
        })
        .soft_wrap(true)
        .submit_on_enter(app.enter_sends)
        .height(60.)
        .font_size(14.)
        .background("#00000000")
        .border_width(0.)
        .on_change(|app: &mut App, v| app.draft = v)
        .on_submit(|app: &mut App| app.send(None));
    let card = vstack![
        field,
        hstack![
            icon_button(
                "attach",
                "Add context",
                Icon::Plus,
                30.,
                ColorToken::Muted,
                |app: &mut App| app.notify("Drop files here to add context"),
            ),
            hstack![
                icon(
                    Icon::Shield,
                    14.,
                    if full {
                        ColorToken::Warning
                    } else {
                        ColorToken::Muted
                    }
                ),
                access,
            ]
            .key("access-layer")
            .gap(0., Axis::Horizontal)
            .align(Align::Center)
            .padding(6., Edge::Left),
            spacer(),
            model,
            effort,
            icon_button(
                "dictate",
                "Dictate",
                Icon::Mic,
                30.,
                ColorToken::Muted,
                |app: &mut App| app.notify("Dictation isn't available in the demo"),
            ),
            send,
        ]
        .key("composer-tools")
        .gap(4., Axis::Horizontal)
        .align(Align::Center),
    ]
    .key("composer-card")
    .gap(4., Axis::Vertical)
    .padding(12., Edge::Horizontal)
    .padding(10., Edge::Vertical)
    .rounded(20.)
    .background(ColorToken::Base200)
    .border_color(ColorToken::BaseContent.alpha(10))
    .border_width(1.)
    .shadow(Shadow::new().color("#0000001A").blur(18.).offset(0., 6.));
    vstack![column("composer-column", card.into())]
        .key("composer")
        .padding(16., Edge::Bottom)
        .into()
}

fn circle_button(
    key: &'static str,
    label: &'static str,
    glyph: Icon,
    enabled: bool,
    on_click: impl FnMut(&mut App) + 'static,
) -> El<App> {
    zstack![
        button("")
            .key(key)
            .accessible_label(label)
            .variant(ButtonVariant::Primary)
            .width(32.)
            .height(32.)
            .rounded(Rounding::Full)
            .enabled(enabled)
            .on_click(on_click),
        icon(
            glyph,
            16.,
            if enabled {
                ColorToken::PrimaryContent
            } else {
                ColorToken::Muted
            }
        ),
    ]
    .key(format!("{key}-layer"))
    .width(32.)
    .height(32.)
    .flow(Flow::Center)
    .into()
}

// ───────────────────────────── Inspector ─────────────────────────────

fn inspector(app: &App) -> El<App> {
    let t = app.thread();
    let (added, removed) = t.changes();
    let files: Vec<&super::FileChange> = t.messages.iter().flat_map(|m| &m.files).collect();
    let last = t.messages.iter().rev().find(|m| m.role == Role::Agent);
    let card_title = |label: &str| -> El<App> {
        muted(label.to_owned())
            .font_size(12.)
            .font_weight(600)
            .into()
    };
    let mut parts: Vec<El<App>> = vec![
        hstack![
            text(t.project.clone()).font_size(13.).font_weight(600),
            spacer(),
            badge(if app.running() { "Running" } else { "Idle" })
                .font_size(10.)
                .rounded(Rounding::Full)
                .background(if app.running() {
                    ColorToken::Primary.alpha(16)
                } else {
                    ColorToken::BaseContent.alpha(8)
                })
                .color(if app.running() {
                    ColorToken::Primary
                } else {
                    ColorToken::Muted
                }),
        ]
        .key("inspector-head")
        .align(Align::Center)
        .into(),
        hstack![
            icon(Icon::Diff, 15., ColorToken::Muted),
            text("Changes").font_size(13.),
            spacer(),
            text(format!("+{added}"))
                .font_size(12.)
                .color(ColorToken::Success),
            text(format!("−{removed}"))
                .font_size(12.)
                .color(ColorToken::Error),
        ]
        .key("inspector-changes")
        .gap(8., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    ];
    for (i, f) in files.iter().enumerate().take(6) {
        let name = f.path.rsplit('/').next().unwrap_or(&f.path).to_owned();
        parts.push(
            hstack![
                icon(Icon::File, 14., ColorToken::Muted),
                muted(name).font_size(12.).flex(1.),
                muted(format!("+{} −{}", f.added, f.removed)).font_size(11.),
            ]
            .key(format!("inspector-file-{i}"))
            .gap(8., Axis::Horizontal)
            .align(Align::Center)
            .padding(4., Edge::Left)
            .into(),
        );
    }
    parts.push(divider().background(ColorToken::Border).into());
    parts.push(card_title("Progress"));
    match last {
        Some(m) if !m.steps.is_empty() => {
            let done = m.steps.iter().filter(|s| s.done).count();
            parts.push(
                progress(done as f32 / m.steps.len() as f32)
                    .key("inspector-progress")
                    .height(6.)
                    .into(),
            );
            for (i, s) in m.steps.iter().enumerate() {
                parts.push(
                    hstack![
                        if s.done {
                            icon(Icon::Check, 13., ColorToken::Success)
                        } else {
                            spinner().width(13.).height(13.).into()
                        },
                        text(s.label.clone()).font_size(12.).flex(1.),
                    ]
                    .key(format!("inspector-step-{i}"))
                    .gap(8., Axis::Horizontal)
                    .align(Align::Center)
                    .into(),
                );
            }
        }
        _ => parts.push(muted("No tool calls yet.").font_size(12.).into()),
    }
    parts.push(divider().background(ColorToken::Border).into());
    parts.push(card_title("Context"));
    let words: usize = t
        .messages
        .iter()
        .map(|m| m.text.split_whitespace().count())
        .sum();
    let used = (0.06 + words as f32 / 2400.).min(1.);
    parts.push(
        hstack![
            progress(used).key("context-meter").height(6.).flex(1.),
            muted(format!("{:.0}%", used * 100.))
                .font_size(11.)
                .width(30.),
        ]
        .key("context")
        .gap(10., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    );
    parts.push(
        muted(format!(
            "{} · {} effort",
            app.model.label(),
            app.effort.label()
        ))
        .font_size(11.)
        .into(),
    );
    vstack![vstack(parts)
        .key("inspector-card")
        .gap(10., Axis::Vertical)
        .padding(16., Edge::All)
        .rounded(16.)
        .background(ColorToken::Base200)
        .border_color(ColorToken::BaseContent.alpha(8))
        .border_width(1.)]
    .key("inspector")
    .width(300.)
    .padding(4., Edge::Top)
    .padding(14., Edge::Right)
    .into()
}

// ───────────────────────────── Overlays ─────────────────────────────

fn settings(app: &App) -> El<App> {
    let mode = match app.mode {
        ThemeMode::System => 0u8,
        ThemeMode::Light => 1,
        ThemeMode::Dark => 2,
    };
    dialog(
        vstack![
            text("Appearance").font_size(13.).font_weight(600),
            segmented_control([(0u8, "System"), (1, "Light"), (2, "Dark")])
                .key("appearance")
                .selected(mode)
                .on_select(|app: &mut App, v| {
                    app.mode = match v {
                        0 => ThemeMode::System,
                        1 => ThemeMode::Light,
                        _ => ThemeMode::Dark,
                    }
                }),
            divider().background(ColorToken::Border),
            toggle("Frosted sidebar", app.glass)
                .key("glass")
                .on_change(|app: &mut App, v| app.glass = v),
            hstack![
                text("Sidebar tint").font_size(13.),
                spacer(),
                muted(format!("{}%", app.tint)).font_size(12.),
            ]
            .align(Align::Center),
            slider(app.tint as f32)
                .key("sidebar-tint")
                .accessible_label("Sidebar tint")
                .range(super::TINT_MIN as f32..=100.)
                .step(1.)
                .enabled(app.glass)
                .fill_width()
                .on_change(|app: &mut App, v| app.tint = v.round() as u8),
            muted("Blur needs a translucent window: run with native-gpu.").font_size(12.),
            toggle("Send with Enter", app.enter_sends)
                .key("enter-sends")
                .on_change(|app: &mut App, v| app.enter_sends = v),
            hstack![
                spacer(),
                button("Done")
                    .key("settings-done")
                    .variant(ButtonVariant::Primary)
                    .on_click(|app: &mut App| app.settings_open = false),
            ],
        ]
        .gap(12., Axis::Vertical),
    )
    .key("settings")
    .title("Settings")
    .open(app.settings_open)
    .width(380.)
    .rounded(14.)
    .on_dismiss(|app: &mut App| app.settings_open = false)
    .into()
}

fn toast(app: &App) -> El<App> {
    let content: El<App> = match &app.toast {
        Some((id, message)) => hstack![
            text(message.clone()).font_size(13.).font_weight(500),
            button("Dismiss")
                .key("toast-dismiss")
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Small)
                .on_click(|app: &mut App| app.toast = None),
        ]
        .key(format!("toast-{id}"))
        .transition(Transition::fade().duration_ms(200))
        .gap(10., Axis::Horizontal)
        .align(Align::Center)
        .padding(14., Edge::Left)
        .padding(6., Edge::Right)
        .height(40.)
        .rounded(Rounding::Full)
        .background(ColorToken::Base300)
        .border_color(ColorToken::Border)
        .border_width(1.)
        .shadow(Shadow::new().color("#00000033").blur(20.).offset(0., 8.))
        .into(),
        None => spacer().width(0.).height(0.).into(),
    };
    vstack![content]
        .key("toast-layer")
        .flow(Flow::Bottom)
        .padding(120., Edge::Bottom)
        .scroll_passthrough(true)
        .into()
}
