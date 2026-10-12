//! Developer-owned desktop service policy; callbacks never mutate app state.
use super::{
    pages::{Doc, Section},
    App, Message,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use volt_ui_kit::{
    native::{
        notifications::{self, Authorization, LocalNotification, NotificationError},
        UiSender,
    },
    ui::prelude::*,
};
const ID: &str = "volt.catalog.test";
#[derive(Debug)]
enum Completion {
    Authorization(Result<Authorization, NotificationError>),
    Submitted(Result<(), NotificationError>, Duration),
}
type Mailbox = Arc<Mutex<Option<(u64, Completion)>>>;
pub struct State {
    pub authorization: Option<Authorization>,
    pub busy: bool,
    pub cancellation_test: bool,
    pub title: String,
    pub body: String,
    pub status: String,
    pub error: Option<String>,
    generation: u64,
    mailbox: Mailbox,
}
impl Default for State {
    fn default() -> Self {
        Self {
            authorization: None,
            busy: false,
            cancellation_test: false,
            title: "Volt desktop test".into(),
            body: "A local notification from the Catalog. No network or personal content.".into(),
            status:
                "No permission requested. Authorization is explicit and belongs to this bundle ID."
                    .into(),
            error: None,
            generation: 0,
            mailbox: Arc::default(),
        }
    }
}
fn begin(app: &mut App) -> Option<(u64, Mailbox, UiSender<Message>)> {
    if app.services.busy {
        return None;
    }
    let Some(sender) = app.sender.clone() else {
        app.services.error =
            Some("Run the native Catalog; snapshots never call OS services.".into());
        return None;
    };
    app.services.generation = app.services.generation.wrapping_add(1);
    app.services.busy = true;
    app.services.error = None;
    app.services.mailbox = Arc::default();
    Some((
        app.services.generation,
        app.services.mailbox.clone(),
        sender,
    ))
}
fn complete(generation: u64, mailbox: Mailbox, sender: UiSender<Message>, completion: Completion) {
    *mailbox.lock().unwrap_or_else(|p| p.into_inner()) = Some((generation, completion));
    // One bounded mailbox. Full means another Catalog update is already queued;
    // every update drains this slot. Never block an OS callback or add idle polling.
    let _ = sender.send(Message::NotificationWake);
}
fn failed(app: &mut App, result: Result<(), NotificationError>) {
    if let Err(error) = result {
        app.services.busy = false;
        app.services.status = "Operation was not started; see the error below.".into();
        app.services.error = Some(error.to_string());
    }
}
pub fn request_authorization(app: &mut App) {
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    app.services.status = "Waiting for an OS authorization decision…".into();
    let result = notifications::request_authorization(move |r| {
        complete(generation, mailbox, sender, Completion::Authorization(r))
    });
    failed(app, result);
}
fn notification_plan(app: &App) -> (LocalNotification, Duration) {
    let delay = Duration::from_secs(if app.services.cancellation_test {
        60
    } else {
        8
    });
    (
        LocalNotification::new(ID, &app.services.title, &app.services.body).after(delay),
        delay,
    )
}
pub fn send(app: &mut App) {
    if app.services.authorization != Some(Authorization::Granted) {
        app.services.error =
            Some("Request alert permission explicitly before sending this example.".into());
        return;
    }
    let Some((generation, mailbox, sender)) = begin(app) else {
        return;
    };
    let (notification, delay) = notification_plan(app);
    app.services.status = format!(
        "Submitting a silent notification for {} seconds from now…",
        delay.as_secs()
    );
    let result = notifications::send(&notification, move |r| {
        complete(generation, mailbox, sender, Completion::Submitted(r, delay))
    });
    failed(app, result);
}
pub fn remove(app: &mut App) {
    if app.services.busy {
        return;
    }
    if app.sender.is_none() {
        app.services.error =
            Some("Run the native Catalog; snapshots never call OS services.".into());
        return;
    }
    app.services.error = None;
    match notifications::remove(ID) {
        Ok(()) => app.services.status =
            "Removal requested for this ID (pending and delivered); no visual-completion receipt."
                .into(),
        Err(e) => app.services.error = Some(e.to_string()),
    }
}
pub fn reset(app: &mut App) {
    if app.services.busy {
        return; // Reset cannot cancel an OS operation or bypass add/removal serialization.
    }
    app.services.error = None;
    // Cancels only this UI witness, not an OS authorization or submitted request.
    app.services.generation = app.services.generation.wrapping_add(1);
    app.services.mailbox = Arc::default();
    app.services.busy = false;
    app.services.status =
        "Witness reset; OS permission and submitted requests were not changed.".into();
}
pub fn drain(app: &mut App) {
    let completion = app
        .services
        .mailbox
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .take();
    let Some((generation, completion)) = completion else {
        return;
    };
    if generation != app.services.generation || !app.services.busy {
        return;
    }
    app.services.busy = false;
    match completion {
        Completion::Authorization(Ok(value)) => {
            app.services.authorization = Some(value);
            app.services.status = match value {
                Authorization::Granted =>
                    "Alert authorization granted at request time; settings/Focus can still suppress display.",
                Authorization::Denied =>
                    "Alert authorization denied. Change it yourself in System Settings if desired; nothing was sent.",
            }.into();
        }
        Completion::Submitted(Ok(()), delay) => {
            app.services.status = format!("OS accepted the request ({} seconds requested delay). This is NOT a delivery receipt. For pending cancellation, remove now, then hide Catalog and check Notification Center after the delay. Foreground banners/actions are not implemented.", delay.as_secs());
        }
        Completion::Authorization(Err(e)) | Completion::Submitted(Err(e), _) => {
            app.services.status = "OS operation failed; see the error below.".into();
            app.services.error = Some(e.to_string());
        }
    }
}

pub fn can_send(app: &App) -> bool {
    notifications::capabilities().local_requests
        && !app.services.busy
        && app.services.authorization == Some(Authorization::Granted)
}
pub fn doc(app: &App) -> Doc {
    let caps = notifications::capabilities();
    Doc {sections:vec![Section {
        id:"desktop-notifications",title:"Local notifications",note:"An explicit, opt-in OS service—not an in-app toast. No permission or notification is requested on launch.",
        demo:vstack![
            hstack![badge(if caps.local_requests {"macOS adapter compiled"} else {"Adapter unavailable"}),
                badge(match app.services.authorization {Some(Authorization::Granted)=>"Last decision: granted",Some(Authorization::Denied)=>"Last decision: denied",None=>"Permission not requested"})],
            text(ID).font_family("SF Mono").font_size(12.).color(ColorToken::Muted),
            text_field(&app.services.title).key("notification-title").accessible_label("Local notification title").read_only(app.services.busy)
                .on_change(|app:&mut App,v|app.services.title=v),
            text_area(&app.services.body).key("notification-body").accessible_label("Local notification body").height(90.).read_only(app.services.busy)
                .on_change(|app:&mut App,v|app.services.body=v),
            checkbox("Cancellation test: use a 60-second delay", app.services.cancellation_test)
                .key("notification-cancellation-test").enabled(!app.services.busy)
                .on_change(|app:&mut App,v|app.services.cancellation_test=v),
            hstack![
                button("Request alert permission").key("notification-authorize").enabled(caps.authorization&&!app.services.busy).on_click(request_authorization),
                button(if app.services.cancellation_test { "Send in 60 seconds" } else { "Send in 8 seconds" }).key("notification-send").enabled(caps.local_requests&&!app.services.busy&&app.services.authorization==Some(Authorization::Granted)).on_click(send),
            ].gap(8.,Axis::Horizontal),
            hstack![button("Remove test notification").key("notification-remove").variant(ButtonVariant::Outline).enabled(caps.removal&&!app.services.busy).on_click(remove),
                button("Reset witness").key("notification-reset").variant(ButtonVariant::Ghost).enabled(!app.services.busy).on_click(reset)].gap(8.,Axis::Horizontal),
            text(&app.services.status).font_size(13.).color(ColorToken::Muted),
            text(app.services.error.as_deref().unwrap_or("")).font_size(13.).color(ColorToken::Error),
            text("Package with native-notifications and an explicit .app ID. Missing bundle, unsupported build and native errors stay visible. Permission is not a live setting; this slice does not supply sound, badge, action-click callbacks or foreground delegates.").font_size(12.).color(ColorToken::Muted),
        ].width(620.).gap(14.,Axis::Vertical).into(),
        code:r#"// Build with native-notifications, then package the executable.
// Only a deliberate user action calls request_authorization.
notifications::request_authorization(move |decision| {
    // Marshal into bounded UiSender; never mutate UI on this OS thread.
})?;
let note = LocalNotification::new("build.ready", "Build ready", "All checks passed.")
    .after(Duration::from_secs(8));
notifications::send(&note, move |accepted| { /* Accepted != displayed */ })?;
// A later Remove action (after this callback) may remove "build.ready"."#,
    },Section {
        id:"desktop-tray",title:"One command registry, multiple surfaces",note:"The optional native-desktop tray reuses these command IDs. Menu/toolbar/palette/tray commands all pass current enablement and modal guards.",
        demo:vstack![text(format!("Command executions: {}",app.command_probe)).font_size(24.).font_weight(700),
            button("Count command execution").key("desktop-command-probe").on_click(|app:&mut App|super::lifecycle::request(app,volt_ui_kit::native::ApplicationRequest::Command("command-probe".into()))),
            text("Find the V-shaped Volt Catalog status item in the macOS menu bar. Its Count command execution item increments this same witness. It does not promise window restore, background-only or multi-window behavior.").font_size(13.).color(ColorToken::Muted),
        ].width(620.).gap(14.,Axis::Vertical).into(),
        code:r#"DesktopConfig::new("My app")
    .tray(TrayConfig { tooltip: "My app".into(), rgba, width: 18, height: 18,
        template: true, commands: vec![
            DesktopCommand::new("command-probe", "Count command execution"),
        ] })
// Register the same ID in CommandRegistry; don't duplicate callbacks."#,
    }],api:vec![
        ("notifications::capabilities()","Adapter availability plus explicit unsupported actions/settings/foreground capabilities."),
        ("request_authorization(callback)","Explicit alert-only decision, no automatic prompting; OS-thread completion."),
        ("LocalNotification::new(id,title,body)","Bounded stable ID and UTF-8 content; no truncation or arbitrary attachments."),
        ("notification.after(Duration)","One-shot 1 second–24 hours, silent local notification."),
        ("notifications::send / remove","Accepted-not-displayed callback; removal has no completion receipt."),
    ]}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_plan_is_explicit_and_keeps_the_same_removable_id() {
        let mut app = App::new();
        assert!(!app.services.cancellation_test);
        let (normal, delay) = notification_plan(&app);
        assert_eq!(delay, Duration::from_secs(8));
        assert_eq!(normal.id(), ID);
        assert!(normal.validate().is_ok());
        app.services.cancellation_test = true;
        let (pending, delay) = notification_plan(&app);
        assert_eq!(delay, Duration::from_secs(60));
        assert_eq!(pending.id(), ID);
        assert!(pending.validate().is_ok());
        app.services.title.clear();
        assert!(notification_plan(&app).0.validate().is_err());
    }
    #[test]
    fn submission_receipt_retains_its_delay_even_if_controls_change() {
        let mut app = App::new();
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() =
            Some((0, Completion::Submitted(Ok(()), Duration::from_secs(60))));
        drain(&mut app);
        assert!(app.services.status.contains("60 seconds"));
        assert!(app.services.status.contains("NOT a delivery receipt"));
        assert!(!app.services.busy);
    }
    #[test]
    fn late_results_and_reset_never_change_newer_permission_witness() {
        let mut app = App::new();
        app.services.busy = true;
        app.services.generation = 2;
        *app.services.mailbox.lock().unwrap() =
            Some((1, Completion::Authorization(Ok(Authorization::Granted))));
        drain(&mut app);
        assert_eq!(app.services.authorization, None);
        assert!(app.services.busy);
        reset(&mut app);
        assert!(app.services.busy);
        assert_eq!(app.services.generation, 2);
        *app.services.mailbox.lock().unwrap() =
            Some((2, Completion::Authorization(Ok(Authorization::Denied))));
        drain(&mut app);
        assert_eq!(app.services.authorization, Some(Authorization::Denied));
        assert!(!app.services.busy);
        reset(&mut app);
        assert_eq!(app.services.authorization, Some(Authorization::Denied));
    }
    #[test]
    fn shared_tray_commands_recheck_permission_and_busy_state() {
        let mut app = App::new();
        let registry = super::super::lifecycle::commands();
        let enabled = |app: &App, id: &str| {
            registry
                .entries(app)
                .into_iter()
                .find(|c| c.id == id)
                .unwrap()
                .enabled
        };
        assert!(!enabled(&app, "notification-send"));
        app.services.authorization = Some(Authorization::Denied);
        assert!(!enabled(&app, "notification-send"));
        app.services.authorization = Some(Authorization::Granted);
        assert_eq!(
            enabled(&app, "notification-send"),
            notifications::capabilities().local_requests
        );
        app.services.busy = true;
        assert!(!enabled(&app, "notification-send"));
        assert!(!enabled(&app, "notification-remove"));
        reset(&mut app);
        assert!(app.services.busy); // Cannot bypass add/removal serialization.
    }
    #[test]
    fn native_failure_is_visible_and_never_reports_delivery() {
        let mut app = App::new();
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() = Some((
            0,
            Completion::Submitted(
                Err(NotificationError::NativeFailure),
                Duration::from_secs(8),
            ),
        ));
        drain(&mut app);
        assert!(app.services.error.is_some());
        assert!(!app.services.busy);
        app.services.busy = true;
        *app.services.mailbox.lock().unwrap() =
            Some((0, Completion::Submitted(Ok(()), Duration::from_secs(8))));
        drain(&mut app);
        assert!(app.services.status.contains("NOT a delivery receipt"));
        let mut headless = App::new();
        request_authorization(&mut headless);
        assert!(headless
            .services
            .error
            .as_deref()
            .unwrap()
            .contains("native Catalog"));
        remove(&mut headless);
        assert!(headless
            .services
            .error
            .as_deref()
            .unwrap()
            .contains("native Catalog"));
        // No sender: pure headless errors, no OS service calls.
    }
}
