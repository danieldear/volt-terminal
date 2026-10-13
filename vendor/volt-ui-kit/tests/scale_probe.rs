#![cfg(feature = "native")]
//! How interaction cost grows with view size (opt-in timing probe).
use std::time::Instant;
use volt_ui_kit::{
    declarative::{Bindings, Runtime},
    native::RasterPainter,
    ui::prelude::*,
    Viewport,
};
#[derive(Default)]
struct App {
    on: bool,
}
fn view(app: &App, cards: usize) -> Element<App> {
    let items: Vec<Element<App>> = (0..cards)
        .map(|i| {
            vstack![
                text(format!("Card {i}")).font_weight(600),
                text("A short description that wraps onto a second line in narrow cards.")
                    .font_size(12.)
                    .color(ColorToken::Muted),
                hstack![
                    badge("New"),
                    spacer(),
                    button("Open")
                        .key(format!("open-{i}"))
                        .on_click(|_: &mut App| {}),
                ]
                .align(Align::Center),
            ]
            .key(format!("card-{i}"))
            .gap(6., Axis::Vertical)
            .padding(12., Edge::All)
            .rounded(10.)
            .border_color(ColorToken::Border)
            .border_width(1.)
            .into()
        })
        .collect();
    vstack![
        toggle("Toggle", app.on)
            .key("t")
            .on_change(|a: &mut App, v| a.on = v),
        grid(items)
            .key("grid")
            .min_column_width(220.)
            .gap(10., Axis::Both),
    ]
    .key("root")
    .scroll(Scroll::Vertical)
    .into()
}
#[test]
#[ignore = "timing probe"]
fn interaction_cost_scales_with_node_count() {
    let viewport = Viewport::new(2880., 1840., 2.).unwrap();
    for cards in [100usize, 250, 500, 640] {
        let mut app = App::default();
        let mut painter = RasterPainter::new(viewport);
        let built = view(&app, cards).build().unwrap();
        let (doc, h) = built.into_parts();
        let mut rt = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        rt.layout(viewport, &Bindings::default(), &mut painter)
            .unwrap();
        let n = 20;
        let (mut compose, mut layout) = (0., 0.);
        let mut nodes = 0;
        for i in 0..n {
            app.on = i % 2 == 0;
            let t = Instant::now();
            let (doc, next) = view(&app, cards).build().unwrap().into_parts();
            nodes = rt.boxes().len();
            rt.replace_document(doc, &next, &Bindings::default())
                .unwrap();
            compose += t.elapsed().as_secs_f64();
            let t = Instant::now();
            rt.layout(viewport, &Bindings::default(), &mut painter)
                .unwrap();
            layout += t.elapsed().as_secs_f64();
        }
        eprintln!(
            "SCALE cards={cards:4} boxes={nodes:5} compose+install={:6.2}ms layout={:6.2}ms",
            compose * 1000. / n as f64,
            layout * 1000. / n as f64
        );
    }
}
