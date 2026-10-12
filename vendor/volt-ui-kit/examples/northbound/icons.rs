//! Original line icons on a 24-unit grid, drawn with retained canvas paths.
use super::App;
use volt_ui_kit::ui::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Compass,
    Calendar,
    Journal,
    Sliders,
    Sun,
    Moon,
    Heart,
    HeartFilled,
    Bed,
    Ferry,
    Check,
    Close,
    Pin,
    Aurora,
    Cloud,
}

fn p() -> VectorPath {
    VectorPath::new()
}

/// Paths in a 24x24 box; the caller scales to the requested size.
fn paths(icon: Icon) -> (Vec<VectorPath>, Vec<VectorPath>) {
    let (mut stroke, mut fill) = (vec![], vec![]);
    match icon {
        Icon::Compass => {
            stroke.push(VectorPath::circle(12., 12., 9.));
            fill.push(
                p().move_to(15.5, 8.5)
                    .line_to(13.4, 13.4)
                    .line_to(8.5, 15.5)
                    .line_to(10.6, 10.6)
                    .close(),
            );
        }
        Icon::Calendar => {
            stroke.push(
                p().move_to(5., 6.)
                    .line_to(19., 6.)
                    .line_to(19., 19.)
                    .line_to(5., 19.)
                    .close(),
            );
            stroke.push(p().move_to(5., 10.).line_to(19., 10.));
            stroke.push(
                p().move_to(9., 4.)
                    .line_to(9., 7.)
                    .move_to(15., 4.)
                    .line_to(15., 7.),
            );
            fill.push(VectorPath::circle(9., 14.5, 1.2));
            fill.push(VectorPath::circle(12.5, 14.5, 1.2));
        }
        Icon::Journal => {
            stroke.push(
                p().move_to(6., 4.5)
                    .line_to(17., 4.5)
                    .line_to(17., 19.5)
                    .line_to(6., 19.5)
                    .close(),
            );
            stroke.push(p().move_to(9., 4.5).line_to(9., 19.5));
            stroke.push(
                p().move_to(11.5, 9.)
                    .line_to(15., 9.)
                    .move_to(11.5, 12.)
                    .line_to(14., 12.),
            );
        }
        Icon::Sliders => {
            stroke.push(
                p().move_to(4., 8.)
                    .line_to(20., 8.)
                    .move_to(4., 16.)
                    .line_to(20., 16.),
            );
            fill.push(VectorPath::circle(9., 8., 2.6));
            fill.push(VectorPath::circle(15., 16., 2.6));
        }
        Icon::Sun => {
            fill.push(VectorPath::circle(12., 12., 4.));
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let (s, c) = a.sin_cos();
                stroke.push(
                    p().move_to(12. + c * 7., 12. + s * 7.)
                        .line_to(12. + c * 9.5, 12. + s * 9.5),
                );
            }
        }
        Icon::Moon => {
            fill.push(
                p().move_to(15., 4.)
                    .cubic_to([9., 4.5], [5.5, 9.], [6., 13.5])
                    .cubic_to([6.6, 18.], [11., 20.5], [15.5, 19.5])
                    .cubic_to([17.6, 19.], [19., 18.], [20., 16.5])
                    .cubic_to([14., 17.5], [10., 12.], [15., 4.])
                    .close(),
            );
        }
        Icon::Heart | Icon::HeartFilled => {
            let heart = p()
                .move_to(12., 19.5)
                .cubic_to([5., 15.], [3.5, 11.5], [4.5, 8.5])
                .cubic_to([5.6, 5.5], [9.6, 4.8], [12., 8.])
                .cubic_to([14.4, 4.8], [18.4, 5.5], [19.5, 8.5])
                .cubic_to([20.5, 11.5], [19., 15.], [12., 19.5])
                .close();
            if icon == Icon::HeartFilled {
                fill.push(heart);
            } else {
                stroke.push(heart);
            }
        }
        Icon::Bed => {
            stroke.push(
                p().move_to(4., 7.)
                    .line_to(4., 18.)
                    .move_to(4., 14.)
                    .line_to(20., 14.)
                    .line_to(20., 18.),
            );
            stroke.push(
                p().move_to(10., 14.)
                    .line_to(10., 10.)
                    .line_to(18., 10.)
                    .cubic_to([19.5, 10.], [20., 11.], [20., 12.])
                    .line_to(20., 14.),
            );
            fill.push(VectorPath::circle(7., 11.5, 1.6));
        }
        Icon::Ferry => {
            stroke.push(
                p().move_to(3.5, 14.)
                    .line_to(20.5, 14.)
                    .line_to(18., 18.5)
                    .line_to(6., 18.5)
                    .close(),
            );
            stroke.push(
                p().move_to(7., 14.)
                    .line_to(7., 10.)
                    .line_to(16., 10.)
                    .line_to(16., 14.),
            );
            stroke.push(
                p().move_to(10., 10.)
                    .line_to(10., 6.5)
                    .line_to(13., 6.5)
                    .line_to(13., 10.),
            );
        }
        Icon::Check => {
            stroke.push(p().move_to(5., 12.5).line_to(10., 17.).line_to(19., 7.));
        }
        Icon::Close => {
            stroke.push(
                p().move_to(6., 6.)
                    .line_to(18., 18.)
                    .move_to(18., 6.)
                    .line_to(6., 18.),
            );
        }
        Icon::Pin => {
            stroke.push(
                p().move_to(12., 21.)
                    .cubic_to([7., 15.], [5.5, 12.], [5.5, 9.5])
                    .cubic_to([5.5, 5.8], [8.5, 3.], [12., 3.])
                    .cubic_to([15.5, 3.], [18.5, 5.8], [18.5, 9.5])
                    .cubic_to([18.5, 12.], [17., 15.], [12., 21.])
                    .close(),
            );
            fill.push(VectorPath::circle(12., 9.5, 2.4));
        }
        Icon::Aurora => {
            stroke.push(
                p().move_to(3., 15.)
                    .cubic_to([7., 6.], [11., 16.], [14., 9.])
                    .cubic_to([16., 5.], [19., 8.], [21., 6.]),
            );
            stroke.push(p().move_to(3., 20.).line_to(21., 20.));
            fill.push(VectorPath::circle(18., 14., 1.));
            fill.push(VectorPath::circle(7., 4.5, 0.9));
        }
        Icon::Cloud => {
            stroke.push(
                p().move_to(7., 18.)
                    .cubic_to([3.5, 18.], [3., 13.], [6.5, 12.5])
                    .cubic_to([7., 8.], [13., 6.5], [15., 10.5])
                    .cubic_to([19.5, 10.], [21., 15.], [18., 17.5])
                    .cubic_to([17.5, 17.9], [17., 18.], [16.5, 18.])
                    .close(),
            );
        }
    }
    (stroke, fill)
}

pub fn icon(icon: Icon, size: f32, color: impl Into<String>) -> Element<App> {
    let color = color.into();
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    let weight = (1.7 * s).max(1.2);
    canvas(move |c| {
        let (stroke, fill) = paths(icon);
        for path in stroke {
            c.stroke(path.transformed(m), color.clone(), weight);
        }
        for path in fill {
            c.fill(path.transformed(m), color.clone());
        }
    })
    .width(size)
    .height(size)
}

/// Brand mark: a compass needle over a horizon line.
pub fn mark(size: f32) -> Element<App> {
    let s = size / 32.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        c.fill(
            VectorPath::circle(16., 16., 15.).transformed(m),
            ColorToken::Primary,
        );
        c.fill(
            p().move_to(16., 5.)
                .line_to(20., 17.)
                .line_to(12., 17.)
                .close()
                .transformed(m),
            ColorToken::PrimaryContent,
        );
        c.fill(
            p().move_to(12., 17.)
                .line_to(20., 17.)
                .line_to(16., 27.)
                .close()
                .transformed(m),
            ColorToken::Secondary,
        );
    })
    .width(size)
    .height(size)
}
