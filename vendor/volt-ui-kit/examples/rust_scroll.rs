//! Nested scrolling, automatic overflow, fixed headers and stable Rust callbacks.
#[path = "support/rust_ui.rs"]
mod support;
use volt_ui_kit::ui::prelude::*;
#[derive(Default)]
struct App {
    clicked: usize,
}
fn view(app: &App) -> Element<App> {
    let items = (0..60)
        .map(|i| {
            hstack![
                text(format!("{:02}", i + 1))
                    .width(40.)
                    .color(ColorToken::Muted),
                text(format!("Document {}", i + 1)),
                spacer(),
                button("Open")
                    .key(format!("open-{i}"))
                    .on_click(|app: &mut App| app.clicked += 1),
            ]
            .height(58.)
            .padding(12., Edge::Horizontal)
            .background(if i % 2 == 0 {
                ColorToken::Base200
            } else {
                ColorToken::Base100
            })
            .into()
        })
        .collect::<Vec<_>>();
    vstack![
        hstack![text("SCROLL / NESTED").font_size(14.).font_weight(700),spacer(),text(format!("Opened {}",app.clicked))]
            .height(48.).flow(Flow::Center),
        text("A fixed header. A scrollable body.").font_size(28.).font_weight(700),
        text("Wheel / trackpad to scroll. Shift-wheel scrolls horizontally. Nested views bubble at their limits.").font_size(13.).color(ColorToken::Muted),
        hstack![
            vstack(items).key("documents").flex(2.).scroll(Scroll::Auto).rounded(12.).border_color(ColorToken::Border),
            vstack![text("INNER VIEW").font_weight(700),
                vstack((0..30).map(|i|text(format!("Nested row {}",i+1)).height(30.).into()).collect::<Vec<_>>())
                    .key("inner").height(240.).scroll(Scroll::Vertical).background(ColorToken::Base100).rounded(8.),
                text("At the bottom, wheel input continues in the containing panel."),
                canvas(|c|c.fill(VectorPath::rectangle(0.,0.,180.,700.),ColorToken::Accent)).height(700.),
            ] .key("outer").flex(1.).padding(16.,Edge::All).gap(16.,Axis::Vertical).scroll(Scroll::Auto)
                .background(ColorToken::Base200).rounded(12.),
        ].flex(1.).gap(20.,Axis::Horizontal).align(Align::Stretch),
    ].padding(24.,Edge::All).gap(16.,Axis::Vertical).background(ColorToken::Base100).color(ColorToken::BaseContent).into()
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    support::launch("Volt UI — Scrolling", App::default(), view)
}
#[cfg(test)]
mod tests {
    use super::*;
    use volt_ui_kit::{
        declarative::{Bindings, Runtime},
        native::RasterPainter,
        Painter, Viewport,
    };
    #[test]
    fn last_document_is_reachable_and_callback_remains_correct() {
        let app = App::default();
        let (doc, mut h) = view(&app).build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        let mut p = RasterPainter::new(Viewport::new(960., 760., 1.).unwrap());
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        let list = r
            .boxes()
            .iter()
            .find(|b| b.id == "documents")
            .unwrap()
            .bounds
            .bounds();
        assert!(r.scroll_at(list[0] + 30., list[1] + 20., [0., 16_384.]));
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        let target = r
            .boxes()
            .iter()
            .find(|b| b.id == "open-59")
            .unwrap()
            .bounds
            .bounds();
        let x = target[0] + target[2] / 2.;
        let y = target[1] + target[3] / 2.;
        let mut app = app;
        r.pointer_down(x, y);
        r.pointer_up(x, y, &mut h, &mut app).unwrap();
        assert_eq!(app.clicked, 1);
    }
}
