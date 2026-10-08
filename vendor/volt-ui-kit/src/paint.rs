//! Static-dispatch drawing adapter. Shapes and glyphs stay in host-owned buffers.
use crate::workspace_card::CardIcon;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub top: f32,
}
impl Viewport {
    pub fn new(width: f32, height: f32, scale: f32) -> Option<Self> {
        (width.is_finite()
            && height.is_finite()
            && scale.is_finite()
            && width > 0.
            && height > 0.
            && scale > 0.)
            .then_some(Self {
                width,
                height,
                scale,
                top: 0.,
            })
    }
}
#[allow(clippy::too_many_arguments)]
pub trait Painter: Sized {
    type Shape;
    type Glyph;
    fn viewport(&self) -> Viewport;
    fn draw_rect(
        &self,
        out: &mut Vec<Self::Shape>,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    );
    fn card_triangle(&self, out: &mut Vec<Self::Shape>, points: [[f32; 2]; 3], color: [f32; 4]);
    /// Text coordinates match Volt: top origin, 1.2-em line height.
    fn card_text(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    );
    /// Color lookup uses UTF-8 byte offsets, not character indexes.
    fn draw_text_colored(
        &mut self,
        out: &mut Vec<Self::Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        line_height: f32,
        color_at: &dyn Fn(usize) -> [f32; 4],
    );
    fn approx_text_width(text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * size * 0.6
    }
    fn card_line(
        &self,
        bg: &mut Vec<Self::Shape>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = dx.hypot(dy).max(0.001);
        let n = [-dy / len * width / 2., dx / len * width / 2.];
        let p = [a[0] + n[0], a[1] + n[1]];
        let q = [a[0] - n[0], a[1] - n[1]];
        let r = [b[0] + n[0], b[1] + n[1]];
        let t = [b[0] - n[0], b[1] - n[1]];
        self.card_triangle(bg, [p, q, r], color);
        self.card_triangle(bg, [q, t, r], color);
    }

    fn card_round(&self, bg: &mut Vec<Self::Shape>, rect: [f32; 4], radius: f32, color: [f32; 4]) {
        let [x, y, w, h] = rect;
        let r = radius.min(w / 2.).min(h / 2.);
        self.draw_rect(bg, x + r, y, w - 2. * r, h, color);
        self.draw_rect(bg, x, y + r, r, h - 2. * r, color);
        self.draw_rect(bg, x + w - r, y + r, r, h - 2. * r, color);
        for (cx, cy, start) in [
            (x + r, y + r, 180_f32),
            (x + w - r, y + r, 270.),
            (x + w - r, y + h - r, 0.),
            (x + r, y + h - r, 90.),
        ] {
            for i in 0..16 {
                let a = (start + i as f32 * 90. / 16.).to_radians();
                let b = (start + (i + 1) as f32 * 90. / 16.).to_radians();
                self.card_triangle(
                    bg,
                    [
                        [cx, cy],
                        [cx + r * a.cos(), cy + r * a.sin()],
                        [cx + r * b.cos(), cy + r * b.sin()],
                    ],
                    color,
                );
            }
        }
    }

    fn card_icon(
        &self,
        bg: &mut Vec<Self::Shape>,
        icon: CardIcon,
        x: f32,
        y: f32,
        s: f32,
        c: [f32; 4],
    ) {
        // A coherent 16-unit outline family. No dependency on terminal font glyphs.
        let paths: &[&[[f32; 2]]] = match icon {
            CardIcon::Notice => &[],
            CardIcon::Folder => &[&[
                [1., 4.],
                [6., 4.],
                [8., 6.],
                [15., 6.],
                [15., 13.],
                [1., 13.],
                [1., 4.],
            ]],
            CardIcon::Branch => &[
                &[[4., 2.], [4., 14.]],
                &[[4., 10.], [12., 6.], [12., 2.]],
                &[[2., 2.], [6., 2.]],
                &[[10., 2.], [14., 2.]],
            ],
            CardIcon::File => &[
                &[
                    [3., 1.],
                    [10., 1.],
                    [14., 5.],
                    [14., 15.],
                    [3., 15.],
                    [3., 1.],
                ],
                &[[10., 1.], [10., 5.], [14., 5.]],
            ],
            CardIcon::Play => &[&[[4., 2.], [13., 8.], [4., 14.], [4., 2.]]],
            CardIcon::Search => &[
                &[
                    [6.5, 1.5],
                    [10., 3.],
                    [11.5, 6.5],
                    [10., 10.],
                    [6.5, 11.5],
                    [3., 10.],
                    [1.5, 6.5],
                    [3., 3.],
                    [6.5, 1.5],
                ],
                &[[10., 10.], [14.5, 14.5]],
            ],
            CardIcon::Plus => &[&[[8., 2.], [8., 14.]], &[[2., 8.], [14., 8.]]],
            CardIcon::Edit => &[
                &[
                    [2., 14.],
                    [3., 10.],
                    [11., 2.],
                    [14., 5.],
                    [6., 13.],
                    [2., 14.],
                ],
                &[[9., 4.], [12., 7.]],
            ],
            CardIcon::Agent => &[
                &[[8., 0.], [8., 3.]],
                &[[2., 4.], [14., 4.], [14., 13.], [2., 13.], [2., 4.]],
                &[[5., 7.], [5., 9.]],
                &[[11., 7.], [11., 9.]],
            ],
            CardIcon::Link => &[
                &[
                    [6., 11.],
                    [3., 14.],
                    [0., 11.],
                    [0., 8.],
                    [5., 3.],
                    [8., 3.],
                ],
                &[
                    [8., 13.],
                    [11., 13.],
                    [16., 8.],
                    [16., 5.],
                    [13., 2.],
                    [10., 5.],
                ],
                &[[5., 11.], [11., 5.]],
            ],
            CardIcon::Server => &[
                &[[1., 2.], [15., 2.], [15., 14.], [1., 14.], [1., 2.]],
                &[[1., 8.], [15., 8.]],
                &[[4., 5.], [6., 5.]],
                &[[4., 11.], [6., 11.]],
            ],
            CardIcon::Pin | CardIcon::Unpin => &[
                &[
                    [5., 2.],
                    [11., 2.],
                    [10., 7.],
                    [13., 10.],
                    [3., 10.],
                    [6., 7.],
                    [5., 2.],
                ],
                &[[8., 10.], [8., 15.]],
            ],
            CardIcon::Minimize => &[&[[3., 8.], [13., 8.]]],
            CardIcon::Expand => &[&[[3., 8.], [13., 8.]], &[[8., 3.], [8., 13.]]],
            CardIcon::Check => &[&[[2., 8.], [6., 12.], [14., 4.]]],
            CardIcon::ArrowLeft => &[&[[10., 3.], [5., 8.], [10., 13.]]],
            CardIcon::ArrowRight => &[&[[6., 3.], [11., 8.], [6., 13.]]],
            CardIcon::Close => &[&[[4., 4.], [12., 12.]], &[[12., 4.], [4., 12.]]],
            CardIcon::Refresh => &[
                &[
                    [13., 6.],
                    [11., 2.],
                    [5., 2.],
                    [2., 5.],
                    [2., 11.],
                    [5., 14.],
                    [11., 14.],
                    [14., 11.],
                ],
                &[[9., 6.], [14., 6.], [14., 1.]],
            ],
        };
        let transform = |p: [f32; 2]| {
            let [px, py] = if icon == CardIcon::Unpin {
                let a = std::f32::consts::FRAC_1_SQRT_2;
                [
                    (p[0] - 8.) * a - (p[1] - 8.) * a + 8.,
                    (p[0] - 8.) * a + (p[1] - 8.) * a + 8.,
                ]
            } else {
                p
            };
            [x + px * s, y + py * s]
        };
        for path in paths {
            for p in path.windows(2) {
                self.card_line(bg, transform(p[0]), transform(p[1]), 1.35 * s, c);
            }
        }
    }
}
