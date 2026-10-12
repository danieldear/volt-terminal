//! Resize below/above 768 logical pixels: the same keyed children change direction.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;

#[derive(Default)]
struct App {
    back: usize,
    front: usize,
    compact: bool,
}
fn view(app: &App) -> Element<App> {
    vstack![
        text("Layouts that adapt").font_size(28.).font_weight(700),
        text("Below 768: vertical. At 768 and above: horizontal. Resize keeps callbacks and focus.")
            .color("#AAB8CA").font_size(13.),
        stack![
            panel![
                badge("NAVIGATION").font_size(10.).color("#95D8C7").rounded(Rounding::Full),
                text("Workspace").font_size(20.).font_weight(600),
                divider().background("#344252"),
                toggle("Compact", app.compact).key("compact")
                    .on_change(|app: &mut App, value| app.compact = value),
            ].key("navigation").width(220.).padding(16., Edge::All).gap(12., Axis::Both)
                .background("#1B2631").rounded(12),
            panel![
                text("Overlapping layers").font_size(20.).font_weight(600),
                text("The small button is above the large button. Click an exposed area to reach the back layer.")
                    .font_size(13.).color("#AAB8CA"),
                zstack![
                    button(format!("Back layer: {}", app.back)).key("back").width(220.).height(96.)
                        .background("#263847").rounded(12)
                        .on_click(|app: &mut App| app.back += 1),
                    vstack![
                        text(format!("Back layer: {}", app.back)).font_size(11.).color("#AAB8CA"),
                        spacer(),
                        button(format!("Front: {}", app.front)).key("front").width(100.).height(38.)
                            .primary(true).rounded(Rounding::Full)
                            .on_click(|app: &mut App| app.front += 1),
                        spacer(),
                    ].width(220.).height(96.).padding(8., Edge::All).flow(Flow::Top),
                ].key("layers").height(120.).padding(8., Edge::All),
                hstack![
                    text(format!("Back {} / Front {}", app.back, app.front)).font_family("monospace").font_size(12.),
                    spacer(),
                    button("Reset").key("reset").enabled(app.back + app.front > 0)
                        .on_click(|app: &mut App| { app.back = 0; app.front = 0; }),
                ].gap(8., Axis::Horizontal),
            ].key("content").flex(1.).padding(16., Edge::All).gap(12., Axis::Both)
                .background("#1B2631").rounded(12),
        ].key("adaptive")
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md, Direction::Horizontal))
            .gap(responsive(if app.compact { 8. } else { 12. })
                .at(Breakpoint::Md, if app.compact { 12. } else { 20. }), Axis::Both),
        text("VStack / HStack: fixed direction. Stack: adaptive. ZStack: back-to-front layers.")
            .font_size(12.).color("#8C9BAD"),
    ].padding(responsive(16.).at(Breakpoint::Md, 28.), Edge::All)
        .gap(16., Axis::Vertical).background("#121B24").color("#E9EFF5").into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch("Volt UI Kit — Responsive Stacks", App::default(), view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Viewport,
    };
    #[test]
    fn real_font_resize_matrix_preserves_layers_and_dispatches_only_front() {
        let mut app = App::default();
        let (doc, mut callbacks) = view(&app).build().unwrap().into_parts();
        let bindings = Bindings::default();
        let mut runtime = Runtime::new(doc, &callbacks, &bindings).unwrap();
        let mut painter = RasterPainter::new(Viewport::new(960., 760., 1.).unwrap());
        for scale in [1., 1.25, 1.5, 2., 3.] {
            for width in [480., 767., 768., 769., 960., 480.] {
                let viewport = Viewport::new(width * scale, 760. * scale, scale).unwrap();
                painter.set_viewport(viewport);
                runtime.cancel_input();
                runtime.layout(viewport, &bindings, &mut painter).unwrap();
                let bounds = |id: &str| {
                    runtime
                        .boxes()
                        .iter()
                        .find(|b| b.id == id)
                        .unwrap()
                        .bounds
                        .bounds()
                        .map(|v| v / scale)
                };
                let nav = bounds("navigation");
                let content = bounds("content");
                if width < 768. {
                    assert!(content[1] >= nav[1] + nav[3]);
                } else {
                    assert!(content[0] >= nav[0] + nav[2]);
                }
                let front = bounds("front");
                let (x, y) = (
                    (front[0] + front[2] / 2.) * scale,
                    (front[1] + front[3] / 2.) * scale,
                );
                runtime.pointer_down(x, y);
                assert!(
                    runtime
                        .pointer_up_outcome(x, y, &mut callbacks, &mut app)
                        .unwrap()
                        .activated
                );
                assert_eq!(app.back, 0);
                assert_eq!(runtime.focused(), Some("front"));
                let batches = runtime.paint(&mut painter, &volt_ui_kit::Theme::dark());
                painter
                    .rasterize(&batches, volt_ui_kit::Theme::dark().background)
                    .unwrap();
            }
        }
        assert_eq!(app.front, 30);
    }
    #[test]
    fn actions_recompose_counts_compact_spacing_and_disabled_reset_at_both_sizes() {
        for width in [480., 960.] {
            let mut app = App::default();
            let (doc, mut handlers) = view(&app).build().unwrap().into_parts();
            let values = Bindings::default();
            let mut runtime = Runtime::new(doc, &handlers, &values).unwrap();
            let viewport = Viewport::new(width, 760., 1.).unwrap();
            let mut painter = RasterPainter::new(viewport);
            runtime.layout(viewport, &values, &mut painter).unwrap();
            assert!(
                !runtime
                    .boxes()
                    .iter()
                    .find(|b| b.id == "reset")
                    .unwrap()
                    .enabled
            );
            for id in ["back", "front", "compact", "reset"] {
                let [x, y, w, h] = runtime
                    .boxes()
                    .iter()
                    .find(|b| b.id == id)
                    .unwrap()
                    .bounds
                    .bounds();
                let (x, y) = if id == "back" {
                    (x + 20., y + 12.)
                } else {
                    (x + w / 2., y + h / 2.)
                };
                runtime.pointer_down(x, y);
                assert_eq!(runtime.focused(), Some(id));
                assert!(
                    runtime
                        .pointer_up_outcome(x, y, &mut handlers, &mut app)
                        .unwrap()
                        .activated
                );
                let (next, next_handlers) = view(&app).build().unwrap().into_parts();
                runtime
                    .replace_document(next, &next_handlers, &values)
                    .unwrap();
                handlers = next_handlers;
                runtime.layout(viewport, &values, &mut painter).unwrap();
                match id {
                    "back" => assert_eq!((app.back, app.front), (1, 0)),
                    "front" => assert_eq!((app.back, app.front), (1, 1)),
                    "compact" => {
                        assert!(app.compact);
                        let nav = runtime
                            .boxes()
                            .iter()
                            .find(|b| b.id == "navigation")
                            .unwrap()
                            .bounds
                            .bounds();
                        let content = runtime
                            .boxes()
                            .iter()
                            .find(|b| b.id == "content")
                            .unwrap()
                            .bounds
                            .bounds();
                        let axis = usize::from(width < 768.);
                        let gap = content[axis] - nav[axis] - nav[axis + 2];
                        assert!((gap - if width < 768. { 8. } else { 12. }).abs() < 0.01);
                    }
                    "reset" => {
                        assert_eq!((app.back, app.front), (0, 0));
                        assert!(
                            !runtime
                                .boxes()
                                .iter()
                                .find(|b| b.id == "reset")
                                .unwrap()
                                .enabled
                        );
                        assert_eq!(runtime.focused(), None);
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
}
