//! Stack direction, placement, selector spacing, full/numeric rounding and spacers.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;

struct LayoutDemo {
    flow: Flow,
    count: usize,
}
impl Default for LayoutDemo {
    fn default() -> Self {
        Self {
            flow: Flow::Center,
            count: 0,
        }
    }
}
fn placement(label: &str, value: Flow, selected: Flow) -> Element<LayoutDemo> {
    button(label)
        .key(format!("place-{value:?}"))
        .flex(1.)
        .rounded(8)
        .background(if value == selected {
            "#B9C8FF"
        } else {
            "#25344B"
        })
        .color(if value == selected {
            "#111A2A"
        } else {
            "#E5ECF7"
        })
        .accent_color("#B9C8FF")
        .on_click(move |app: &mut LayoutDemo| app.flow = value)
        .into()
}
fn view(app: &LayoutDemo) -> Element<LayoutDemo> {
    vstack![
        hstack![
            text("Stacks & placement").font_size(26.).font_weight(700),
            spacer(),
            badge(format!("{} clicks", app.count)).rounded(Rounding::Full)
                .background("#293B36").color("#B0DFCA"),
        ].gap(16., Axis::Horizontal).align(Align::Center),
        text("Direction arranges siblings. Placement anchors them inside their container.")
            .color("#AAB8CA"),
        vstack![
            hstack![placement("Top left", Flow::TopLeft, app.flow), placement("Top", Flow::Top, app.flow), placement("Top right", Flow::TopRight, app.flow)]
                .gap(8., Axis::Horizontal),
            hstack![placement("Left", Flow::Left, app.flow), placement("Center", Flow::Center, app.flow), placement("Right", Flow::Right, app.flow)]
                .gap(8., Axis::Horizontal),
            hstack![placement("Bottom left", Flow::BottomLeft, app.flow), placement("Bottom", Flow::Bottom, app.flow), placement("Bottom right", Flow::BottomRight, app.flow)]
                .gap(8., Axis::Horizontal),
        ].gap(8., Axis::Vertical),
        divider().background("#34445B"),
        panel![
            vstack![
                text(format!("{:?}", app.flow)).font_family("serif").italic(true).font_size(24.).color("#C5D2FC"),
                text("A vertical stack").color("#AAB8CA"),
                hstack![
                    button("Add").key("add").rounded(Rounding::Full)
                        .background("#B9C8FF").color("#111A2A").accent_color("#B9C8FF")
                        .on_click(|app: &mut LayoutDemo| app.count += 1),
                    button("Reset").key("reset").rounded(4.5).enabled(app.count > 0)
                        .background("#29384F").color("#E5ECF7").accent_color("#B9C8FF")
                        .on_click(|app: &mut LayoutDemo| app.count = 0),
                ].gap(12., Axis::Horizontal).flow(Flow::Center),
            ].key("preview-stack").gap(12., Axis::Vertical).flow(Flow::Center),
        ].key("preview").height(260.)
            .padding(20.5, Edge::Horizontal).padding(18., Edge::Vertical)
            .flow(app.flow).rounded(16).background("#1B283D")
            .border_color("#3A4F6F").border_width(1.),
        text("Full rounding makes a pill; numeric rounding sets a corner radius. Spacers are invisible and flexible.")
            .font_size(12.).color("#AAB8CA"),
        spacer(),
        hstack![text("Rust UI • no JSON bridge").font_size(12.).font_weight(600), spacer(),
            text("Try Tab / Enter too").font_size(12.).color("#AAB8CA")].gap(12., Axis::Horizontal),
    ].padding(24., Edge::All).gap(16., Axis::Vertical)
        .font_family("system").font_size(13.).color("#E5ECF7").background("#111A2A").into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch(
        "Volt UI Kit — Stacks & Placement",
        LayoutDemo::default(),
        view,
    )
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
    fn placement_and_button_callbacks_recompose_the_same_shared_runtime() {
        let mut state = LayoutDemo::default();
        let values = Bindings::default();
        let (doc, mut handlers) = view(&state).build().unwrap().into_parts();
        let mut runtime = Runtime::new(doc, &handlers, &values).unwrap();
        let viewport = Viewport::new(960., 760., 1.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        runtime.layout(viewport, &values, &mut painter).unwrap();
        for key in ["place-BottomRight", "add", "reset"] {
            let area = runtime
                .boxes()
                .iter()
                .find(|b| b.id == key)
                .unwrap()
                .bounds
                .bounds();
            let [x, y, w, h] = area;
            runtime.pointer_down(x + w / 2., y + h / 2.);
            assert!(
                runtime
                    .pointer_up_outcome(x + w / 2., y + h / 2., &mut handlers, &mut state)
                    .unwrap()
                    .activated
            );
            let (next, next_handlers) = view(&state).build().unwrap().into_parts();
            runtime
                .replace_document(next, &next_handlers, &values)
                .unwrap();
            handlers = next_handlers;
            runtime.layout(viewport, &values, &mut painter).unwrap();
            assert_eq!(state.flow, Flow::BottomRight);
            let parent = runtime
                .boxes()
                .iter()
                .find(|b| b.id == "preview")
                .unwrap()
                .bounds
                .bounds();
            let stack = runtime
                .boxes()
                .iter()
                .find(|b| b.id == "preview-stack")
                .unwrap()
                .bounds
                .bounds();
            assert!((stack[0] + stack[2] - (parent[0] + parent[2] - 20.5)).abs() < 0.01);
            assert!((stack[1] + stack[3] - (parent[1] + parent[3] - 18.)).abs() < 0.01);
            assert_eq!(state.count, usize::from(key == "add"));
        }
    }

    #[test]
    fn real_font_size_and_dpi_matrix_preserves_visible_hits_and_hidden_focus() {
        // This exercises the real software adapter, not OS resize/DPI events.
        for scale in [1., 1.25, 1.5, 2., 3.] {
            for [width, height] in [[320., 240.], [640., 480.], [960., 760.]] {
                let viewport = Viewport::new(width * scale, height * scale, scale).unwrap();
                let values = Bindings::default();
                let mut painter = RasterPainter::new(viewport);
                for flow in [
                    Flow::TopLeft,
                    Flow::Top,
                    Flow::TopRight,
                    Flow::Left,
                    Flow::Center,
                    Flow::Right,
                    Flow::BottomLeft,
                    Flow::Bottom,
                    Flow::BottomRight,
                ] {
                    let mut state = LayoutDemo { flow, count: 0 };
                    let (doc, mut handlers) = view(&state).build().unwrap().into_parts();
                    let mut runtime = Runtime::new(doc, &handlers, &values).unwrap();
                    runtime.layout(viewport, &values, &mut painter).unwrap();
                    for area in runtime.boxes() {
                        let [x, y, w, h] = area.bounds.bounds();
                        let [cx, cy, cw, ch] = area.clip.bounds();
                        assert!([x, y, w, h, cx, cy, cw, ch].iter().all(|n| n.is_finite()));
                        let epsilon = 0.01 * scale;
                        assert!(cx >= x - epsilon && cy >= y - epsilon);
                        assert!(cx + cw <= x + w + epsilon && cy + ch <= y + h + epsilon);
                        assert!(cx >= 0. && cy >= 0.);
                        assert!(
                            cx + cw <= viewport.width + epsilon
                                && cy + ch <= viewport.height + epsilon
                        );
                    }
                    let Some(add) = runtime.boxes().iter().find(|b| b.id == "add") else {
                        // The dense demo cannot fit at the host's minimum size:
                        // preview height is smaller than its fixed vertical insets.
                        // No scroll/adaptive breakpoint support is claimed here.
                        assert_eq!([width, height], [320., 240.]);
                        let preview = runtime.boxes().iter().find(|b| b.id == "preview").unwrap();
                        assert!(preview.bounds.bounds()[3] <= 36. * scale);
                        for _ in 0..12 {
                            runtime.focus_next(false);
                            assert!(!matches!(runtime.focused(), Some("add" | "reset")));
                        }
                        assert_eq!(state.count, 0);
                        assert!(!runtime
                            .paint(&mut painter, &volt_ui_kit::Theme::dark())
                            .is_empty());
                        continue;
                    };
                    let [x, y, w, h] = add.bounds.bounds();
                    let center = [x + w / 2., y + h / 2.];
                    assert!(add.clip.contains(center[0], center[1]));
                    runtime.pointer_down(center[0], center[1]);
                    assert!(
                        runtime
                            .pointer_up_outcome(center[0], center[1], &mut handlers, &mut state)
                            .unwrap()
                            .activated
                    );
                    assert_eq!(state.count, 1);
                    let (doc, next_handlers) = view(&state).build().unwrap().into_parts();
                    runtime
                        .replace_document(doc, &next_handlers, &values)
                        .unwrap();
                    handlers = next_handlers;
                    runtime.layout(viewport, &values, &mut painter).unwrap();
                    assert_eq!(runtime.focused(), Some("add"));
                    runtime.focus_next(false);
                    assert_eq!(runtime.focused(), Some("reset"));
                    runtime.key_down();
                    assert!(
                        runtime
                            .key_up_outcome(&mut handlers, &mut state)
                            .unwrap()
                            .activated
                    );
                    assert_eq!(state.count, 0);
                    assert_eq!(state.flow, flow);
                    // Shape/glyph generation uses exactly the laid-out font/clip state.
                    assert!(!runtime
                        .paint(&mut painter, &volt_ui_kit::Theme::dark())
                        .is_empty());
                }
            }
        }
    }
}
