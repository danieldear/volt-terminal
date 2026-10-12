//! Line icons on a 24-unit grid, drawn as vectors (crisp at any scale).
use super::App;
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Compose,
    Search,
    Bell,
    Folder,
    Sidebar,
    Inspector,
    More,
    Plus,
    ArrowUp,
    Stop,
    Mic,
    Check,
    Terminal,
    File,
    Shield,
    Gear,
    Retry,
    Heart,
    Diff,
    Pin,
    Spark,
    ChevronDown,
    ChevronRight,
}

pub fn icon(glyph: Icon, size: f32, color: impl Into<String>) -> Element<App> {
    let color = color.into();
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        let p = VectorPath::new;
        let stroke = |c: &mut Canvas, path: VectorPath| {
            c.stroke(path.transformed(m), color.clone(), 1.7 * s)
        };
        let fill = |c: &mut Canvas, path: VectorPath| c.fill(path.transformed(m), color.clone());
        let rounded = |x: f32, y: f32, w: f32, h: f32, r: f32| {
            p().move_to(x + r, y)
                .line_to(x + w - r, y)
                .quad_to([x + w, y], [x + w, y + r])
                .line_to(x + w, y + h - r)
                .quad_to([x + w, y + h], [x + w - r, y + h])
                .line_to(x + r, y + h)
                .quad_to([x, y + h], [x, y + h - r])
                .line_to(x, y + r)
                .quad_to([x, y], [x + r, y])
                .close()
        };
        match glyph {
            Icon::Compose => {
                stroke(
                    c,
                    p().move_to(11., 4.)
                        .line_to(6., 4.)
                        .quad_to([4., 4.], [4., 6.])
                        .line_to(4., 18.)
                        .quad_to([4., 20.], [6., 20.])
                        .line_to(18., 20.)
                        .quad_to([20., 20.], [20., 18.])
                        .line_to(20., 13.),
                );
                stroke(
                    c,
                    p().move_to(17.5, 3.5)
                        .line_to(20.5, 6.5)
                        .line_to(12., 15.)
                        .line_to(8.5, 15.5)
                        .line_to(9., 12.)
                        .close(),
                );
            }
            Icon::Search => {
                stroke(c, VectorPath::circle(10.5, 10.5, 6.));
                stroke(c, p().move_to(15., 15.).line_to(20., 20.));
            }
            Icon::Bell => {
                stroke(
                    c,
                    p().move_to(6., 16.5)
                        .line_to(6., 11.)
                        .cubic_to([6., 7.5], [8.7, 5.], [12., 5.])
                        .cubic_to([15.3, 5.], [18., 7.5], [18., 11.])
                        .line_to(18., 16.5)
                        .line_to(19.5, 18.)
                        .line_to(4.5, 18.)
                        .close(),
                );
                stroke(c, p().move_to(10., 21.).line_to(14., 21.));
            }
            Icon::Folder => stroke(
                c,
                p().move_to(3.5, 7.)
                    .quad_to([3.5, 5.], [5.5, 5.])
                    .line_to(9.5, 5.)
                    .line_to(11.5, 7.5)
                    .line_to(18.5, 7.5)
                    .quad_to([20.5, 7.5], [20.5, 9.5])
                    .line_to(20.5, 17.)
                    .quad_to([20.5, 19.], [18.5, 19.])
                    .line_to(5.5, 19.)
                    .quad_to([3.5, 19.], [3.5, 17.])
                    .close(),
            ),
            Icon::Sidebar | Icon::Inspector => {
                stroke(c, rounded(3.5, 5., 17., 14., 2.5));
                let x = if glyph == Icon::Sidebar { 9.5 } else { 14.5 };
                stroke(c, p().move_to(x, 5.).line_to(x, 19.));
            }
            Icon::More => {
                for x in [6., 12., 18.] {
                    fill(c, VectorPath::circle(x, 12., 1.5));
                }
            }
            Icon::Plus => {
                stroke(c, p().move_to(12., 5.).line_to(12., 19.));
                stroke(c, p().move_to(5., 12.).line_to(19., 12.));
            }
            Icon::ArrowUp => {
                stroke(c, p().move_to(12., 19.).line_to(12., 5.5));
                stroke(
                    c,
                    p().move_to(6.5, 11.).line_to(12., 5.5).line_to(17.5, 11.),
                );
            }
            Icon::Stop => fill(c, rounded(7., 7., 10., 10., 2.)),
            Icon::Mic => {
                stroke(c, rounded(9., 3.5, 6., 11., 3.));
                stroke(
                    c,
                    p().move_to(5.5, 11.)
                        .cubic_to([5.5, 15.], [8.5, 17.5], [12., 17.5])
                        .cubic_to([15.5, 17.5], [18.5, 15.], [18.5, 11.]),
                );
                stroke(c, p().move_to(12., 17.5).line_to(12., 21.));
            }
            Icon::Check => stroke(c, p().move_to(5., 12.5).line_to(10., 17.).line_to(19., 7.)),
            Icon::Terminal => {
                stroke(c, rounded(3., 4.5, 18., 15., 2.5));
                stroke(c, p().move_to(7., 9.5).line_to(10., 12.).line_to(7., 14.5));
                stroke(c, p().move_to(12., 15.).line_to(16.5, 15.));
            }
            Icon::File => {
                stroke(
                    c,
                    p().move_to(13.5, 3.5)
                        .line_to(7., 3.5)
                        .quad_to([5., 3.5], [5., 5.5])
                        .line_to(5., 18.5)
                        .quad_to([5., 20.5], [7., 20.5])
                        .line_to(17., 20.5)
                        .quad_to([19., 20.5], [19., 18.5])
                        .line_to(19., 9.)
                        .close(),
                );
                stroke(c, p().move_to(13.5, 3.5).line_to(13.5, 9.).line_to(19., 9.));
            }
            Icon::Shield => {
                stroke(
                    c,
                    p().move_to(12., 3.5)
                        .line_to(19., 6.)
                        .line_to(19., 11.5)
                        .cubic_to([19., 16.], [16., 19.], [12., 20.5])
                        .cubic_to([8., 19.], [5., 16.], [5., 11.5])
                        .line_to(5., 6.)
                        .close(),
                );
                stroke(c, p().move_to(12., 8.).line_to(12., 12.5));
                fill(c, VectorPath::circle(12., 15.5, 1.1));
            }
            Icon::Gear => {
                stroke(c, VectorPath::circle(12., 12., 3.));
                for i in 0..8 {
                    let (sn, cs) = (i as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
                    stroke(
                        c,
                        p().move_to(12. + cs * 6., 12. + sn * 6.)
                            .line_to(12. + cs * 8.5, 12. + sn * 8.5),
                    );
                }
                stroke(c, VectorPath::circle(12., 12., 6.));
            }
            Icon::Retry => {
                stroke(
                    c,
                    p().move_to(19., 12.)
                        .cubic_to([19., 15.9], [15.9, 19.], [12., 19.])
                        .cubic_to([8.1, 19.], [5., 15.9], [5., 12.])
                        .cubic_to([5., 8.1], [8.1, 5.], [12., 5.])
                        .cubic_to([14.4, 5.], [16.5, 6.2], [17.8, 8.]),
                );
                stroke(c, p().move_to(18.5, 3.5).line_to(18., 8.5).line_to(13., 8.));
            }
            Icon::Heart => stroke(
                c,
                p().move_to(12., 19.5)
                    .cubic_to([5., 15.], [3.5, 11.5], [3.5, 9.])
                    .cubic_to([3.5, 6.5], [5.5, 4.5], [8., 4.5])
                    .cubic_to([9.7, 4.5], [11.2, 5.5], [12., 7.])
                    .cubic_to([12.8, 5.5], [14.3, 4.5], [16., 4.5])
                    .cubic_to([18.5, 4.5], [20.5, 6.5], [20.5, 9.])
                    .cubic_to([20.5, 11.5], [19., 15.], [12., 19.5])
                    .close(),
            ),
            Icon::Diff => {
                stroke(c, rounded(4., 3.5, 16., 17., 2.5));
                stroke(c, p().move_to(12., 7.5).line_to(12., 12.5));
                stroke(c, p().move_to(9.5, 10.).line_to(14.5, 10.));
                stroke(c, p().move_to(9.5, 16.).line_to(14.5, 16.));
            }
            Icon::Pin => {
                stroke(
                    c,
                    p().move_to(9., 4.)
                        .line_to(15., 4.)
                        .line_to(14., 10.)
                        .line_to(17., 13.5)
                        .line_to(7., 13.5)
                        .line_to(10., 10.)
                        .close(),
                );
                stroke(c, p().move_to(12., 13.5).line_to(12., 20.));
            }
            Icon::ChevronDown => {
                stroke(c, p().move_to(7., 10.).line_to(12., 15.).line_to(17., 10.))
            }
            Icon::ChevronRight => {
                stroke(c, p().move_to(10., 7.).line_to(15., 12.).line_to(10., 17.))
            }
            Icon::Spark => fill(
                c,
                p().move_to(12., 2.5)
                    .cubic_to([12.8, 8.], [16., 11.2], [21.5, 12.])
                    .cubic_to([16., 12.8], [12.8, 16.], [12., 21.5])
                    .cubic_to([11.2, 16.], [8., 12.8], [2.5, 12.])
                    .cubic_to([8., 11.2], [11.2, 8.], [12., 2.5])
                    .close(),
            ),
        }
    })
    .width(size)
    .height(size)
}

/// An icon-only button: a ghost button with a vector glyph layered on top.
/// The label is the accessible name, since there is no visible text.
pub fn icon_button(
    key: impl Into<String>,
    label: &str,
    glyph: Icon,
    size: f32,
    color: impl Into<String>,
    on_click: impl FnMut(&mut App) + 'static,
) -> Element<App> {
    let key = key.into();
    zstack![
        button("")
            .key(key.clone())
            .accessible_label(label)
            .variant(ButtonVariant::Ghost)
            .width(size)
            .height(size)
            .rounded(8.)
            .on_click(on_click),
        icon(glyph, (size * 0.55).round(), color),
    ]
    .key(format!("{key}-layer"))
    .width(size)
    .height(size)
    .flow(Flow::Center)
    .into()
}

/// A small filled spark inside a rounded tile: the app mark.
pub fn mark(size: f32) -> Element<App> {
    zstack![icon(Icon::Spark, size * 0.6, ColorToken::PrimaryContent)]
        .width(size)
        .height(size)
        .flow(Flow::Center)
        .background(ColorToken::Primary)
        .rounded(size * 0.28)
        .into()
}
