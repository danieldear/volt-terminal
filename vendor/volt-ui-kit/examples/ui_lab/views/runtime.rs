use super::*;
use crate::lab::jobs;

pub fn view(app: &App) -> Element<App> {
    let status = if app.job_running {
        format!("Working: {}%", app.job_progress)
    } else if app.job_progress == 100 {
        "Complete".into()
    } else {
        "Idle / cancelled".into()
    };
    vstack![
        specimen("Event-driven background work", [
            description("A worker sends typed messages; the native event loop wakes only when work arrives. Click and cancel while the job runs."),
            hstack![
                button("Start job").key("job-start").height(38.).on_click(jobs::start),
                button("Cancel").key("job-cancel").height(38.).enabled(app.job_running).on_click(jobs::cancel),
                tooltip("This callback stays on the UI thread; it works while the job is running.",
                    button(format!("Clicks: {}",app.count)).key("job-counter").height(38.)
                        .on_click(|app:&mut App|{app.count+=1;app.record("Click handled during background work");}))
                    .key("job-counter-help"),
            ].gap(10.,Axis::Horizontal).into(),
            progress(app.job_progress as f32/100.).key("job-progress").width(320.).height(10.).into(),
            text(status).key("job-status").font_family("monospace").font_size(12.).into(),
            if app.job_running {loading("Worker active — cancellation is application-owned").into()}
                else {description("No worker, no spinner, no idle polling.")},
        ]),
        specimen("Menus, context actions & anchored cards", [
            description("Dropdown: arrow keys skip disabled rows; Enter commits; Escape/outside dismiss. Right-click the card for a context menu."),
            hstack![
                menu("Actions",[menu_item(1usize,"Save specimen"),menu_item(2,"Unavailable").enabled(false),menu_separator(),menu_item(3,"Clear counter").danger(true)])
                    .key("runtime-menu").open(app.menu_open)
                    .on_open_change(|app:&mut App,open|app.menu_open=open)
                    .on_action(|app:&mut App,id|{if id==3 {app.count=0;}app.record(format!("Menu action: {id}"));}),
                button("Inspect runtime").key("runtime-inspect").height(36.).on_click(|app:&mut App|app.popover_open = !app.popover_open),
            ].gap(12.,Axis::Horizontal).into(),
            context_menu(panel([text("Right-click here. This target is a composed panel, not a special native widget.").font_size(13.).into()])
                    .height(76.).padding(14.,Edge::All).background(ColorToken::Base300).rounded(10),
                [menu_item(1usize,"Copy demo label"),menu_separator(),menu_item(2,"Mark inspected")])
                .key("runtime-context").open(app.context_open).position(app.context_position)
                .on_context_open(|app:&mut App,point|{app.context_position=point;app.context_open=true;})
                .on_open_change(|app:&mut App,open|app.context_open=open)
                .on_action(|app:&mut App,id|app.record(format!("Context action {id}; position {:?}",app.context_position))).into(),
            popover(vstack![heading("Bounded, typed, local"),description("256 queued messages. 64 updates per wake. One composition per batch."),
                button("Close inspector").key("runtime-inspector-close").on_click(|app:&mut App|app.popover_open=false)])
                .key("runtime-popover").anchor("runtime-inspect").open(app.popover_open)
                .on_dismiss(|app:&mut App|app.popover_open=false).into(),
        ]),
        specimen("Soft-wrapped plain text", [
            description("Opt-in wrap shares metrics between drawing, caret, hit testing and selection. Explicit-line mode is still the default."),
            text_area(&app.notes).key("runtime-wrapped-editor").soft_wrap(true).height(144.).width(360.)
                .font_family("system").on_change(|app:&mut App,value|app.notes=value).into(),
        ]),
    ].gap(18.,Axis::Vertical).into()
}
