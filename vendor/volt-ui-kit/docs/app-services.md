# Application services

## Background work

Callbacks run on the UI thread, so do slow work elsewhere and send the result
back as a typed message:

```rust,no_run
use volt_ui_kit::{native::{Application, UiSender}, ui::prelude::*};

enum Message {
    Loaded(String),
}

#[derive(Default)]
struct App {
    sender: Option<UiSender<Message>>,
    result: String,
}

fn view(app: &App) -> Element<App> {
    vstack![
        text(&app.result),
        button("Load").key("load").on_click(|app: &mut App| {
            let sender = app.sender.clone().expect("installed before the first view");
            std::thread::spawn(move || {
                let data = "loaded on a worker thread".to_string(); // I/O goes here
                let _ = sender.send(Message::Loaded(data));
            });
        }),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Worker", App::default(), view)
        .messages(
            |app, sender| app.sender = Some(sender),
            |app, message| match message {
                Message::Loaded(data) => app.result = data,
            },
        )
        .run()
}
```

- Messages need `Send`; your state does not.
- `send` never blocks. The queue holds 256 messages; when it is full or the
  window has closed, `send` drops the message and returns `UiSendError::Full`
  or `Closed`. Decide whether to retry, drop or merge.
- The UI handles up to 64 messages per wake-up and calls `view` once per batch.
- Use any executor (threads, tokio, async-std); the kit starts none of its own.
- Tag long jobs with an ID so you can ignore results from cancelled work.

## Commands and shortcuts

A command registry gives actions an ID, a label, an optional shortcut and a
rule for when they are enabled. The same command can then run from a keyboard
shortcut, the macOS menu bar, a tray menu, your own buttons or a command
palette built from `registry.entries(&app)`.

```rust,no_run
use volt_ui_kit::{
    native::{
        Application, ApplicationRequest, CloseDecision, CloseReason, Command, CommandRegistry,
        Shortcut, UiSender,
    },
    ui::prelude::*,
};

#[derive(Default)]
struct App {
    draft: String,
    dirty: bool,
    closing: Option<CloseReason>,
    requests: Option<UiSender<ApplicationRequest>>,
}

fn commands() -> CommandRegistry<App> {
    CommandRegistry::new()
        .command(
            Command::new("save", "Save", |app: &mut App| app.dirty = false)
                .shortcut(Shortcut::primary('s'))
                .enabled_when(|app| app.dirty),
        )
        .command(
            Command::new("close", "Close window", |_: &mut App| {})
                .shortcut(Shortcut::primary('w'))
                .request_close(CloseReason::Window),
        )
}

fn view(app: &App) -> Element<App> {
    let editor = text_area(&app.draft).key("draft").on_change(|app: &mut App, text| {
        app.draft = text;
        app.dirty = true;
    });
    let Some(reason) = app.closing else {
        return editor.into();
    };
    zstack![
        editor,
        dialog(hstack![
            button("Cancel").key("cancel").on_click(|app: &mut App| app.closing = None),
            button("Discard").key("discard").on_click(move |app: &mut App| {
                app.dirty = false;
                if let Some(requests) = &app.requests {
                    let _ = requests.send(ApplicationRequest::Close(reason));
                }
            }),
        ].gap(8., Axis::Horizontal))
        .key("confirm")
        .title("Discard unsaved changes?")
        .open(true)
        .on_dismiss(|app: &mut App| app.closing = None),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Editor", App::default(), view)
        .commands(commands())
        .requests(|app, sender| app.requests = Some(sender))
        .on_close_request(|app, reason| {
            if app.dirty {
                app.closing = Some(reason);
                CloseDecision::KeepOpen
            } else {
                CloseDecision::Close
            }
        })
        .run()
}
```

- `Shortcut::primary('s')` is Cmd+S on macOS and Ctrl+S elsewhere; add
  `.shift(true)` or `.alt(true)`. Shortcuts are ignored while typing with an IME
  or while a modal is open, and clipboard and undo keys stay with text fields.
- Disabled commands never run, whichever way they are triggered.
- `ApplicationRequest::Command(id)` runs a command from your own UI;
  `ApplicationRequest::Close(reason)` asks to close, and the close guard is
  checked again.

### Guarding close

`on_close_request` decides whether the window may close. Return `KeepOpen`,
show your own Save / Discard / Cancel dialog, and request the close again when
the user decides. Save asynchronously with a background message and close when
the save succeeds.

The guard covers the window's close button, Cmd+W and, with `native-desktop`,
the owned Quit menu item. It cannot stop Force Quit, system shutdown or a crash.

## Files and dialogs

```rust,no_run
use volt_ui_kit::native::{
    file_dialog::{self, FileDialogOptions},
    files::{read_text_file, save_text_file, SavePolicy},
};
// Panels run on the main thread (inside a callback); `Ok(None)` means cancelled.
if let Some(path) = file_dialog::open_file(&FileDialogOptions::new("Open notes"))? {
    // Do the I/O on a worker thread, then report back with a message.
    let original = read_text_file(&path)?;
    save_text_file(&path, "Updated", SavePolicy::MatchContents(&original))?;
}
# Ok::<(), Box<dyn std::error::Error>>(())
```

- `open_file` and `save_file` (with `.suggested_name(..)`) show the native macOS
  panels and need `native-desktop`. Other platforms return `Unsupported` for now.
- `save_text_file` writes to a temporary file and moves it into place.
  `CreateNew` never overwrites; `MatchContents(original)` refuses to save if the
  file changed on disk since you read it; `ApprovedReplacement` overwrites.
- These helpers are for small UTF-8 text files (up to 8 KiB). Use `std::fs`
  directly for anything larger.

## Menus and tray

With `native-desktop` on macOS, add an application menu and a tray item. Their
items share IDs with your registered commands, so they enable, disable and run
exactly like shortcuts:

```rust,no_run
use volt_ui_kit::native::system::{DesktopCommand, DesktopConfig, DesktopMenu};
let desktop = DesktopConfig::new("My app").menu(DesktopMenu::new(
    "File",
    [DesktopCommand::new("save", "Save"), DesktopCommand::new("close", "Close window")],
));
// Application::new(..).commands(..).desktop(desktop, |_app, _unregistered_id| {})
```

The menu includes the standard About, Services, Hide and Quit items. A
`TrayConfig` adds a menu-bar icon (RGBA, up to 128×128) with its own commands.

## Notifications

With `native-notifications` on macOS:

```rust,no_run
use std::time::Duration;
use volt_ui_kit::native::notifications::{self, LocalNotification};
// Ask from a deliberate user action; the callback runs on an OS thread.
notifications::request_authorization(|decision| { let _ = decision; })?;
let note = LocalNotification::new("build.ready", "Build ready", "All checks passed.")
    .after(Duration::from_secs(5));
notifications::send(&note, |accepted| { let _ = accepted; })?;
notifications::remove("build.ready")?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Notifications work only from a bundled `.app` (see [Packaging](#packaging)).
"Accepted" means the OS took the request; Focus modes and user settings decide
whether it is shown. Forward the callback results to your UI with a
background message.

## The development loop

`cargo volt dev` rebuilds your project when a source or manifest changes and
restarts the app after a successful build. To keep your place across restarts,
save and restore state:

```rust,no_run
use volt_ui_kit::{native::Application, ui::prelude::*};
#[derive(Default)]
struct App { page: usize }
fn view(app: &App) -> Element<App> { text(format!("Page {}", app.page)).into() }
Application::new("Dev", App::default(), view)
    .dev_state(
        |app| app.page.to_string(),
        |app, saved| app.page = saved.parse().unwrap_or(0),
    )
    .theme_file("theme.json")
    .run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

- `dev_state` runs only under `cargo volt dev`; the window keeps its size and
  position too. Use any format for the saved string.
- `theme_file` reloads the palette whenever the file changes, without a rebuild.
- The dev loop runs your project's build scripts; use it on code you trust.

## Packaging

`cargo volt bundle` turns a release binary into a macOS `.app`:

```sh
cargo build --release
cargo volt bundle \
  --binary target/release/my-app \
  --output MyApp.app \
  --name "My App" \
  --identifier com.example.myapp \
  --version 0.1.0 \
  --minimum-system-version 11.0 \
  --icon assets/AppIcon.icns
```

It writes `Info.plist`, copies the files you pass with `--resource`, adds
privacy descriptions from `--usage KEY=TEXT`, checks that the binary links only
system libraries, and signs the bundle ad hoc for local use. It never
overwrites an existing path. Distribution to other Macs still needs your
Developer ID signature and Apple notarization. Run `cargo volt bundle --help`
for every option.
