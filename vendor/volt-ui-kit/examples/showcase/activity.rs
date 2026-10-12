use super::{style::*, App};
use crate::lab::jobs;
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let status = if app.job_running {
        format!("Preparing your workspace  ·  {}%", app.job_progress)
    } else if app.job_progress == 100 {
        "Workspace ready".into()
    } else {
        "Ready when you are".into()
    };
    let work = glass_card(app, [
        label("BACKGROUND / TYPED RUST MESSAGES").into(),
        title("Keep the conversation moving.").into(),
        muted("Prepare a local workspace without blocking the interface. Try the counter while work arrives.").into(),
        text(status).key("activity-job-status").font_size(15.).font_weight(500).into(),
        progress(app.job_progress as f32 / 100.).key("activity-progress").fill_width()
            .height(8.).background(ColorToken::Base300).rounded(Rounding::Full).into(),
        hstack![
            action(if app.job_running { "Restart" } else { "Prepare workspace" }, "activity-start", true, jobs::start).flex(1.),
            button("Cancel").key("activity-cancel").height(40.).width(84.)
                .rounded(10.).background(ColorToken::Base300).border_width(0.)
                .enabled(app.job_running).on_click(jobs::cancel),
        ].gap(10., Axis::Horizontal).into(),
        tooltip("Rust handles this click on the UI thread, even while the worker is active.",
            action(format!("Try a live click  ·  {}", app.count), "activity-counter", false, |app| {
                app.count += 1;
                app.record("Atelier: click handled while background work was available");
            }))
            .key("activity-counter-tooltip").into(),
        if app.job_running {
            loading("Receiving worker messages").font_size(12.).color(ColorToken::Muted).into()
        } else {
            muted("No active worker. No animation or idle polling.").font_size(12.).into()
        },
    ]).key("activity-work").flex(1.);
    let commands = menu(
        "Workspace actions",
        [
            menu_item(1usize, "Save a local draft"),
            menu_item(2, "Publish to service").enabled(false),
            menu_separator(),
            menu_item(3, "Clear live counter").danger(true),
        ],
    )
    .key("activity-menu")
    .fill_width()
    .open(app.menu_open)
    .height(40.)
    .fill_width()
    .font_size(12.)
    .rounded(10.)
    .on_open_change(|app: &mut App, open| app.menu_open = open)
    .on_action(|app: &mut App, action| {
        if action == 3 {
            app.count = 0;
        }
        if action == 1 {
            app.showcase.saved = true;
        }
        app.record(match action {
            3 => "Atelier: counter cleared",
            _ => "Atelier: draft saved locally",
        });
    });
    let inspect = action("Inspect the session", "activity-inspect", false, |app| {
        app.popover_open = !app.popover_open;
    });
    let context = context_menu(
        panel![
            hstack![mark(24.), title("Local workspace")].gap(10., Axis::Horizontal),
            muted("Right-click this surface for contextual actions.")
        ]
        .key("activity-context-surface")
        .padding(16., Edge::All)
        .gap(12., Axis::Vertical)
        .background(ColorToken::Base300)
        .rounded(14.),
        [
            menu_item(1usize, "Mark reviewed"),
            menu_separator(),
            menu_item(2, "Reset progress"),
        ],
    )
    .key("activity-context")
    .open(app.context_open)
    .position(app.context_position)
    .on_context_open(|app: &mut App, position| {
        app.context_position = position;
        app.context_open = true;
    })
    .on_open_change(|app: &mut App, open| app.context_open = open)
    .on_action(|app: &mut App, id| {
        if id == 2 {
            jobs::cancel(app);
            app.job_progress = 0;
        }
        app.record(if id == 1 {
            "Atelier: workspace reviewed"
        } else {
            "Atelier: progress reset"
        });
    });
    let actions = glass_card(
        app,
        [
            label("COMMANDS / CONTEXT").into(),
            title("Small actions, clear intent.").into(),
            muted("Menus, tooltips and anchored details share the same native input model.").into(),
            commands.into(),
            inspect.into(),
            context.into(),
            muted("Arrow keys skip disabled actions. Escape closes and returns focus.")
                .font_size(12.)
                .into(),
        ],
    )
    .key("activity-actions")
    .flex(1.);
    let notes = glass_card(app, [
        hstack![title("Working notes"), spacer(), pill("LOCAL", ColorToken::Base300, ColorToken::BaseContent)].into(),
        muted("Plain text, soft-wrapped. Paste a paragraph, move the caret and select across lines.").into(),
        text_area(&app.notes).key("activity-notes").fill_width().soft_wrap(true)
            .font_family("SF").font_size(14.).height(144.).rounded(12.)
            .on_change(|app: &mut App, value| app.notes = value).into(),
    ]).key("activity-notes-card");
    let inspector = popover(vstack![
        title("A bounded session"),
        muted("256 queued messages. At most 64 updates per native event-loop turn. State stays in Rust on the UI thread."),
        text(format!("Clicks: {}\nProgress: {}%", app.count, app.job_progress)).font_family("SF Mono").font_size(12.),
        action("Close details", "activity-inspector-close", true, |app| app.popover_open = false),
    ].gap(14., Axis::Vertical).padding(6., Edge::All))
        .key("activity-popover").anchor("activity-inspect").open(app.popover_open)
        .on_dismiss(|app: &mut App| app.popover_open = false);
    vstack![
        text("Work that stays responsive.")
            .font_size(34.)
            .font_weight(700),
        muted("Real callbacks and background state, presented in the Atelier design language."),
        adaptive([work.into(), actions.into()]),
        notes,
        inspector,
    ]
    .gap(20., Axis::Vertical)
    .into()
}
