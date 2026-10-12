//! Ordinary Rust loops with stable keys: reorder and removal retain identities.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::{column, prelude::*};

struct Task {
    id: u64,
    title: &'static str,
}
struct Tasks {
    items: Vec<Task>,
    removed: usize,
}
impl Default for Tasks {
    fn default() -> Self {
        Self {
            items: vec![
                Task {
                    id: 1,
                    title: "Window and event lifecycle",
                },
                Task {
                    id: 2,
                    title: "Shared layout and typography",
                },
                Task {
                    id: 3,
                    title: "Typed Rust callback integration",
                },
            ],
            removed: 0,
        }
    }
}
fn view(app: &Tasks) -> Element<Tasks> {
    let mut rows = Vec::new();
    for task in &app.items {
        let id = task.id; // Capture stable identity, not the current vector index.
        rows.push(
            row![
                text(task.title).flex(1.),
                button("Remove")
                    .key(format!("remove-{id}"))
                    .background("#29364A")
                    .color("#ECF1F8")
                    .accent_color("#A7B8FA")
                    .on_click(move |app: &mut Tasks| {
                        app.items.retain(|task| task.id != id);
                        app.removed += 1;
                    }),
            ]
            .key(format!("task-{id}"))
            .gap(14., Axis::Both)
            .padding(12., Edge::All)
            .background("#1C2533")
            .rounded(8.)
            .into(),
        );
    }
    if rows.is_empty() {
        rows.push(
            text("All tasks removed. Restore the example to try again.")
                .key("empty_state")
                .color("#AAB8CA")
                .into(),
        );
    }
    column![
        text("Keyed Rust list").font_size(25.).font_weight(700),
        text("Stable keys belong to items, not their position in a loop.").color("#AAB8CA"),
        row![
            button("Reverse order").key("reverse").primary(true)
                .background("#A7B8FA").color("#151B25").accent_color("#A7B8FA")
                .enabled(app.items.len() > 1)
                .on_click(|app: &mut Tasks| app.items.reverse()),
            button("Restore tasks").key("restore").background("#29364A")
                .color("#ECF1F8").accent_color("#A7B8FA")
                .on_click(|app: &mut Tasks| *app = Tasks::default()),
            badge(format!("{} remaining", app.items.len())).background("#263249")
                .color("#C5D2FC").font_size(12.).rounded(6.),
        ].gap(12., Axis::Both),
        separator().background("#334154"),
        column(rows).key("tasks").gap(10., Axis::Both),
        text(format!("{} tasks removed", app.removed)).font_size(12.).color("#97A7BE"),
        text("Callbacks capture item IDs. Reordering does not redirect a Remove action to another item.")
            .font_size(12.).italic(true).color("#AAB8CA"),
    ].padding(24., Edge::All).gap(18., Axis::Both).font_family("system").font_size(13.)
        .color("#ECF1F8").background("#151B25").into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch("Volt UI Kit — Rust Keyed List", Tasks::default(), view)
}
#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Viewport,
    };
    fn focus(runtime: &mut Runtime, key: &str) {
        for _ in 0..16 {
            if runtime.focused() == Some(key) {
                return;
            }
            runtime.focus_next(false);
        }
        panic!("missing focus target {key}");
    }
    #[test]
    fn reorder_preserves_focus_and_callback_targets_stable_item() {
        let mut app = Tasks::default();
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        let values = Bindings::default();
        let mut runtime = Runtime::new(document, &callbacks, &values).unwrap();
        let viewport = Viewport::new(960., 760., 1.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime.layout(viewport, &values, &mut painter).unwrap();
        focus(&mut runtime, "remove-1");
        app.items.reverse();
        let (document, mut callbacks) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &callbacks, &values)
            .unwrap();
        runtime.layout(viewport, &values, &mut painter).unwrap();
        assert_eq!(runtime.focused(), Some("remove-1"));
        let row_order: Vec<_> = runtime
            .boxes()
            .iter()
            .filter(|b| b.id.starts_with("task-"))
            .map(|b| b.id.as_str())
            .collect();
        assert_eq!(row_order, ["task-3", "task-2", "task-1"]);
        runtime.key_down();
        runtime.key_up(&mut callbacks, &mut app).unwrap();
        assert_eq!(
            app.items.iter().map(|item| item.id).collect::<Vec<_>>(),
            [3, 2]
        );
        assert_eq!(app.removed, 1);
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &callbacks, &values)
            .unwrap();
        runtime.layout(viewport, &values, &mut painter).unwrap();
        assert_eq!(runtime.focused(), None);
        assert!(!runtime.boxes().iter().any(|b| b.id == "remove-1"));
    }
    #[test]
    fn removal_and_empty_state_recompose_without_stale_rows() {
        let mut app = Tasks::default();
        let values = Bindings::default();
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        let mut runtime = Runtime::new(document, &callbacks, &values).unwrap();
        app.items.clear();
        let (document, callbacks) = view(&app).build().unwrap().into_parts();
        runtime
            .replace_document(document, &callbacks, &values)
            .unwrap();
        let viewport = Viewport::new(960., 760., 1.).unwrap();
        runtime
            .layout(viewport, &values, &mut RasterPainter::new(viewport))
            .unwrap();
        assert!(runtime.boxes().iter().any(|b| b.id == "empty_state"));
        assert!(!runtime.boxes().iter().any(|b| b.id.starts_with("remove-")));
        assert!(
            !runtime
                .boxes()
                .iter()
                .find(|b| b.id == "reverse")
                .unwrap()
                .enabled
        );
    }
}
