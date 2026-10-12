use super::{description, specimen};
use crate::lab::App;
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let bars = if app.hidden_bars {
        Scrollbars::Hidden
    } else {
        Scrollbars::Auto
    };
    let rows = (0..60)
        .map(|i| {
            hstack![
                text(format!("Document {:02}", i + 1)).flex(1.),
                button("Open")
                    .key(format!("document-{i}"))
                    .width(80.)
                    .height(36.)
                    .on_click(move |app: &mut App| {
                        app.opened = Some(i);
                        app.record(format!("Opened document {}", i + 1));
                    })
            ]
            .height(48.)
            .padding(10., Edge::Horizontal)
            .align(Align::Center)
            .background(if i % 2 == 0 {
                ColorToken::Base200
            } else {
                ColorToken::Base300
            })
            .into()
        })
        .collect::<Vec<_>>();
    let cards = (0..12)
        .map(|i| {
            panel![
                badge(format!("Card {}", i + 1))
                    .background(ColorToken::Accent)
                    .color(ColorToken::AccentContent),
                button("Choose")
                    .key(format!("horizontal-{i}"))
                    .on_click(move |app: &mut App| app
                        .record(format!("Selected horizontal card {}", i + 1)))
            ]
            .key(format!("horizontal-card-{i}"))
            .width(156.)
            .height(112.)
            .padding(14., Edge::All)
            .gap(14., Axis::Vertical)
            .background(ColorToken::Base300)
            .rounded(6.)
            .into()
        })
        .collect::<Vec<_>>();
    vstack![
        description("The page itself scrolls. Inner views scroll first and pass unused movement to parents at their limits."),
        toggle("Hide inner scrollbars (keep scrolling)",app.hidden_bars).key("hide-scrollbars")
            .on_change(|app:&mut App,value|{app.hidden_bars=value;app.record(format!("Inner scrollbars hidden: {value}"));}),
        vstack(rows).key("scroll-documents").height(260.).scroll(Scroll::Vertical).scrollbars(bars).rounded(8.),
        description(format!("60 rows. Last opened: {}",app.opened.map_or_else(||"none".into(),|i|(i+1).to_string()))),
        specimen("Horizontal cards",vec![
            description("Use a horizontal trackpad gesture or Shift-wheel. Indicators can be hidden without changing input."),
            hstack(cards).key("horizontal-scroll").height(126.).gap(12.,Axis::Horizontal).scroll(Scroll::Horizontal).scrollbars(bars).into(),
        ]),
        specimen("Both axes",vec![
            description("An 800x400 logical canvas inside a smaller viewport. Pan each axis independently."),
            vstack![canvas(|c|{
                for y in 0..4 {for x in 0..8 {
                    c.fill(VectorPath::rectangle(x as f32*100.+8.,y as f32*100.+8.,84.,84.),if (x+y)%2==0 {ColorToken::Primary} else {ColorToken::Secondary});
                }}
            }).width(800.).height(400.)]
                .key("both-scroll").height(190.).scroll(Scroll::Both).scrollbars(bars).rounded(8.).into(),
        ]),
        specimen("Nested boundaries",vec![
            vstack![description("Outer scroll view"),
                vstack((0..24).map(|i|text(format!("Inner line {}",i+1)).height(28.).into()).collect::<Vec<_>>())
                    .key("nested-inner").height(140.).scroll(Scroll::Vertical).scrollbars(bars)
                    .background(ColorToken::Base100).rounded(4.),
                canvas(|c|c.fill(VectorPath::rectangle(0.,0.,120.,350.),ColorToken::Accent)).height(350.),
                text("End of the outer scroll view"),
            ].key("nested-outer").height(240.).gap(12.,Axis::Vertical).padding(12.,Edge::All)
                .scroll(Scroll::Vertical).scrollbars(bars).background(ColorToken::Base300).rounded(6.).into(),
        ]),
        description("Thin indicators are not draggable yet. Keyboard focus auto-reveal, touch-drag and smooth scrolling are pending."),
    ].gap(16.,Axis::Vertical).into()
}
