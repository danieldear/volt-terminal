# Application commands and guarded closing

Included in v0.4.0; this is a **single-window slice**. The
multi-window runtime, persistence and complete OS termination lifecycle are still
pending. No new default dependency or unsafe code is introduced.

```text
                  Your Rust application
                  state + view + policies
                            |
            +---------------+----------------+
            |               |                |
       Custom button    Custom palette    Native menu
            |               |                |
       Bounded requests     |           Known menu IDs
            +---------------+----------------+
                            |
                   CommandRegistry<C>
               ID / enabled predicate / callback
                            |
          Exact logical Cmd/Ctrl shortcut also enters here
                            |
                   Optional close request
                            |
                    on_close_request
                     /            \
                  Close          KeepOpen
                    |               |
             close queues       Rust dialog:
             exit the host    Save / Discard / Cancel
```

## Declarative Rust usage

Commands do not dictate a sidebar, shell, palette or background. Define them in
Rust and build your own UI from their metadata:

```rust,ignore
fn commands() -> CommandRegistry<App> {
    CommandRegistry::new()
        .command(Command::new("save", "Save document", |app: &mut App| {
            app.save_to_your_backend(); // your own persistence and error handling
        })
        .shortcut(Shortcut::primary('s'))
        .enabled_when(|app| app.dirty && !app.saving))
        .command(Command::new("close", "Close window", |_: &mut App| {})
            .shortcut(Shortcut::primary('w'))
            .request_close(CloseReason::Window))
}

Application::new("My editor", App::default(), view)
    .commands(commands())
    .requests(|app, sender| app.requests = Some(sender))
    .on_close_request(|app, reason| {
        if app.dirty || app.saving {
            app.close_prompt = Some(reason);
            CloseDecision::KeepOpen
        } else {
            CloseDecision::Close
        }
    })
    .run()?;
```

A button still uses an ordinary typed Rust callback:

```rust,ignore
button("Save")
    .key("save")
    .enabled(app.dirty && !app.saving)
    .on_click(|app: &mut App| {
        if let Some(sender) = &app.requests {
            // Handle Full/Closed; never pretend that enqueue equals execution.
            if let Err(error) = sender.send(ApplicationRequest::Command("save".into())) {
                app.show_request_error(error); // developer-owned error UI
            }
        }
    })
```

`registry.entries(&app)` returns IDs, labels, shortcut labels and current enabled
state for a custom palette or action bar. `dispatch(id, &mut app)` is also usable
without a native window, but **only runs callbacks and returns a close effect**;
it does not close an OS window. Native host dispatch checks enablement again,
so a stale button/menu cannot execute a now-disabled command. Registered IDs take
precedence over the older `Application::desktop(config, callback)` callback.
Unknown IDs do nothing.

## Close decisions and asynchronous saving

`CloseReason::Window` covers the native close button and explicit window-close
requests. `CloseReason::Quit` covers explicit application requests and the owned
macOS Quit menu when both a close hook and `native-desktop` are configured. In the
current single-window host, either accepted reason exits the application.
Without a hook, the old close-immediately behavior is unchanged.

`KeepOpen` does not close worker/request queues. Update app state to show a dialog;
Cancel leaves the document and window intact. Save/Discard should explicitly
update developer-owned state, then enqueue `ApplicationRequest::Close(reason)`.
The guard is evaluated **again**, not bypassed by the confirmation dialog.
Repeated close requests are allowed; developers should make their dialog policy
idempotent and preserve the intended reason.

Do not block the UI thread on disk/network saving. Start a worker through the
existing typed messages API, set `saving`, and keep the window open. On successful
completion, update dirty state and request closing. On failure, keep the dirty
state and show an error. There is no toolkit autosave, hidden filesystem write,
serialization of Rust state, or forced discard.

### macOS boundary: not universal quit protection

- With an owned `native-desktop` menu and a close hook, Quit/Cmd+Q is a custom menu
  command routed through the hook rather than a predefined direct termination.
- Without that menu, the default system Quit may terminate without a window-close
  event. **A window-close hook alone is not unsaved-document protection for every
  quit path.**
- Dock/system termination, shutdown, signals, crashes and Force Quit are **not
  vetoed**. No AppKit delegate replacement or private API is used. A later native
  termination-lifecycle bridge is required for broader OS coverage.
- Desktop integration remains macOS-first, opt-in and single-host/process. Native
  menu/tray command IDs are shared; live command enabled states are reflected in
  their retained items. Static menu `enabled(false)` remains an additional gate.

Pinned source semantics were checked for winit 0.30.13 (`windowShouldClose` queues
`CloseRequested`) and muda 0.19.3 (predefined Quit invokes termination). Logical
native accelerators use [muda's KeyAccelerator](https://docs.rs/muda/0.19.3/muda/accelerator/struct.KeyAccelerator.html),
not physical US-keyboard codes.

## Shortcut and request boundaries

- Maximum 256 commands; bounded ASCII IDs (64 bytes), labels (256 bytes), unique
  IDs and normalized shortcuts. Definition validation is cached; current-state
  predicates are not cached. Invalid registries cannot dispatch.
- Shortcuts currently support ASCII letters/digits with Cmd on macOS and Ctrl
  elsewhere, plus optional Shift/Alt. Exact modifiers and logical character
  matching are required; host keyboard auto-repeat is consumed without running
  callbacks. Native-menu accelerator delivery/repeat behavior still requires
  runtime acceptance; menu events do not expose winit's repeat flag.
- Editor copy/cut/paste/select-all/undo/redo and the system Quit chord are reserved.
  IME composition and active modal/menu scope suppress global command shortcuts
  and native command-menu dispatch. Default typing/focus traversal remain intact.
- Explicit requests from your own Rust UI can run a command inside a dialog;
  this is how Save/Discard confirmations work. It is trusted application code,
  **not** an agent sandbox or permission boundary. Never route untrusted network
  or model instructions to this sender without your own authorization policy.
- Request queues share the existing nonblocking 256-capacity, 64-per-turn,
  event-driven/coalesced queue implementation. Commands in a request batch compose
  once. Accepted closing/fatal errors/drop reject late sends; veto does not.
- Native accelerators are installed only for registered commands actually present
  in the owned application menu. Tray commands share IDs without registering
  keyboard shortcuts twice. Unsupported platforms still report capability errors.

## Catalog acceptance

```sh
cargo run --release --features native-gpu,native-desktop --example catalog -- \
  --page application-lifecycle
```

The page includes a SF Mono draft, dirty badge, command rows, Cmd/Ctrl+S/W/K,
command palette and guarded close dialog. **Save is in-memory only** and no files
are written; the page and dialog say so. Headless snapshots do not pretend to
send native requests or close a window. Existing background-task messages remain
available alongside the separate application-request queue.

Tests cover validation, disabled/unknown callbacks, normalized/exact shortcut
matching, modal/IME scope, registry/legacy command precedence, queue bounds and
close re-evaluation. Runtime receipts and remaining acceptance gates belong in
[application delivery plan](APPLICATION_DEVELOPMENT.md), not inferred from unit
or offscreen tests.
