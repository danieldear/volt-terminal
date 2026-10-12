use super::style::avatar;
use super::{
    state::{radius_label, PALETTES, RADII},
    style::*,
    App,
};
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let palettes = PALETTES
        .into_iter()
        .enumerate()
        .map(|(i, name)| {
            action(
                name,
                format!("atelier-palette-{i}"),
                i == app.showcase.palette,
                move |app| {
                    app.showcase.palette = i;
                    app.record(format!("Atelier palette: {name}"));
                },
            )
            .flex(1.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let corners = RADII
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            action(
                radius_label(r),
                format!("atelier-radius-{i}"),
                i == app.showcase.radius,
                move |app| {
                    app.showcase.radius = i;
                    app.record("Changed preview corner radius");
                },
            )
            .flex(1.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let roles = [
        ("PRIMARY", ColorToken::Primary, ColorToken::PrimaryContent),
        ("WARM", ColorToken::Secondary, ColorToken::SecondaryContent),
        ("SUCCESS", ColorToken::Success, ColorToken::SuccessContent),
        ("WARNING", ColorToken::Warning, ColorToken::WarningContent),
        ("ERROR", ColorToken::Error, ColorToken::ErrorContent),
        ("NEUTRAL", ColorToken::Neutral, ColorToken::NeutralContent),
    ];
    let swatches = roles
        .chunks(2)
        .map(|pair| {
            hstack(pair.iter().map(|&(name, bg, fg)| {
                panel![
                    text("Aa").font_size(24.).font_weight(600),
                    text(name).font_family("SF Mono").font_size(9.)
                ]
                .flex(1.)
                .height(82.)
                .padding(12., Edge::All)
                .gap(8., Axis::Vertical)
                .background(bg)
                .color(fg)
                .rounded(12.)
                .into()
            }))
            .gap(10., Axis::Horizontal)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    let mut style_children = vec![
        label("THEME / LIVE TOKENS").into(),
        title("A palette with purpose.").into(),
        muted("Paired foregrounds adapt with the selected appearance.").into(),
        hstack(palettes).gap(6., Axis::Horizontal).into(),
    ];
    style_children.extend(swatches);
    style_children.extend([
        divider().background(ColorToken::Border).into(),
        label("CORNER LANGUAGE / LOGICAL PX").into(),
        hstack(corners).gap(6., Axis::Horizontal).into(),
        toggle("Clip the illustration", app.showcase.clipping)
            .key("atelier-clip-toggle")
            .height(42.)
            .font_size(12.)
            .on_change(|app: &mut App, v| {
                app.showcase.clipping = v;
                app.record("Changed illustration clipping");
            })
            .into(),
        toggle("Hide gallery scrollbars", app.showcase.hidden_scrollbars)
            .key("atelier-scrollbars")
            .height(42.)
            .font_size(12.)
            .on_change(|app: &mut App, v| {
                app.showcase.hidden_scrollbars = v;
                app.record("Changed horizontal scrollbar policy");
            })
            .into(),
    ]);
    let styling = card(style_children).key("studio-styling");
    let radius = RADII[app.showcase.radius];
    let buttons=card(vec![label("ACTION / STATEFUL").into(),title("An invitation to interact.").into(),
        action(format!("Add one  ·  {}",app.showcase.counter),"studio-add",true,|app|{app.showcase.counter+=1;app.record("Studio counter increased");}).rounded(radius).into(),
        action(if app.showcase.saved {"Saved ✓"} else {"Save a draft"},"studio-save",false,|app|{app.showcase.saved= !app.showcase.saved;app.record("Changed shared saved state");})
            .rounded_on(18.,[Corner::TopLeft,Corner::BottomRight]).border_width(1.).border_color(ColorToken::Border).background(ColorToken::Base200).into(),
        button("Unavailable action").key("studio-disabled").height(40.).font_size(12.).enabled(false).on_click(|app: &mut App| app.record("Disabled action must not run")).rounded(radius).into(),
        checkbox("Approve the direction",app.approved).key("studio-approve").height(40.).font_size(12.)
            .on_change(|app:&mut App,v|{app.approved=v;app.record("Changed approval checkbox");}).into(),
        muted("Hover, press, Tab and Space. These controls call Rust, including after screen changes.").into(),
    ]).key("studio-actions");
    let typography=card(vec![label("TYPE / HIERARCHY").into(),
        text("Aa, beautifully\nconsidered.").font_size(30.).font_weight(700).into(),
        text("SF for interface text. SF Mono for data, labels and code.").font_size(14.).into(),
        text("Let the details do the talking.").font_size(16.).font_weight(300).italic(true).color(ColorToken::Muted).into(),
        divider().background(ColorToken::Border).into(),
        text("let ui = vstack![\n    text(\"Hello, native.\"),\n    button(\"Create\")\n        .on_click(|app: &mut App| {\n            app.showcase.counter += 1;\n        }),\n];").font_family("SF Mono").font_size(11.).color(ColorToken::Primary).into(),
        muted("macOS: installed SF faces. Other platforms: sans-serif and monospace fallbacks. No bundled fonts.").font_size(11.).into(),
    ]).key("studio-type");
    let alerts=card(vec![label("FEEDBACK / SEMANTIC COLORS").into(),title("A clear signal.").into(),
        pill("READY  ·  All changes are local",ColorToken::Success,ColorToken::SuccessContent).into(),
        pill("REVIEW  ·  Keyboard flow matters",ColorToken::Warning,ColorToken::WarningContent).into(),
        pill("OFFLINE  ·  No service connected",ColorToken::Error,ColorToken::ErrorContent).into(),
        muted("Status colors are semantic theme tokens, not hard-coded text on arbitrary backgrounds.").into(),
    ]);
    let membership = card(vec![
        hstack![
            label("FICTIONAL PLAN PREVIEW"),
            spacer(),
            pill("DEMO", ColorToken::Base300, ColorToken::BaseContent)
        ]
        .into(),
        title("Space to explore.").into(),
        text(if app.showcase.annual {
            "$16 / month"
        } else {
            "$20 / month"
        })
        .font_size(30.)
        .font_weight(600)
        .into(),
        muted(if app.showcase.annual {
            "Annual illustration · no billing"
        } else {
            "Monthly illustration · no billing"
        })
        .into(),
        hstack![
            action("Monthly", "plan-monthly", !app.showcase.annual, |app| {
                app.showcase.annual = false;
                app.record("Selected monthly demo plan");
            })
            .flex(1.),
            action("Yearly", "plan-annual", app.showcase.annual, |app| {
                app.showcase.annual = true;
                app.record("Selected yearly demo plan");
            })
            .flex(1.)
        ]
        .gap(6., Axis::Horizontal)
        .into(),
        action(
            if app.showcase.subscribed {
                "Selected locally ✓"
            } else {
                "Select demo plan"
            },
            "plan-select",
            true,
            |app| {
                app.showcase.subscribed = !app.showcase.subscribed;
                app.record("Changed fictional plan selection; no payment or subscription");
            },
        )
        .rounded(Rounding::Full)
        .into(),
    ]);
    let artwork = canvas(|c| {
        c.fill(VectorPath::rectangle(0., 0., 240., 160.), "#343342");
        c.fill(VectorPath::ellipse(34., 74., 98., 70.), "#BEB0F4");
        c.fill(VectorPath::circle(170., 116., 77.), "#9EE0C5");
        c.fill(
            VectorPath::new()
                .move_to(210., -20.)
                .line_to(275., 160.)
                .line_to(110., 160.)
                .close(),
            "#F5B895",
        );
        c.stroke(
            VectorPath::new()
                .move_to(-10., 120.)
                .cubic_to([20., -40.], [220., 230.], [260., 25.]),
            "#F4F2ED",
            2.,
        );
    })
    .key("studio-art")
    .width(240.)
    .height(160.);
    let mut art_frame = zstack![artwork]
        .width(240.)
        .height(160.)
        .rounded(radius)
        .background(ColorToken::Base300);
    if app.showcase.clipping {
        art_frame = art_frame.clip(Clip::Rounded);
    }
    let surface = card(vec![
        label("SURFACE / VECTOR + CLIP").into(),
        hstack![spacer(), art_frame, spacer()]
            .height(180.)
            .scroll(Scroll::Horizontal)
            .scrollbars(Scrollbars::Hidden)
            .into(),
        title("Beyond rectangles.").into(),
        muted("Ellipses, paths and cubic curves. Change corners and clipping in the theme panel.")
            .into(),
    ])
    .key("studio-surface");
    let gallery = (0..6)
        .map(|i| {
            card(vec![
                avatar(
                    ["AL", "MK", "JT", "RI", "NO", "SK"][i],
                    ["#BEB0F4", "#F5B895", "#9EE0C5"][i % 3],
                    38.,
                ),
                text(
                    [
                        "A quiet beginning.",
                        "Build with clarity.",
                        "Make space for play.",
                        "A little more color.",
                        "Stay curious.",
                        "Native, by design.",
                    ][i],
                )
                .font_size(17.)
                .font_weight(600)
                .into(),
                label(format!("STUDY / {:02}", i + 1)).into(),
            ])
            .key(format!("studio-gallery-{i}"))
            .width(210.)
            .height(168.)
            .into()
        })
        .collect::<Vec<Element<App>>>();
    vstack![
        vstack![label("THE COMPONENT STUDIO"),text("Polish is a collection of details.").font_size(34.).font_weight(700),
            muted("Change a token. Try a corner. Follow the keyboard focus. Everything below is composed with the toolkit.")].gap(10.,Axis::Vertical),
        adaptive(vec![vstack![styling].key("studio-left").flex(1.).into(),
            vstack![buttons,typography].key("studio-center").flex(1.).gap(20.,Axis::Vertical).into(),
            vstack![surface,alerts,membership].key("studio-right").flex(1.).gap(20.,Axis::Vertical).into()]).key("studio-columns"),
        hstack![title("Little studies, one continuous flow."),spacer(),label("SCROLL →")],
        hstack(gallery).key("studio-gallery").height(180.).gap(14.,Axis::Horizontal).scroll(Scroll::Horizontal)
            .scrollbars(if app.showcase.hidden_scrollbars {Scrollbars::Hidden} else {Scrollbars::Auto}),
    ].key("showcase-studio").gap(24.,Axis::Vertical).into()
}
