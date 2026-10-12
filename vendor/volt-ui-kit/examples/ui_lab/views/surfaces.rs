use super::{choice, description, specimen};
use crate::lab::{state::OPACITIES, App};
use volt_ui_kit::ui::prelude::*;

pub fn view(app: &App) -> Element<App> {
    let fits = [
        ("Cover", ImageFit::Cover),
        ("Contain", ImageFit::Contain),
        ("Stretch", ImageFit::Stretch),
        ("Tile", ImageFit::Tile),
    ]
    .into_iter()
    .map(|(name, fit)| {
        choice(
            name,
            format!("image-fit-{name}"),
            app.image_fit == fit,
            move |app| {
                app.image_fit = fit;
                app.record(format!("Image fit: {name}"));
            },
        )
        .width(100.)
    })
    .collect::<Vec<_>>();
    let clip_names = ["Rounded", "Circle", "Ellipse", "Diamond"];
    let clipping = match app.clip {
        1 => Clip::Circle,
        2 => Clip::Path(VectorPath::ellipse(110., 70., 110., 70.)),
        3 => Clip::Path(
            VectorPath::new()
                .move_to(110., 0.)
                .line_to(220., 70.)
                .line_to(110., 140.)
                .line_to(0., 70.)
                .close(),
        ),
        _ => Clip::Rounded,
    };
    let clip_target = zstack![
        canvas(|c| {
            c.fill(
                VectorPath::rectangle(0., 0., 220., 140.),
                ColorToken::Primary,
            );
            c.fill(
                VectorPath::ellipse(70., 60., 100., 45.),
                ColorToken::Warning,
            );
            c.stroke(
                VectorPath::new()
                    .move_to(0., 120.)
                    .cubic_to([60., 0.], [170., 170.], [220., 20.]),
                ColorToken::PrimaryContent,
                8.,
            );
        })
        .width(220.)
        .height(140.),
        button("Clipped click target")
            .key("clipped-action")
            .width(220.)
            .height(140.)
            .background("#00000000")
            .border_width(0.)
            .color(ColorToken::PrimaryContent)
            .on_click(|app: &mut App| app.record("Clicked inside the clipping mask")),
    ]
    .width(220.)
    .height(140.)
    .rounded(24.)
    .clip(clipping);
    vstack![
        description("Shared image handles, background-only alpha, paths and matching visual/input masks."),
        specimen("Image background",vec![
            hstack(fits).height(46.).gap(8.,Axis::Horizontal).scroll(Scroll::Horizontal).into(),
            panel![badge("Foreground remains crisp").font_weight(700).background(ColorToken::Neutral).color(ColorToken::NeutralContent),spacer(),
                badge("Generated image; no downloads or file watching.").font_size(12.).background(ColorToken::Neutral).color(ColorToken::NeutralContent)]
                .height(190.).padding(18.,Edge::All).background(ColorToken::Base300)
                .background_image(app.image.clone().background().fit(app.image_fit).position(app.image_position).opacity(OPACITIES[app.opacity]))
                .rounded(16.).clip_contents(true).into(),
            stack![button(format!("Image opacity: {:.0}%",OPACITIES[app.opacity]*100.)).key("image-opacity")
                    .on_click(|app:&mut App|{app.opacity=(app.opacity+1)%OPACITIES.len();app.record("Changed image-only opacity");}),
                button(format!("Position: {:?}",app.image_position)).key("image-position")
                    .on_click(|app:&mut App|{app.image_position=if app.image_position==ImagePosition::Center {ImagePosition::TopLeft} else {ImagePosition::Center};app.record("Changed image position");})]
                .direction(responsive(Direction::Vertical).at(Breakpoint::Md,Direction::Horizontal)).gap(8.,Axis::Both).into(),
        ]),
        specimen("Vector shapes",vec![
            description("Rectangle, circle, ellipse, triangle, quadratic/cubic curves. Scroll sideways on a smaller window."),
            hstack![canvas(|c|{
                c.fill(VectorPath::rectangle(10.,25.,90.,80.),ColorToken::Primary);
                c.fill(VectorPath::circle(170.,65.,44.),ColorToken::Success);
                c.fill(VectorPath::ellipse(300.,65.,72.,38.),ColorToken::Warning);
                c.fill(VectorPath::new().move_to(415.,105.).line_to(450.,12.).line_to(485.,105.).close(),ColorToken::Error);
                c.stroke(VectorPath::new().move_to(20.,150.).quad_to([100.,80.],[180.,150.]).cubic_to([240.,220.],[330.,80.],[480.,145.]),ColorToken::Accent,4.);
                c.stroke(VectorPath::ellipse(300.,65.,72.,38.).transformed([1.,0.,0.,1.,3.,3.]),ColorToken::BaseContent,2.);
            }).key("shape-canvas").width(520.).height(190.)]
                .height(200.).scroll(Scroll::Horizontal).background(ColorToken::Base100).rounded(8.).into(),
        ]),
        specimen("Clipped contents and events",vec![
            button(format!("Mask: {} (next)",clip_names[app.clip])).key("clip-next")
                .on_click(move |app:&mut App|{app.clip=(app.clip+1)%clip_names.len();app.record(format!("Clip mask: {}",clip_names[app.clip]));}).into(),
            hstack![spacer(),clip_target,spacer()].height(140.).into(),
            description("Click outside the mask: no action. Click inside: the footer records the callback."),
        ]),
        specimen("Background alpha versus group alpha",vec![
            panel![text("Only the background is translucent"),button("Opaque foreground").key("background-alpha-action")
                .on_click(|app:&mut App|app.record("Foreground remains opaque and interactive"))]
                .padding(18.,Edge::All).gap(12.,Axis::Vertical).background(ColorToken::Primary).background_opacity(0.25).into(),
            description("Effects section changes whole-group opacity. Here text and controls keep their original opacity."),
        ]),
    ].gap(18.,Axis::Vertical).into()
}
