#![cfg(feature = "native")]
use volt_ui_kit::native::{
    Application, ApplicationRequest, CloseDecision, CloseReason, Command, CommandDispatch,
    CommandRegistry, Shortcut,
};
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
struct App {
    dirty: bool,
    saved: usize,
}
fn commands() -> CommandRegistry<App> {
    CommandRegistry::new()
        .command(
            Command::new("save", "Save document", |app: &mut App| {
                app.saved += 1;
                app.dirty = false;
            })
            .shortcut(Shortcut::primary('s'))
            .enabled_when(|app| app.dirty),
        )
        .command(
            Command::new("close", "Close window", |_: &mut App| {})
                .shortcut(Shortcut::primary('w'))
                .request_close(CloseReason::Window),
        )
}
#[test]
fn state_aware_commands_are_shared_with_custom_palettes_and_unknown_ids_are_inert() {
    let mut registry = commands();
    registry.validate().unwrap();
    let mut app = App::default();
    assert_eq!(
        registry.dispatch("save", &mut app),
        CommandDispatch::Disabled
    );
    assert_eq!(
        registry.dispatch("invented", &mut app),
        CommandDispatch::Unknown
    );
    assert_eq!(app.saved, 0);
    app.dirty = true;
    assert!(
        registry
            .entries(&app)
            .iter()
            .find(|c| c.id == "save")
            .unwrap()
            .enabled
    );
    assert_eq!(
        registry.dispatch("save", &mut app),
        CommandDispatch::Handled(None)
    );
    assert_eq!(app.saved, 1);
    assert!(!app.dirty);
    assert_eq!(
        registry.dispatch("close", &mut app),
        CommandDispatch::Handled(Some(CloseReason::Window))
    );
}
#[test]
fn duplicate_ids_shortcuts_reserved_editor_keys_and_oversized_registries_are_rejected() {
    for registry in [
        CommandRegistry::new()
            .command(Command::new("same", "One", |_: &mut ()| {}))
            .command(Command::new("same", "Two", |_| {})),
        CommandRegistry::new()
            .command(Command::new("one", "One", |_: &mut ()| {}).shortcut(Shortcut::primary('s')))
            .command(Command::new("two", "Two", |_| {}).shortcut(Shortcut::primary('S'))),
        CommandRegistry::new().command(Command::new("bad/id", "Bad", |_: &mut ()| {})),
        CommandRegistry::new()
            .command(Command::new("bad", "Bad", |_: &mut ()| {}).shortcut(Shortcut::primary('c'))),
        CommandRegistry::new()
            .command(Command::new("bad", "Bad", |_: &mut ()| {}).shortcut(Shortcut::primary('é'))),
    ] {
        assert!(registry.validate().is_err());
    }
    let mut registry = CommandRegistry::new();
    for n in 0..257 {
        registry = registry.command(Command::new(format!("id-{n}"), "Item", |_: &mut ()| {}));
    }
    assert!(registry.validate().is_err());
}
#[test]
fn close_hooks_and_request_senders_are_additive_and_validation_does_not_call_them() {
    let app = Application::new("Documents", App::default(), |_| text("Editor").into())
        .commands(commands())
        .on_close_request(|app, _| {
            if app.dirty {
                CloseDecision::KeepOpen
            } else {
                CloseDecision::Close
            }
        })
        .requests(|_, sender| {
            sender
                .send(ApplicationRequest::Command("save".into()))
                .unwrap();
        });
    assert!(app.validate().is_ok());
    let invalid = Application::new("Documents", (), |_| text("Editor").into())
        .commands(CommandRegistry::new().command(Command::new("bad/id", "Bad", |_| {})));
    assert!(invalid.validate().is_err());
}

#[test]
fn configuration_validation_never_invokes_views_callbacks_or_enabled_predicates() {
    let app = Application::new("App", (), |_| panic!("view during validation"))
        .commands(
            CommandRegistry::new().command(
                Command::new("save", "Save", |_| panic!("callback"))
                    .enabled_when(|_| panic!("predicate during validation")),
            ),
        )
        .on_close_request(|_, _| panic!("close during validation"));
    app.validate().unwrap();
}
