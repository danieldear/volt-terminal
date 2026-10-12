//! Generated artwork and line icons; nothing is downloaded or bundled.
use super::App;
use volt_ui_kit::ui::{prelude::*, Image};

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let t = t.clamp(0., 1.);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// A soft dusk landscape: gradient sky, sun, two ridgelines and still water.
pub fn landscape(width: u32, height: u32) -> Image {
    let sky = ([0.20, 0.25, 0.45], [0.95, 0.62, 0.48]);
    let ridge = |u: f32, k: f32, seed: f32| {
        0.55 - k * (0.35 * (u * 7. + seed).sin().abs() + 0.15 * (u * 19. + seed * 2.).sin())
    };
    let mut pixels = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let (u, v) = (x as f32 / width as f32, y as f32 / height as f32);
            let horizon = 0.66;
            let sample = |v: f32| {
                let mut c = mix(sky.0, sky.1, v / horizon);
                let d = ((u - 0.68) * 1.6).hypot(v - 0.5);
                c = mix(c, [1., 0.9, 0.75], 1. - (d / 0.07).clamp(0., 1.));
                if v > ridge(u, 0.35, 1.3) {
                    c = mix(c, [0.36, 0.30, 0.45], 0.85);
                }
                if v > ridge(u, 0.22, 4.1) + 0.08 {
                    c = [0.17, 0.15, 0.26];
                }
                c
            };
            let c = if v > horizon {
                let mirrored = sample(2. * horizon - v + 0.004 * (v * 300.).sin());
                mix(mirrored, [0.12, 0.13, 0.25], 0.35 + (v - horizon))
            } else {
                sample(v)
            };
            pixels.extend(c.map(|v| (v.clamp(0., 1.) * 255.) as u8));
            pixels.push(255);
        }
    }
    Image::from_rgba(width, height, pixels).expect("bounded generated image")
}

/// Line icons on a 24-unit grid, scaled to `size` logical pixels.
#[derive(Clone, Copy)]
pub enum Icon {
    Search,
    Sun,
    Moon,
    Check,
    Bell,
    Spark,
    Layers,
    Chip,
    Person,
    Palette,
    Bolt,
    Window,
    Code,
    Arrow,
}
pub fn icon(glyph: Icon, size: f32, color: impl Into<String>) -> Element<App> {
    let color = color.into();
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        let p = VectorPath::new;
        let stroke = |c: &mut Canvas, path: VectorPath| {
            c.stroke(path.transformed(m), color.clone(), 1.8 * s)
        };
        match glyph {
            Icon::Search => {
                stroke(c, VectorPath::circle(10.5, 10.5, 6.5));
                stroke(c, p().move_to(15.5, 15.5).line_to(20.5, 20.5));
            }
            Icon::Sun => {
                stroke(c, VectorPath::circle(12., 12., 4.));
                for i in 0..8 {
                    let (sn, cs) = (i as f32 * std::f32::consts::FRAC_PI_4).sin_cos();
                    stroke(
                        c,
                        p().move_to(12. + cs * 7., 12. + sn * 7.)
                            .line_to(12. + cs * 9.5, 12. + sn * 9.5),
                    );
                }
            }
            Icon::Moon => stroke(
                c,
                p().move_to(15., 4.)
                    .cubic_to([9., 4.5], [5.5, 9.], [6., 13.5])
                    .cubic_to([6.6, 18.], [11., 20.5], [15.5, 19.5])
                    .cubic_to([17.6, 19.], [19., 18.], [20., 16.5])
                    .cubic_to([14., 17.5], [10., 12.], [15., 4.])
                    .close(),
            ),
            Icon::Check => stroke(c, p().move_to(5., 12.5).line_to(10., 17.).line_to(19., 7.)),
            Icon::Bell => {
                stroke(
                    c,
                    p().move_to(6., 17.)
                        .line_to(6., 11.)
                        .cubic_to([6., 7.5], [8.7, 5.], [12., 5.])
                        .cubic_to([15.3, 5.], [18., 7.5], [18., 11.])
                        .line_to(18., 17.)
                        .close(),
                );
                stroke(c, p().move_to(10., 20.).line_to(14., 20.));
            }
            Icon::Layers => {
                stroke(
                    c,
                    p().move_to(12., 4.)
                        .line_to(20.5, 8.5)
                        .line_to(12., 13.)
                        .line_to(3.5, 8.5)
                        .close(),
                );
                stroke(
                    c,
                    p().move_to(3.5, 12.5).line_to(12., 17.).line_to(20.5, 12.5),
                );
                stroke(
                    c,
                    p().move_to(3.5, 16.).line_to(12., 20.5).line_to(20.5, 16.),
                );
            }
            Icon::Chip => {
                stroke(
                    c,
                    p().move_to(7., 6.)
                        .line_to(17., 6.)
                        .quad_to([18., 6.], [18., 7.])
                        .line_to(18., 17.)
                        .quad_to([18., 18.], [17., 18.])
                        .line_to(7., 18.)
                        .quad_to([6., 18.], [6., 17.])
                        .line_to(6., 7.)
                        .quad_to([6., 6.], [7., 6.])
                        .close(),
                );
                stroke(c, VectorPath::rectangle(9.5, 9.5, 5., 5.));
                for k in [9.5, 14.5] {
                    stroke(c, p().move_to(k, 2.5).line_to(k, 6.));
                    stroke(c, p().move_to(k, 18.).line_to(k, 21.5));
                    stroke(c, p().move_to(2.5, k).line_to(6., k));
                    stroke(c, p().move_to(18., k).line_to(21.5, k));
                }
            }
            Icon::Person => {
                stroke(c, VectorPath::circle(12., 5.5, 2.));
                stroke(c, p().move_to(5., 9.).line_to(19., 9.));
                stroke(c, p().move_to(12., 9.).line_to(12., 14.5));
                stroke(
                    c,
                    p().move_to(8.5, 21.).line_to(12., 14.5).line_to(15.5, 21.),
                );
            }
            Icon::Palette => {
                stroke(
                    c,
                    p().move_to(12., 3.5)
                        .cubic_to([7., 3.5], [3.5, 7.3], [3.5, 12.])
                        .cubic_to([3.5, 16.7], [7.3, 20.5], [12., 20.5])
                        .cubic_to([13.4, 20.5], [14., 19.6], [13.6, 18.5])
                        .cubic_to([13.1, 17.2], [13.8, 16.], [15.2, 16.])
                        .line_to(17., 16.)
                        .cubic_to([19., 16.], [20.5, 14.5], [20.5, 12.])
                        .cubic_to([20.5, 7.3], [16.7, 3.5], [12., 3.5])
                        .close(),
                );
                for (x, y) in [(8., 11.), (10.5, 7.5), (14.5, 7.5), (17., 11.)] {
                    c.fill(VectorPath::circle(x, y, 1.2).transformed(m), color.clone());
                }
            }
            Icon::Bolt => stroke(
                c,
                p().move_to(13.5, 3.)
                    .line_to(5.5, 13.5)
                    .line_to(11.5, 13.5)
                    .line_to(10.5, 21.)
                    .line_to(18.5, 10.5)
                    .line_to(12.5, 10.5)
                    .close(),
            ),
            Icon::Window => {
                stroke(c, VectorPath::rectangle(3.5, 5., 17., 14.));
                stroke(c, p().move_to(3.5, 9.).line_to(20.5, 9.));
                for x in [6., 8.5] {
                    c.fill(VectorPath::circle(x, 7., 0.9).transformed(m), color.clone());
                }
            }
            Icon::Code => {
                stroke(c, p().move_to(8.5, 7.).line_to(3.5, 12.).line_to(8.5, 17.));
                stroke(
                    c,
                    p().move_to(15.5, 7.).line_to(20.5, 12.).line_to(15.5, 17.),
                );
                stroke(c, p().move_to(13.5, 5.).line_to(10.5, 19.));
            }
            Icon::Arrow => {
                stroke(c, p().move_to(5., 12.).line_to(19., 12.));
                stroke(
                    c,
                    p().move_to(13.5, 6.5).line_to(19., 12.).line_to(13.5, 17.5),
                );
            }
            Icon::Spark => {
                c.fill(
                    p().move_to(12., 3.)
                        .line_to(14., 10.)
                        .line_to(21., 12.)
                        .line_to(14., 14.)
                        .line_to(12., 21.)
                        .line_to(10., 14.)
                        .line_to(3., 12.)
                        .line_to(10., 10.)
                        .close()
                        .transformed(m),
                    color.clone(),
                );
            }
        }
    })
    .width(size)
    .height(size)
}

/// The catalog mark: a rounded square holding a lightning stroke.
pub fn mark(size: f32) -> Element<App> {
    let s = size / 24.;
    let m = [s, 0., 0., s, 0., 0.];
    canvas(move |c| {
        let square = VectorPath::new()
            .move_to(6., 0.)
            .line_to(18., 0.)
            .cubic_to([21.3, 0.], [24., 2.7], [24., 6.])
            .line_to(24., 18.)
            .cubic_to([24., 21.3], [21.3, 24.], [18., 24.])
            .line_to(6., 24.)
            .cubic_to([2.7, 24.], [0., 21.3], [0., 18.])
            .line_to(0., 6.)
            .cubic_to([0., 2.7], [2.7, 0.], [6., 0.])
            .close();
        c.fill(square.transformed(m), ColorToken::Primary);
        c.fill(
            VectorPath::new()
                .move_to(13.5, 4.)
                .line_to(7., 13.5)
                .line_to(11.5, 13.5)
                .line_to(10.5, 20.)
                .line_to(17., 10.5)
                .line_to(12.5, 10.5)
                .close()
                .transformed(m),
            ColorToken::PrimaryContent,
        );
    })
    .width(size)
    .height(size)
}

// ── Landing hero artwork ────────────────────────────────────────────────

/// A heavy, extended custom wordmark — V O L T — drawn as vector shapes so it
/// needs no installed display face and stays crisp at any size.
pub fn wordmark(height: f32, color: impl Into<String>) -> Element<App> {
    let color = color.into();
    let s = height / 100.;
    // Letter outlines on a 100-unit cap height, with their advance widths.
    let letters: Vec<(f32, VectorPath)> = vec![
        (
            124.,
            VectorPath::new()
                .move_to(0., 0.)
                .line_to(28., 0.)
                .line_to(62., 74.)
                .line_to(96., 0.)
                .line_to(124., 0.)
                .line_to(76., 100.)
                .line_to(48., 100.)
                .close(),
        ),
        (156., stadium_ring(0., 0., 156., 100., 25.)),
        (
            104.,
            VectorPath::new()
                .move_to(0., 0.)
                .line_to(25., 0.)
                .line_to(25., 76.)
                .line_to(104., 76.)
                .line_to(104., 100.)
                .line_to(0., 100.)
                .close(),
        ),
        (
            124.,
            VectorPath::new()
                .move_to(0., 0.)
                .line_to(124., 0.)
                .line_to(124., 24.)
                .line_to(74.5, 24.)
                .line_to(74.5, 100.)
                .line_to(49.5, 100.)
                .line_to(49.5, 24.)
                .line_to(0., 24.)
                .close(),
        ),
    ];
    let gap = 16.;
    let total: f32 = letters.iter().map(|(w, _)| w).sum::<f32>() + gap * (letters.len() - 1) as f32;
    canvas(move |c| {
        let mut x = 0.;
        for (advance, path) in &letters {
            c.fill(
                path.clone().transformed([s, 0., 0., s, x * s, 0.]),
                color.clone(),
            );
            x += advance + gap;
        }
    })
    .width(total * s)
    .height(height)
}

/// A wide rounded ring: outer and inner stadiums as one even-odd path.
fn stadium_ring(x: f32, y: f32, w: f32, h: f32, stroke: f32) -> VectorPath {
    let stadium = |p: VectorPath, x: f32, y: f32, w: f32, h: f32| {
        let r = h * 0.5;
        let k = 0.5523 * r;
        p.move_to(x + r, y)
            .line_to(x + w - r, y)
            .cubic_to([x + w - r + k, y], [x + w, y + r - k], [x + w, y + r])
            .cubic_to(
                [x + w, y + r + k],
                [x + w - r + k, y + h],
                [x + w - r, y + h],
            )
            .line_to(x + r, y + h)
            .cubic_to([x + r - k, y + h], [x, y + r + k], [x, y + r])
            .cubic_to([x, y + r - k], [x + r - k, y], [x + r, y])
            .close()
    };
    let outer = stadium(VectorPath::new(), x, y, w, h);
    stadium(
        outer,
        x + stroke,
        y + stroke,
        w - stroke * 2.,
        h - stroke * 2.,
    )
}

/// Circular arc as cubic segments (each ≤ 90°), in degrees, clockwise.
fn arc(p: VectorPath, cx: f32, cy: f32, r: f32, from: f32, to: f32, start: bool) -> VectorPath {
    let segments = ((to - from).abs() / 90.).ceil().max(1.) as usize;
    let step = (to - from) / segments as f32;
    let point = |deg: f32| {
        let a = deg.to_radians();
        [cx + r * a.cos(), cy + r * a.sin()]
    };
    let mut p = if start {
        let [x, y] = point(from);
        p.move_to(x, y)
    } else {
        p
    };
    for i in 0..segments {
        let (a0, a1) = (from + step * i as f32, from + step * (i + 1) as f32);
        let k = 4. / 3. * ((a1 - a0).to_radians() / 4.).tan() * r;
        let (s0, c0) = a0.to_radians().sin_cos();
        let (s1, c1) = a1.to_radians().sin_cos();
        let [x0, y0] = point(a0);
        let [x1, y1] = point(a1);
        p = p.cubic_to(
            [x0 - k * s0, y0 + k * c0],
            [x1 + k * s1, y1 - k * c1],
            [x1, y1],
        );
    }
    p
}

/// A hairline orbit in the text colour, with one accent arc fading toward
/// its tail. Theme tokens, so it follows light/dark and the accent.
pub fn orbit(size: f32, radius: f32, sweep: f32, start: f32) -> Element<App> {
    canvas(move |c| {
        c.stroke(
            arc(
                VectorPath::new(),
                size * 0.5,
                size * 0.5,
                radius,
                0.,
                360.,
                true,
            ),
            ColorToken::BaseContent.alpha(7),
            1.,
        );
        let steps = 12;
        for i in 0..steps {
            let t = i as f32 / steps as f32;
            let from = start + sweep * t;
            c.stroke(
                arc(
                    VectorPath::new(),
                    size * 0.5,
                    size * 0.5,
                    radius,
                    from,
                    from + sweep / steps as f32 + 0.6,
                    true,
                ),
                ColorToken::Primary.alpha((t * t * 70.) as u8 + 4),
                1.5,
            );
        }
    })
    .width(size)
    .height(size)
}
