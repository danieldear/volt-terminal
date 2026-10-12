use super::style::avatar;
use super::{
    state::{radius_label, RADII},
    style::*,
    App, Screen,
};
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let done = app.showcase.completed.iter().filter(|&&v| v).count();
    let chart = canvas(move |c| {
        for y in [20., 60., 100., 140.] {
            c.stroke(
                VectorPath::new().move_to(0., y).line_to(460., y),
                ColorToken::Border,
                0.7,
            );
        }
        let values = if app.showcase.week {
            [32., 48., 43., 68., 53., 87., 64., 96., 75., 104., 91., 128.]
        } else {
            [
                110., 82., 94., 70., 84., 62., 93., 104., 95., 111., 120., 130.,
            ]
        };
        for (i, value) in values.into_iter().enumerate() {
            c.fill(
                VectorPath::rectangle(i as f32 * 38. + 5., 150. - value, 22., value),
                if i >= 9 {
                    ColorToken::Primary
                } else {
                    ColorToken::Base300
                },
            );
        }
    })
    .key("workspace-chart")
    .width(460.)
    .height(156.);
    let activity = card(vec![
        hstack![
            vstack![label("CREATIVE ACTIVITY"), title("Steady progress")].flex(1.),
            action(
                if app.showcase.week {
                    "This week"
                } else {
                    "Last week"
                },
                "activity-range",
                false,
                |app| {
                    app.showcase.week = !app.showcase.week;
                    app.record("Changed activity period");
                }
            )
            .width(112.)
        ]
        .align(Align::Center)
        .into(),
        hstack![
            text(if app.showcase.week { "38.5" } else { "32.2" })
                .font_family("SF Mono")
                .font_size(38.)
                .font_weight(500),
            vstack![
                muted("hours of focused work"),
                pill(
                    "+19.6% vs previous",
                    ColorToken::Success,
                    ColorToken::SuccessContent
                )
            ]
            .gap(8., Axis::Vertical)
        ]
        .gap(16., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        hstack![chart]
            .height(164.)
            .scroll(Scroll::Horizontal)
            .scrollbars(Scrollbars::Hidden)
            .into(),
        hstack![
            label("MON"),
            spacer(),
            label("WED"),
            spacer(),
            label("FRI"),
            spacer(),
            label("SUN")
        ]
        .into(),
    ])
    .key("workspace-activity");
    let task_names = [
        "Establish the visual direction",
        "Refine the component library",
        "Review motion and keyboard flow",
        "Share the first native preview",
    ];
    let tasks = task_names
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            checkbox(name, app.showcase.completed[i])
                .key(format!("atelier-task-{i}"))
                .font_size(13.)
                .height(42.)
                .on_change(move |app: &mut App, value| {
                    app.showcase.completed[i] = value;
                    app.record(format!(
                        "Task {} {}",
                        i + 1,
                        if value { "complete" } else { "reopened" }
                    ));
                })
                .into()
        })
        .collect::<Vec<Element<App>>>();
    let mut task_children = vec![hstack![
        title("Today's intention"),
        spacer(),
        pill(
            format!("{done} / 4"),
            ColorToken::Base300,
            ColorToken::BaseContent
        )
    ]
    .into()];
    task_children.extend(tasks);
    task_children.push(muted("Small steps add up. Changes stay in this session.").into());
    let tasks = card(task_children).key("workspace-tasks");
    let days = ["M  12", "T  13", "W  14", "T  15", "F  16"];
    let calendar = card(vec![
        hstack![
            title("A little breathing room"),
            spacer(),
            label("OCT / 2026")
        ]
        .into(),
        hstack(days.into_iter().enumerate().map(|(i, day)| {
            action(
                day,
                format!("atelier-day-{i}"),
                i == app.showcase.day,
                move |app| {
                    app.showcase.day = i;
                    app.record(format!("Selected demo day {}", 12 + i));
                },
            )
            .width(58.)
            .height(48.)
            .rounded(14.)
            .into()
        }))
        .key("workspace-days")
        .height(56.)
        .gap(8., Axis::Horizontal)
        .scroll(Scroll::Horizontal)
        .scrollbars(Scrollbars::Hidden)
        .into(),
        hstack![
            avatar("AL", "#F5B895", 36.),
            vstack![
                text(
                    [
                        "Design review",
                        "Quiet focus block",
                        "Studio sync",
                        "Prototype review",
                        "Weekly wrap-up"
                    ][app.showcase.day]
                )
                .font_size(14.)
                .font_weight(600),
                muted("10:30 – 11:15  ·  Sample schedule")
            ]
            .flex(1.)
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    ]);
    let tracks = ["Soft edges", "After the rain", "A slower morning"];
    let player = card(vec![
        hstack![
            label("FOCUS / LOCAL PREVIEW"),
            spacer(),
            pill(
                if app.showcase.playing {
                    "PLAYING"
                } else {
                    "PAUSED"
                },
                ColorToken::Base300,
                ColorToken::BaseContent
            )
        ]
        .into(),
        hstack![
            avatar("♫", "#9EE0C5", 56.),
            vstack![
                title(tracks[app.showcase.track]),
                muted("Studio sessions · visual player demo")
            ]
            .flex(1.)
        ]
        .gap(14., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        hstack![
            action("Previous", "focus-previous", false, |app| {
                app.showcase.track = (app.showcase.track + 2) % 3;
                app.record("Selected previous demo track");
            })
            .flex(1.),
            action(
                if app.showcase.playing {
                    "Pause"
                } else {
                    "Play"
                },
                "focus-play",
                true,
                |app| {
                    app.showcase.playing = !app.showcase.playing;
                    app.record("Toggled visual player (no audio output)");
                }
            )
            .flex(1.),
            action("Next", "focus-next", false, |app| {
                app.showcase.track = (app.showcase.track + 1) % 3;
                app.record("Selected next demo track");
            })
            .flex(1.)
        ]
        .gap(8., Axis::Horizontal)
        .into(),
        muted("No audio file is loaded; these are real state callbacks.").into(),
    ]);
    let project = card(vec![
        hstack![
            label("PINNED PROJECT"),
            spacer(),
            pill(
                "IN PROGRESS",
                ColorToken::Secondary,
                ColorToken::SecondaryContent
            )
        ]
        .into(),
        text("The quiet\nside of bold.")
            .font_size(32.)
            .font_weight(600)
            .into(),
        canvas(|c| {
            c.fill(VectorPath::ellipse(80., 66., 100., 55.), "#BEB0F4");
            c.fill(VectorPath::circle(165., 67., 52.), "#F5B895");
            c.fill(
                VectorPath::new()
                    .move_to(96., 20.)
                    .line_to(190., 105.)
                    .line_to(96., 105.)
                    .close(),
                "#9EE0C5",
            );
            c.stroke(VectorPath::ellipse(137., 65., 112., 50.), "#17181F", 1.5);
        })
        .key("workspace-cover")
        .height(132.)
        .clip(Clip::Rounded)
        .rounded(16.)
        .background("#343342"),
        hstack![
            avatar("AL", "#F5B895", 28.),
            avatar("MK", "#BEB0F4", 28.),
            avatar("JT", "#9EE0C5", 28.),
            spacer(),
            label("3 COLLABORATORS")
        ]
        .gap(5., Axis::Horizontal)
        .align(Align::Center)
        .into(),
        action(
            if app.showcase.saved {
                "Project saved ✓"
            } else {
                "Save project"
            },
            "atelier-project-save",
            true,
            |app| {
                app.showcase.saved = !app.showcase.saved;
                app.showcase.notice = true;
                app.record("Changed local project saved state");
            },
        )
        .into(),
    ])
    .key("workspace-project")
    .rounded_on(28., [Corner::TopLeft, Corner::BottomRight]);
    let notice: Element<App> = if app.showcase.notice {
        hstack![
            pill("✓", ColorToken::Success, ColorToken::SuccessContent),
            text("Local changes are ready. Nothing was sent to a server.")
                .font_size(12.)
                .flex(1.),
            action("Dismiss", "atelier-dismiss", false, |app| {
                app.showcase.notice = false;
                app.record("Dismissed notice");
            })
            .width(84.)
        ]
        .padding(14., Edge::All)
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .background(ColorToken::Base300)
        .rounded(12.)
        .key("workspace-notice")
        .transition(Transition::fade().duration_ms(140))
        .into()
    } else {
        spacer().height(0.).into()
    };
    vstack![
        stack![
            vstack![
                label("YOUR SPACE / SAMPLE WORKSPACE"),
                text("Good work starts here.")
                    .font_size(34.)
                    .font_weight(700),
                muted("A considered home for your projects, people and next small step.")
            ]
            .flex(1.)
            .gap(10., Axis::Vertical),
            route(
                Screen::Studio,
                "workspace-components",
                "Explore components →"
            )
            .width(196.)
        ]
        .direction(responsive(Direction::Vertical).at(Breakpoint::Lg, Direction::Horizontal))
        .gap(18., Axis::Both)
        .align(Align::Center),
        notice,
        adaptive(vec![
            vstack![activity, tasks]
                .key("workspace-main")
                .flex(1.5)
                .gap(20., Axis::Vertical)
                .into(),
            vstack![project, calendar, player]
                .key("workspace-side")
                .flex(1.)
                .gap(20., Axis::Vertical)
                .into()
        ])
        .key("workspace-columns"),
        muted(format!(
            "Live state: {done}/4 tasks · day {} · radius {} · no network, no preference writes.",
            12 + app.showcase.day,
            radius_label(RADII[app.showcase.radius])
        )),
    ]
    .key("showcase-workspace")
    .gap(24., Axis::Vertical)
    .into()
}
