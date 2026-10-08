//! Dependency-light SVG backend for documentation, snapshots and component galleries.
//! Native applications should implement `Painter` using their own text/GPU backend.
use crate::{Painter, Viewport};
use std::fmt::Write;

pub struct SvgPainter {
    viewport: Viewport,
}
impl SvgPainter {
    pub fn new(viewport: Viewport) -> Option<Self> {
        Viewport::new(viewport.width, viewport.height, viewport.scale)?;
        if !viewport.top.is_finite() || viewport.top < 0. || viewport.top >= viewport.height {
            return None;
        }
        Some(Self { viewport })
    }
    pub fn document(&self, shapes: &[String], text: &[String], background: [f32; 4]) -> String {
        let v = self.viewport;
        self.document_region(shapes, text, background, [0., 0., v.width, v.height])
            .unwrap()
    }
    /// Crop a documentation scene without rebuilding or resizing its components.
    pub fn document_region(
        &self,
        shapes: &[String],
        text: &[String],
        background: [f32; 4],
        region: [f32; 4],
    ) -> Option<String> {
        let [x, y, w, h] = region;
        if !region.into_iter().all(f32::is_finite) || w <= 0. || h <= 0. {
            return None;
        }
        Some(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{x} {y} {w} {h}"><rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{}"/>{}{}</svg>"#,
            color(background),
            shapes.concat(),
            text.concat()
        ))
    }
}
fn color(c: [f32; 4]) -> String {
    format!(
        "rgba({},{},{},{:.4})",
        (c[0].clamp(0., 1.) * 255.).round() as u8,
        (c[1].clamp(0., 1.) * 255.).round() as u8,
        (c[2].clamp(0., 1.) * 255.).round() as u8,
        if c[3].is_finite() {
            c[3].clamp(0., 1.)
        } else {
            0.
        }
    )
}
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .fold(String::new(), |mut out, c| {
            out.push_str(match c {
                '&' => "&amp;",
                '<' => "&lt;",
                '>' => "&gt;",
                '"' => "&quot;",
                '\'' => "&apos;",
                _ => {
                    out.push(c);
                    return out;
                }
            });
            out
        })
}
impl Painter for SvgPainter {
    type Shape = String;
    type Glyph = String;
    fn viewport(&self) -> Viewport {
        self.viewport
    }
    fn draw_rect(&self, out: &mut Vec<String>, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        out.push(format!(
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{}"/>"#,
            color(c)
        ));
    }
    fn card_triangle(&self, out: &mut Vec<String>, points: [[f32; 2]; 3], c: [f32; 4]) {
        out.push(format!(
            r#"<polygon points="{},{} {},{} {},{}" fill="{}"/>"#,
            points[0][0],
            points[0][1],
            points[1][0],
            points[1][1],
            points[2][0],
            points[2][1],
            color(c)
        ));
    }
    fn card_text(
        &mut self,
        out: &mut Vec<String>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        c: [f32; 4],
    ) {
        out.push(format!(r#"<text x="{x}" y="{}" font-size="{size}" font-family="SFMono-Regular,Consolas,monospace" xml:space="preserve" fill="{}">{}</text>"#, y + size, color(c), escape(text)));
    }
    fn draw_text_colored(
        &mut self,
        out: &mut Vec<String>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        _line_height: f32,
        color_at: &dyn Fn(usize) -> [f32; 4],
    ) {
        let mut spans = String::new();
        for (i, c) in text.char_indices() {
            let _ = write!(
                spans,
                r#"<tspan fill="{}">{}</tspan>"#,
                color(color_at(i)),
                escape(&c.to_string())
            );
        }
        out.push(format!(r#"<text x="{x}" y="{}" font-size="{size}" font-family="SFMono-Regular,Consolas,monospace" xml:space="preserve">{spans}</text>"#,y+size));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exported_text_cannot_inject_svg_markup() {
        let mut p = SvgPainter::new(Viewport::new(800., 600., 1.).unwrap()).unwrap();
        let mut out = vec![];
        p.card_text(
            &mut out,
            "<script>alert('x')</script> & \"",
            0.,
            0.,
            12.,
            [1.; 4],
        );
        assert!(!out[0].contains("<script>"));
        assert!(out[0].contains("&lt;script&gt;"));
        assert!(out[0].contains("&amp;"));
    }
    #[test]
    fn highlights_use_byte_offsets_for_unicode() {
        let mut p = SvgPainter::new(Viewport::new(800., 600., 1.).unwrap()).unwrap();
        let mut out = vec![];
        let seen = std::cell::RefCell::new(vec![]);
        p.draw_text_colored(&mut out, "aé界", 0., 0., 12., 1.2, &|i| {
            seen.borrow_mut().push(i);
            [1.; 4]
        });
        assert_eq!(*seen.borrow(), vec![0, 1, 3]);
    }
    #[test]
    fn invalid_viewports_are_rejected() {
        for n in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(Viewport::new(n, 600., 1.).is_none());
            assert!(Viewport::new(800., 600., n).is_none());
        }
    }
}
