use super::{choice, description, specimen};
use crate::lab::App;
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let placements = [
        ("Top left", Flow::TopLeft),
        ("Top", Flow::Top),
        ("Top right", Flow::TopRight),
        ("Left", Flow::Left),
        ("Center", Flow::Center),
        ("Right", Flow::Right),
        ("Bottom left", Flow::BottomLeft),
        ("Bottom", Flow::Bottom),
        ("Bottom right", Flow::BottomRight),
    ];
    let rows = placements
        .chunks(3)
        .map(|row| {
            hstack(row.iter().map(|&(name, flow)| {
                choice(
                    name,
                    format!("flow-{flow:?}"),
                    app.flow == flow,
                    move |app| {
                        app.flow = flow;
                        app.record(format!("Placement: {flow:?}"));
                    },
                )
                .flex(1.)
            }))
            .gap(6., Axis::Horizontal)
            .into()
        })
        .collect::<Vec<_>>();
    let gap = [8., 16., 24.][app.spacing];
    let sample = button("Corner sample")
        .key("corner-sample")
        .height(54.)
        .width(180.)
        .primary(true)
        .color(ColorToken::PrimaryContent)
        .on_click(|app: &mut App| app.record("Clicked corner sample"));
    let sample: Element<App> = match app.rounding {
        1 => sample.rounded(8.).into(),
        2 => sample.rounded(Rounding::Full).into(),
        3 => sample
            .rounded_on(18., [Corner::TopLeft, Corner::BottomRight])
            .into(),
        _ => sample.into(),
    };
    vstack![
        description("Resize across 768 logical pixels. Stack changes direction; VStack/HStack stay fixed.").key("layout-intro"),
        specimen("Adaptive stack",vec![
            stack![panel![text("First panel"),badge("Primary").background(ColorToken::Primary).color(ColorToken::PrimaryContent)]
                    .key("adaptive-first").flex(1.).height(100.).padding(16.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300),
                panel![text("Second panel"),badge("Accent").background(ColorToken::Accent).color(ColorToken::AccentContent)]
                    .key("adaptive-second").flex(1.).height(100.).padding(16.,Edge::All).gap(8.,Axis::Vertical).background(ColorToken::Base300)]
                .direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal))
                .gap(gap,Axis::Both).into(),
            button(format!("Padding / gap: {gap:.0}px")).key("spacing-next")
                .on_click(|app:&mut App|{app.spacing=(app.spacing+1)%3;app.record("Changed multi-edge padding and gap");}).into(),
            panel![hstack![badge("Left"),spacer(),badge("Right")].gap(gap,Axis::Horizontal),divider().background(ColorToken::Border),text("Padding uses [Top, Left, Right]; bottom uses its own value.")]
                .padding(gap,[Edge::Top,Edge::Left,Edge::Right]).padding(6.,Edge::Bottom)
                .gap(gap,[Axis::Horizontal,Axis::Vertical]).background(ColorToken::Base300).into(),
        ]),
        specimen("Placement",vec![
            vstack(rows).gap(6.,Axis::Vertical).into(),
            panel![badge(format!("{:?}",app.flow)).height(32.).width(112.)
                .background(ColorToken::Accent).color(ColorToken::AccentContent)]
                .height(180.).flow(app.flow).padding(12.,Edge::All).background(ColorToken::Base300).into(),
        ]),
        specimen("Corner selection",vec![
            description("Default 2px / numeric radius / full / top-left + bottom-right"),sample,
            button("Next corner style").key("rounding-next").on_click(|app:&mut App|{app.rounding=(app.rounding+1)%4;app.record("Changed selected corners");}).into(),
        ]),
        specimen("Overlapping layers",vec![
            zstack![
                button(format!("Back: {}",app.back)).key("layer-back").width(220.).height(96.)
                    .background(ColorToken::Base300).on_click(|app:&mut App|{app.back+=1;app.record("Back layer clicked");}),
                button(format!("Front: {}",app.front)).key("layer-front").width(100.).height(40.)
                    .primary(true).color(ColorToken::PrimaryContent).on_click(|app:&mut App|{app.front+=1;app.record("Front layer clicked; back unchanged");}),
            ].height(120.).flow(Flow::Center).into(),
            description("Click the small front button or an exposed part of the back button. Only the topmost target activates."),
        ]),
    ].gap(18.,Axis::Vertical).into()
}
