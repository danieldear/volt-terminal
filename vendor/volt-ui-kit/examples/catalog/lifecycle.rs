//! Application policies are Rust-owned; the sample saves only an in-memory draft.
use super::{
    pages::{Doc, Section},
    App,
};
use volt_ui_kit::{
    native::{ApplicationRequest, CloseDecision, CloseReason, Command, CommandRegistry, Shortcut},
    ui::{prelude::*, Element as El},
};

pub fn dirty(app: &App) -> bool {
    app.document_text != app.saved_document
}
pub fn save(app: &mut App) {
    app.saved_document.clone_from(&app.document_text);
    app.notify("Draft saved in memory only; no files were written");
}
pub fn commands() -> CommandRegistry<App> {
    CommandRegistry::new()
        .command(Command::new(
            "desktop-services",
            "Show desktop services page",
            |app: &mut App| app.go(super::Page::DesktopServices),
        ))
        .command(
            Command::new(
                "notification-send",
                "Send test notification",
                super::desktop_services::send,
            )
            .enabled_when(super::desktop_services::can_send),
        )
        .command(
            Command::new(
                "notification-remove",
                "Remove test notification",
                super::desktop_services::remove,
            )
            .enabled_when(|app| {
                !app.services.busy && volt_ui_kit::native::notifications::capabilities().removal
            }),
        )
        .command(
            Command::new("save-draft", "Save active draft", |app: &mut App| {
                if app.page == super::Page::Documents {
                    super::documents::save(app);
                } else {
                    save(app);
                }
            })
            .shortcut(Shortcut::primary('s'))
            .enabled_when(|app| {
                if app.page == super::Page::Documents {
                    !app.documents.busy && (app.documents.dirty() || app.documents.path.is_none())
                } else {
                    dirty(app)
                }
            }),
        )
        .command(
            Command::new("open-file", "Open notes file…", super::documents::open)
                .shortcut(Shortcut::primary('o'))
                .enabled_when(|app| !app.documents.busy),
        )
        .command(
            Command::new("save-file", "Save notes file", super::documents::save).enabled_when(
                |app| {
                    !app.documents.busy && (app.documents.dirty() || app.documents.path.is_none())
                },
            ),
        )
        .command(
            Command::new(
                "save-as-file",
                "Save notes file as…",
                super::documents::save_as,
            )
            .shortcut(Shortcut::primary('s').shift(true))
            .enabled_when(|app| !app.documents.busy),
        )
        .command(
            Command::new(
                "command-probe",
                "Count command execution",
                |app: &mut App| app.command_probe = app.command_probe.saturating_add(1),
            )
            .shortcut(Shortcut::primary('p').shift(true)),
        )
        .command(
            Command::new(
                "command-palette",
                "Open command palette",
                |app: &mut App| {
                    app.app_commands.open = true;
                    app.app_commands.query.clear();
                },
            )
            .shortcut(Shortcut::primary('k')),
        )
        .command(
            Command::new("close-window", "Close window", |_: &mut App| {})
                .shortcut(Shortcut::primary('w'))
                .request_close(CloseReason::Window),
        )
}
pub fn request(app: &mut App, request: ApplicationRequest) {
    let result = app.requests.as_ref().map(|sender| sender.send(request));
    match result {
        Some(Ok(())) => {}
        Some(Err(error)) => app.notify(format!("Application request not queued: {error}")),
        None => app.notify("Application requests run in the native Catalog window only"),
    }
}
pub fn close_request(app: &mut App, reason: CloseReason) -> CloseDecision {
    if super::documents::close_guard(app, reason) {
        return CloseDecision::KeepOpen;
    }
    if dirty(app) {
        app.app_commands.open = false;
        app.palette.open = false;
        app.close_prompt = Some(reason);
        CloseDecision::KeepOpen
    } else {
        CloseDecision::Close
    }
}
fn confirm(app: &mut App, save_first: bool) {
    let Some(reason) = app.close_prompt else {
        return;
    };
    if save_first {
        save(app);
    } else {
        app.document_text.clone_from(&app.saved_document);
    }
    app.close_prompt = None;
    request(app, ApplicationRequest::Close(reason));
}
pub fn doc(app: &App) -> Doc {
    let registry = commands();
    let mut rows: Vec<El<App>> = vec![];
    for command in registry.entries(app) {
        let id = command.id.to_owned();
        rows.push(
            hstack![
                text(command.label).font_size(13.).flex(1.),
                text(command.shortcut.map(Shortcut::label).unwrap_or_default())
                    .font_family("SF Mono")
                    .font_size(12.)
                    .color(ColorToken::Muted),
                button("Run")
                    .key(format!("command-{id}"))
                    .enabled(command.enabled)
                    .variant(ButtonVariant::Outline)
                    .size(ControlSize::Small)
                    .on_click(move |app: &mut App| request(
                        app,
                        ApplicationRequest::Command(id.clone())
                    )),
            ]
            .gap(12., Axis::Horizontal)
            .align(Align::Center)
            .into(),
        );
    }
    Doc {
        sections: vec![Section {
            id: "application-document", title: "A guarded document",
            note: "Edit the draft, use Save or Cmd/Ctrl+S, then request closing. Dirty work opens Save / Discard / Cancel. Saving here is in memory, not disk persistence.",
            demo: vstack![
                hstack![text("notes.volt").font_family("SF Mono").font_size(13.), spacer(),
                    badge(if dirty(app) { "Unsaved" } else { "Saved in memory" }).background(if dirty(app) { ColorToken::Warning.alpha(15) } else { ColorToken::Success.alpha(15) }).color(if dirty(app) { ColorToken::Warning } else { ColorToken::Success })],
                text_area(&app.document_text).key("application-draft").accessible_label("Example document draft")
                    .height(150.).soft_wrap(true).font_family("SF Mono").font_size(13.)
                    .on_change(|app: &mut App, value| app.document_text = value),
                vstack(rows).gap(12., Axis::Vertical),
                text(format!("Command executions: {}", app.command_probe)).font_family("SF Mono").font_size(12.),
                text(if cfg!(all(target_os = "macos", feature = "native-desktop")) {
                    "Native close and the owned Quit menu are guarded. Forced OS/Dock termination is not intercepted."
                } else if cfg!(target_os = "macos") {
                    "Window close is guarded. Enable native-desktop for the owned guarded Quit menu; default system Quit is not intercepted."
                } else {
                    "Window close and explicit quit requests are guarded; forced OS termination is not intercepted."
                })
                    .font_size(12.).color(ColorToken::Muted),
            ].width(600.).gap(16., Axis::Vertical).into(),
            code: r#"Application::new("Documents", app, view)
    .commands(commands())
    .requests(|app, sender| app.requests = Some(sender))
    .on_close_request(|app, reason| {
        if app.dirty() {
            app.close_prompt = Some(reason);
            CloseDecision::KeepOpen
        } else { CloseDecision::Close }
    })
    .run()?;
// Save / Discard: update app state, then enqueue Close(reason).
// Cancel: dismiss the dialog without closing or clearing the draft."#,
        }],
        api: vec![
            ("CommandRegistry::new().command(...)", "Up to 256 validated, uniquely named Rust commands."),
            ("Command::enabled_when(predicate)", "Rechecked against current state before every execution."),
            ("Shortcut::primary('s')", "Exact Cmd/Ctrl logical letter/digit chord; optional shift/alt."),
            ("registry.entries(&app)", "Metadata for your own palette, buttons or other UI."),
            ("Application::on_close_request(callback)", "Close or KeepOpen; the developer owns dirty state and dialogs."),
            ("Application::requests(initialize)", "Bounded, event-driven commands and close requests; no idle polling."),
        ],
    }
}
pub fn overlays(app: &App) -> Vec<El<App>> {
    // Registered commands that may run now, with their shortcut hints; the
    // toolkit's command palette does the search, grouping and Enter-to-run.
    let commands = commands()
        .entries(app)
        .into_iter()
        .filter(|c| c.enabled)
        .map(|c| {
            let mut entry = command(c.id.to_owned(), c.label).group("Application");
            if let Some(shortcut) = c.shortcut {
                entry = entry.shortcut(shortcut.label());
            }
            entry
        })
        .collect::<Vec<_>>();
    vec![
        command_palette(commands, &app.app_commands)
            .key("application-palette")
            .placeholder("Find a command…")
            .state(|app: &mut App| &mut app.app_commands)
            .on_run(|app: &mut App, id: String| request(app, ApplicationRequest::Command(id)))
            .into(),
        dialog(vstack![
            text("The example draft has unsaved changes. Saving keeps them in memory only; quitting does not persist them to disk.").font_size(14.),
            hstack![spacer(),
                button("Cancel").key("application-close-cancel").variant(ButtonVariant::Outline)
                    .on_click(|app: &mut App| app.close_prompt = None),
                button("Discard & close").key("application-close-discard").variant(ButtonVariant::Danger)
                    .on_click(|app: &mut App| confirm(app, false)),
                button("Save & close").key("application-close-save").variant(ButtonVariant::Primary)
                    .on_click(|app: &mut App| confirm(app, true)),
            ].gap(8., Axis::Horizontal),
        ].gap(20., Axis::Vertical)).key("application-close-dialog").title("Save your changes?")
            .open(app.close_prompt.is_some()).width(520.).rounded(12.).background(ColorToken::Base100)
            .backdrop_dismiss(false).on_dismiss(|app: &mut App| app.close_prompt = None).into(),
    ]
}
#[cfg(all(feature = "native-desktop", target_os = "macos"))]
pub fn desktop() -> volt_ui_kit::native::system::DesktopConfig {
    use volt_ui_kit::native::system::{DesktopCommand, DesktopConfig, DesktopMenu, TrayConfig};
    let rgba = (0i32..18 * 18)
        .flat_map(|n| {
            let (x, y) = (n % 18, n / 18);
            let stroke = (4..=14).contains(&y)
                && ((x - (4 + (y - 4) / 2)).abs() <= 1 || (x - (14 - (y - 4) / 2)).abs() <= 1);
            [255, 255, 255, if stroke { 255 } else { 0 }]
        })
        .collect();
    DesktopConfig::new("Volt UI")
        .tray(TrayConfig {
            tooltip: "Volt Catalog desktop services".into(),
            rgba,
            width: 18,
            height: 18,
            template: true,
            commands: vec![
                DesktopCommand::new("desktop-services", "Show desktop services page"),
                DesktopCommand::new("command-probe", "Count command execution"),
                DesktopCommand::new("notification-send", "Send test notification"),
                DesktopCommand::new("notification-remove", "Remove test notification"),
            ],
        })
        .menu(DesktopMenu::new(
            "File",
            [
                DesktopCommand::new("save-draft", "Save active draft"),
                DesktopCommand::new("open-file", "Open notes file…"),
                DesktopCommand::new("save-file", "Save notes file"),
                DesktopCommand::new("save-as-file", "Save notes file as…"),
                DesktopCommand::new("command-probe", "Count command execution"),
                DesktopCommand::new("command-palette", "Open command palette"),
                DesktopCommand::new("close-window", "Close window"),
            ],
        ))
}
