//! Minimal lifecycle/resize/DPI example. No callbacks or file access needed.
mod support;
use volt_ui_kit::declarative::Handlers;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch(
        "Volt UI Kit — Native Window",
        r#"{"schema_version":1,"root":{"type":"column","id":"root","padding":32,"gap":20,"children":[{"type":"text","id":"title","text":"A native window, nothing hidden.","font_size":30},{"type":"panel","id":"panel","padding":24,"flex":1,"children":[{"type":"text","id":"body","text":"Resize this window. Text is shaped with cosmic-text and drawing uses the toolkit Painter boundary. The event loop waits when idle."}]}]}}"#,
        None,
        (),
        Handlers::default(),
        |_, _| {},
    )
}
