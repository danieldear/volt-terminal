use super::style::avatar;
use super::{style::*, App, Screen};
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let art = canvas(|c| {
        // A folded ribbon and orbital guide lines: original vector artwork.
        for x in (0..=360).step_by(30) {
            c.stroke(
                VectorPath::new()
                    .move_to(x as f32, 0.)
                    .line_to(x as f32, 250.),
                "#BEB0F416",
                1.,
            );
        }
        for y in (0..=250).step_by(30) {
            c.stroke(
                VectorPath::new()
                    .move_to(0., y as f32)
                    .line_to(360., y as f32),
                "#BEB0F416",
                1.,
            );
        }
        c.stroke(VectorPath::ellipse(180., 126., 144., 92.), "#BEB0F44D", 1.5);
        c.fill(
            VectorPath::new()
                .move_to(46., 152.)
                .cubic_to([82., 40.], [126., 14.], [185., 56.])
                .line_to(232., 89.)
                .cubic_to([166., 58.], [108., 109.], [121., 167.])
                .close(),
            "#BEB0F4",
        );
        c.fill(
            VectorPath::new()
                .move_to(121., 167.)
                .cubic_to([141., 241.], [232., 230.], [308., 110.])
                .line_to(230., 88.)
                .cubic_to([255., 130.], [211., 193.], [165., 182.])
                .close(),
            "#9EE0C5",
        );
        c.fill(
            VectorPath::new()
                .move_to(46., 152.)
                .line_to(121., 167.)
                .line_to(165., 182.)
                .cubic_to([131., 192.], [82., 190.], [46., 152.])
                .close(),
            "#F5B895",
        );
        c.fill(VectorPath::circle(289., 53., 9.), "#F5B895");
        c.fill(VectorPath::circle(64., 219., 4.), "#BEB0F4");
        c.stroke(
            VectorPath::new()
                .move_to(304., 199.)
                .line_to(322., 199.)
                .move_to(313., 190.)
                .line_to(313., 208.),
            "#9EE0C5",
            2.,
        );
    })
    .key("atelier-ribbon")
    .width(360.)
    .height(250.);
    let hero_visual = card(vec![
        hstack![
            pill(
                "LIVE COMPOSITION",
                ColorToken::Base300,
                ColorToken::BaseContent
            ),
            spacer(),
            label("VECTOR STUDY")
        ]
        .into(),
        zstack![art].height(260.).clip_contents(true).into(),
        hstack![
            avatar("V", "#BEB0F4", 36.),
            vstack![
                text("A little structure. A lot of possibility.")
                    .font_size(14.)
                    .font_weight(600),
                muted("Paths, layers, clipping. All declared in Rust.")
            ]
            .flex(1.)
        ]
        .gap(12., Axis::Horizontal)
        .align(Align::Center)
        .into(),
    ])
    .key("welcome-art")
    .flex(1.)
    .rounded(28.);
    let hero_copy=vstack![label("A NATIVE UI TOOLKIT, WITH ROOM TO CREATE"),
        text("Make room\nfor great ideas.").key("atelier-hero-title").font_size(52.).font_weight(700),
        text("Thoughtful interfaces. Ordinary Rust.").font_size(19.).font_weight(400).color(ColorToken::Primary),
        muted("Build expressive desktop experiences with composable layouts, considered typography and real callbacks. No webview. No borrowed identity."),
        stack![route(Screen::Workspace,"atelier-enter","Explore workspace  →").flex(1.),
            action("Open component studio","atelier-open-studio",false,|app|{app.showcase.screen=Screen::Studio;app.record("Opened component studio");}).flex(1.)]
            .direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal)).gap(10.,Axis::Both),
        hstack![label("SF / SF MONO").width(110.),spacer().height(1.).background(ColorToken::Border),label("VECTORS / RUST").width(116.)].gap(14.,Axis::Horizontal),
    ].key("welcome-copy").flex(1.).gap(22.,Axis::Vertical);
    let preview=card(vec![
        hstack![title("Ready for a closer look?"),spacer(),pill("INTERACTIVE",ColorToken::Success,ColorToken::SuccessContent)].into(),
        muted("Start with a small interaction. The counter, saved state and project screens share one App." ).into(),
        hstack![text(format!("{:02}",app.showcase.counter)).font_family("SF Mono").font_size(42.).font_weight(500),spacer(),
            action("−","welcome-decrement",false,|app|{app.showcase.counter=app.showcase.counter.saturating_sub(1);app.record("Decreased preview count");}).width(44.),
            action("+","welcome-increment",true,|app|{app.showcase.counter+=1;app.record("Increased preview count");}).width(44.)]
            .gap(8.,Axis::Horizontal).align(Align::Center).into(),
    ]).flex(1.);
    let code=card(vec![
        hstack![label("DECLARATIVE, NOT DISCONNECTED"),spacer(),pill(".rs",ColorToken::Base300,ColorToken::BaseContent)].into(),
        text("vstack![\n    text(\"Make something great\"),\n    button(\"Save\")\n        .on_click(|app: &mut App| {\n            app.showcase.saved = true;\n        }),\n]\n.rounded(20)")
            .font_family("SF Mono").font_size(13.).color(ColorToken::Primary).into(),
        action(if app.showcase.saved {"Saved to this session ✓"} else {"Try the Rust callback"},"welcome-save",false,|app|{
            app.showcase.saved= !app.showcase.saved; app.record(if app.showcase.saved {"Saved sample in memory"} else {"Unsaved sample"});
        }).into(),
    ]).flex(1.);
    vstack![
        adaptive(vec![hero_copy.into(),hero_visual.into()]).key("welcome-hero").align(Align::Center),
        spacer().height(12.),divider().background(ColorToken::Border),
        stack![vstack![label("01 / COMPOSE"),title("Small pieces. Clear intent."),muted("Stacks, panels, buttons and semantic colors.")].flex(1.),
            vstack![label("02 / RESPOND"),title("One idea, every window."),muted("Resize to see columns become a natural flow.")].flex(1.),
            vstack![label("03 / REFINE"),title("Details you can feel."),muted("Corners, subtle motion and honest interaction.")].flex(1.)]
            .direction(responsive(Direction::Vertical).at(Breakpoint::Lg,Direction::Horizontal)).gap(28.,Axis::Both),
        adaptive(vec![preview.into(),code.into()]),
        muted("An original component showcase. All project names, activity and people below are fictional local demo data."),
    ].key("showcase-welcome").gap(28.,Axis::Vertical).into()
}
