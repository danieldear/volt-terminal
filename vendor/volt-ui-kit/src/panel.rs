//! General-purpose rounded panel frame, with optional host-owned backdrop blur.
use crate::{geometry::Rect, surface::SurfaceStyle, Painter};
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackdropRequest {
    pub bounds: Rect,
    pub radius: f32,
    pub blur_radius: f32,
    pub saturation: f32,
    pub tint: [f32; 4],
}
#[derive(Clone, Copy, Debug)]
pub struct Panel {
    rect: Rect,
    radius: f32,
    scale: f32,
    style: SurfaceStyle,
}
impl Panel {
    /// Bounds are physical pixels; corner radius is logical pixels.
    pub fn new(rect: Rect, radius: f32, scale: f32, style: SurfaceStyle) -> Option<Self> {
        if !radius.is_finite()
            || radius < 0.
            || !scale.is_finite()
            || scale <= 0.
            || !(radius * scale).is_finite()
            || !(style.blur_radius * scale).is_finite()
            || !style.valid()
        {
            return None;
        }
        let [_, _, w, h] = rect.bounds();
        Some(Self {
            rect,
            radius: (radius * scale).min(w * 0.5).min(h * 0.5),
            scale,
            style,
        })
    }
    pub fn hit(self, x: f32, y: f32) -> bool {
        self.rect.contains_rounded(self.radius, x, y)
    }
    /// Consume only when the backend really supports backdrop blur. Otherwise
    /// call `paint`. Cache the blurred backdrop until its source pixels change.
    pub fn backdrop_request(self) -> Option<BackdropRequest> {
        (self.style.blur_radius > 0.).then_some(BackdropRequest {
            bounds: self.rect,
            radius: self.radius,
            blur_radius: self.style.blur_radius * self.scale,
            saturation: self.style.saturation,
            tint: self.style.fill,
        })
    }
    /// Backend-independent opaque fallback. Never pretends to provide blur.
    pub fn paint<P: Painter>(self, p: &P, shapes: &mut Vec<P::Shape>) {
        let [x, y, w, h] = self.rect.bounds();
        // A modest offset silhouette, not a Gaussian blur/shadow pipeline.
        p.card_round(
            shapes,
            [x, y + 3. * self.scale, w, h],
            self.radius,
            [0., 0., 0., self.style.shadow_alpha],
        );
        p.card_round(shapes, self.rect.bounds(), self.radius, self.style.fallback);
        self.paint_composited_frame(p, shapes);
    }
    /// Frame only, after the host composites the requested backdrop/tint.
    /// Border geometry does not overwrite the glass interior.
    pub fn paint_composited_frame<P: Painter>(self, p: &P, shapes: &mut Vec<P::Shape>) {
        let [x, y, w, h] = self.rect.bounds();
        let r = self.radius;
        let edge = |a, b| p.card_line(shapes, a, b, self.scale, self.style.border);
        // Use a local helper to avoid a mutable capture across the arc loop.
        let mut edge = edge;
        edge([x + r, y], [x + w - r, y]);
        edge([x + w, y + r], [x + w, y + h - r]);
        edge([x + w - r, y + h], [x + r, y + h]);
        edge([x, y + h - r], [x, y + r]);
        if r > 0. {
            for (cx, cy, start) in [
                (x + w - r, y + r, -90.),
                (x + w - r, y + h - r, 0.),
                (x + r, y + h - r, 90.),
                (x + r, y + r, 180.),
            ] {
                for i in 0..8 {
                    let a = (start + i as f32 * 11.25).to_radians();
                    let b = (start + (i + 1) as f32 * 11.25).to_radians();
                    edge(
                        [cx + r * a.cos(), cy + r * a.sin()],
                        [cx + r * b.cos(), cy + r * b.sin()],
                    );
                }
            }
        }
        if w > 2. * r && h > 2. * self.scale {
            p.draw_rect(
                shapes,
                x + r,
                y + self.scale,
                w - 2. * r,
                self.scale,
                self.style.highlight,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{surface::Tint, svg::SvgPainter, Viewport};
    #[test]
    fn invalid_inputs_do_not_produce_geometry() {
        let rect = Rect::new(0., 0., 100., 60.).unwrap();
        let s = SurfaceStyle::glass(Tint::Dark);
        for scale in [0., -1., f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(Panel::new(rect, 14., scale, s).is_none());
        }
        let mut invalid = s;
        invalid.fallback[3] = 0.5;
        assert!(Panel::new(rect, 14., 1., invalid).is_none());
    }
    #[test]
    fn requests_scale_once_and_reduced_transparency_disables_blur() {
        for scale in [1., 1.5, 2., 3.] {
            let rect = Rect::new(20. * scale, 20. * scale, 200. * scale, 120. * scale).unwrap();
            let s = SurfaceStyle::glass(Tint::Dark);
            let p = Panel::new(rect, 14., scale, s).unwrap();
            let request = p.backdrop_request().unwrap();
            assert_eq!(request.blur_radius, 26. * scale);
            assert_eq!(request.radius, 14. * scale);
            assert!(!p.hit(20. * scale, 20. * scale));
            assert!(p.hit(50. * scale, 50. * scale));
            assert!(Panel::new(rect, 14., scale, s.reduced_transparency())
                .unwrap()
                .backdrop_request()
                .is_none());
        }
    }
    #[test]
    fn fallback_is_opaque_and_frame_is_separate() {
        let painter = SvgPainter::new(Viewport::new(300., 200., 1.).unwrap()).unwrap();
        let panel = Panel::new(
            Rect::new(20., 20., 200., 120.).unwrap(),
            14.,
            1.,
            SurfaceStyle::glass(Tint::Dark),
        )
        .unwrap();
        let mut full = vec![];
        panel.paint(&painter, &mut full);
        let mut frame = vec![];
        panel.paint_composited_frame(&painter, &mut frame);
        assert!(full.len() > frame.len());
        let svg = painter.document(&full, &[], [0., 0., 0., 1.]);
        assert!(!svg.contains("NaN"));
        assert!(!svg.contains("<filter"));
    }
}
