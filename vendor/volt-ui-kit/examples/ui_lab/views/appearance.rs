use super::{description, specimen};
use crate::lab::{state::OPACITIES, App};
use volt_ui_kit::ui::prelude::*;

pub fn typography(app: &App) -> Element<App> {
    let families = ["system", "sans-serif", "serif", "monospace"]
        .into_iter()
        .map(|family| {
            button(family)
                .key(format!("font-{family}"))
                .width(132.)
                .font_family(family)
                .primary(app.font == family)
                .color(if app.font == family {
                    ColorToken::PrimaryContent
                } else {
                    ColorToken::BaseContent
                })
                .on_click(move |app: &mut App| {
                    app.font = family.into();
                    app.record(format!("Font family: {family}"));
                })
                .into()
        })
        .collect::<Vec<_>>();
    let weights = [300, 400, 600, 700, 900]
        .into_iter()
        .map(|weight| {
            text(format!("Weight {weight}  —  The quick brown fox"))
                .font_weight(weight)
                .font_size(20.)
                .into()
        })
        .collect::<Vec<_>>();
    let pairs = [
        ("Primary", ColorToken::Primary, ColorToken::PrimaryContent),
        (
            "Secondary",
            ColorToken::Secondary,
            ColorToken::SecondaryContent,
        ),
        ("Accent", ColorToken::Accent, ColorToken::AccentContent),
        ("Neutral", ColorToken::Neutral, ColorToken::NeutralContent),
        ("Info", ColorToken::Info, ColorToken::InfoContent),
        ("Success", ColorToken::Success, ColorToken::SuccessContent),
        ("Warning", ColorToken::Warning, ColorToken::WarningContent),
        ("Error", ColorToken::Error, ColorToken::ErrorContent),
    ];
    let swatches = pairs
        .into_iter()
        .map(|(name, bg, fg)| {
            panel![
                text(name).font_weight(600),
                text("Paired foreground").font_size(12.)
            ]
            .width(148.)
            .height(70.)
            .padding(12., Edge::All)
            .gap(6., Axis::Vertical)
            .background(bg)
            .color(fg)
            .rounded(4.)
            .into()
        })
        .collect::<Vec<_>>();
    vstack![
        description(format!("Active family: {}. Missing families or weights use the installed font fallback.",app.font)),
        hstack(families).height(46.).gap(8.,Axis::Horizontal).scroll(Scroll::Horizontal),
        specimen("Type scale and weight",vec![
            text("A heading at 32px").font_size(32.).font_weight(700).into(),
            text("A readable body at 16px").font_size(16.).into(),
            text("A caption at 12px").font_size(12.).color(ColorToken::Muted).into(),
            vstack(weights).gap(8.,Axis::Vertical).into(),
            text("Italic text, including Unicode: café • Ελληνικά • 日本語").italic(true).font_size(18.).into(),
            text("fn view(app: &App) -> Element<App>").font_family("monospace").font_size(13.).into(),
        ]),
        specimen("Semantic colors",vec![
            description("Change Light/Dark/System or the accent above. Each role has a matching foreground."),
            hstack(swatches).height(82.).gap(10.,Axis::Horizontal).scroll(Scroll::Horizontal).into(),
            hstack![badge("Base 100").background(ColorToken::Base100),badge("Base 200").background(ColorToken::Base200),badge("Base 300").background(ColorToken::Base300)]
                .gap(10.,Axis::Horizontal).into(),
            text("Literal color override: #E07C45").color("#E07C45").font_weight(600).into(),
        ]),
        description("Unicode glyph coverage depends on installed fonts and backend fallback. Missing glyphs reveal gaps rather than being simulated."),
        description("For an installed custom family: --font \"Your Font Name\". The lab does not load arbitrary font files."),
    ].gap(18.,Axis::Vertical).into()
}
pub fn effects(app: &App) -> Element<App> {
    let notice: Element<App> = if app.notice {
        panel![text("Saved. This notice fades.").font_size(13.)]
            .key("effects-notice")
            .width(220.)
            .height(64.)
            .flow(Flow::Center)
            .padding(10., Edge::All)
            .background(ColorToken::Success)
            .color(ColorToken::SuccessContent)
            .rounded_on(12., Corners::Left)
            .transition(Transition::fade().duration_ms(180))
            .into()
    } else {
        spacer().width(220.).height(64.).into()
    };
    vstack![
        description("Hover, press, and Tab to test appearance states. Callbacks are synchronous, not delayed until animations finish."),
        specimen("Glow, shadow and state animation",vec![
            hstack![button("Save").key("save-effects").width(132.).height(44.).primary(true).color(ColorToken::PrimaryContent)
                .glow(Glow::new().color(ColorToken::Accent).blur(10.).spread(1.))
                .hover_style(|s|s.background(ColorToken::Accent).glow(Glow::new().color(ColorToken::Accent).blur(18.).spread(2.)))
                .pressed_style(|s|s.opacity(0.65))
                .focus_style(|s|s.border_color(ColorToken::Accent).border_width(2.))
                .animate_on(Interaction::Hover,Animation::ease_out().duration_ms(140))
                .animate_on(Interaction::Pressed,Animation::ease_out().duration_ms(70))
                .on_click(|app:&mut App|{app.saved+=1;app.notice=true;app.record(format!("Saved {} times",app.saved));}),
                text(format!("{} saves",app.saved)).font_size(16.)]
                .gap(22.,Axis::Horizontal).align(Align::Center).padding(20.,Edge::All).into(),
            panel![text("Shadow outside this panel"),description("Blur 12px, offset 0 / 5px. Ancestor scroll clipping remains intentional.")]
                .padding(16.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300).rounded(8.)
                .border_width(1.).border_color(ColorToken::Border).shadow(Shadow::new().blur(12.).offset(0.,5.).color("#00000060")).into(),
        ]),
        specimen("Centered fade transition",vec![
            button("Show / hide notice").key("notice-toggle").on_click(|app:&mut App|{app.notice = !app.notice;app.record(format!("Notice visible: {}",app.notice));}).into(),
            hstack![spacer(),notice,spacer()].height(64.).into(),
            description("The wrapper's spacers center the notice. Fade-out is paint-only; removed content cannot receive clicks."),
        ]),
        specimen("Group opacity",vec![
            button(format!("Opacity: {:.0}%",OPACITIES[app.opacity]*100.)).key("opacity-next")
                .on_click(|app:&mut App|{app.opacity=(app.opacity+1)%OPACITIES.len();app.record("Changed group opacity");}).into(),
            panel![text("Text fades with the group"),button("Still interactive").key("faded-action").primary(true).color(ColorToken::PrimaryContent)
                .on_click(|app:&mut App|app.record("Opacity changes paint, not hit testing"))]
                .padding(18.,Edge::All).gap(12.,Axis::Vertical).background(ColorToken::Base300).opacity(OPACITIES[app.opacity]).into(),
        ]),
        description(if app.reduced_motion {"Reduced motion requested at startup: animations settle immediately."} else {"Restart with --reduced-motion (or VOLT_UI_REDUCED_MOTION=1) to disable animation."}),
    ].gap(18.,Axis::Vertical).into()
}
