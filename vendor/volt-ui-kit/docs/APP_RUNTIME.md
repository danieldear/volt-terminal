# Rust background work and state updates

`native::run_ui_with_messages` and `run_ui_with_messages_and_theme` add typed worker results without changing the existing host APIs or requiring an executor dependency.

```rust,no_run
use volt_ui_kit::{native::{NativeOptions, UiSender, run_ui_with_messages}, ui::prelude::*};
#[derive(Default)]
struct App { sender: Option<UiSender<Message>>, answer: String }
enum Message { Finished(String) }
fn view(app: &App) -> Element<App> {
    vstack![
        text(&app.answer),
        button("Compute").on_click(|app: &mut App| {
            let sender = app.sender.as_ref().unwrap().clone();
            std::thread::spawn(move || {
                // Do I/O or expensive work here, not inside view/on_click/update.
                let result = "Finished on a worker".to_string();
                // Handle Full/Closed according to your application's policy.
                let _ = sender.send(Message::Finished(result));
            });
        }),
    ].into()
}
run_ui_with_messages(NativeOptions::default(), App::default(), view,
    |app, sender| app.sender = Some(sender),
    |app, message| match message { Message::Finished(value) => app.answer = value },
)?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Scheduling contract

```text
thread / executor       queue (256)          UI event thread
   send(Message) -----> [typed results] ---> update(&mut App, Message)
                          |                 up to 64 results per wake
                          +-- coalesced --> view(&App) once per batch
                              winit wake    layout -> repaint
```

- `Message: Send + 'static`; app state does **not** need `Send`. Initializer runs before initial composition and proxy installation drains any messages sent that early.
- `UiSender::send` never waits for queue capacity. It consumes its message and returns `UiSendError::Full` or `Closed`; it briefly locks the queue and enqueues a native event. It is not a lock-free real-time API.
- One wake is armed for queued work, not one frame/timer per sender. `ControlFlow::Wait` remains the idle policy. An active spinner or transition can intentionally request animation frames.
- No queue mutex is held while calling application code. At most 64 messages are dispatched per event; callbacks may enqueue more work. One recomposition follows the batch, not every result. A turn latch rearms pending work only in `AboutToWait`, so macOS winit cannot keep draining a reentrant producer forever before redraw/input.
- Invalid views keep the previous document/handlers. State mutations are **not** rolled back. Callback/message work must be short; the host cannot make a blocking update nonblocking for you.
- Window shutdown closes the queue and releases queued payloads. Worker lifetimes are application-owned; `is_closed()` and explicit cancellation tokens are useful. Detached threads are not forcibly terminated.
- Use a job generation/ID to ignore stale results after cancellation/restart.
  Catalog Background tasks and existing Showcase Activity demonstrate this.
- Memory bound is **message count**, not byte size. Do not enqueue unbounded payloads; use compact results, `Arc` assets and explicit retry/drop/coalescing policies. A Full result is not automatic retry.

The native wakeup follows the pinned [winit 0.30.13 EventLoopProxy contract](https://docs.rs/winit/0.30.13/winit/event_loop/struct.EventLoopProxy.html). There is no polling file watcher, global mutable UI state or implicit async runtime.

## Example fixtures

```sh
cargo run --locked --features native-gpu --example showcase -- --screen activity
cargo run --locked --features native-gpu --example showcase -- --screen explorer
VOLT_UI_RENDERER=software cargo run --locked --features native-gpu --example showcase -- --screen activity
```

Snapshots/benchmarks are deterministic and do not start workers. Native **Prepare workspace**, **Cancel** and **Try a live click** exercise event wakeups while input remains live.

Use [Catalog](CATALOG.md) for current acceptance and new demonstrations; the
Showcase commands above are retained regression/learning routes.
