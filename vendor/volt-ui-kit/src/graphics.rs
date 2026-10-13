//! Renderer-independent image handles, vector paths and explicit clip geometry.
//! Assets are supplied by the host; this module never reads files or the network.
use std::sync::Arc;

/// Immutable premultiplied RGBA pixels. Clones share storage; at most 16 MP.
#[derive(Clone, Debug)]
pub struct Image(Arc<ImageData>);
#[derive(Debug)]
struct ImageData {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Image {
    /// Input is straight RGBA; conversion happens once, never during painting.
    pub fn from_rgba(width: u32, height: u32, mut pixels: Vec<u8>) -> Result<Self, &'static str> {
        let count = u64::from(width) * u64::from(height);
        if count == 0 || count > 16_777_216 || count * 4 != pixels.len() as u64 {
            return Err("invalid image dimensions/data (16 MP maximum)");
        }
        for p in pixels.chunks_exact_mut(4) {
            for i in 0..3 {
                p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
            }
        }
        Ok(Self(Arc::new(ImageData {
            width,
            height,
            pixels,
        })))
    }
    pub fn width(&self) -> u32 {
        self.0.width
    }
    pub fn height(&self) -> u32 {
        self.0.height
    }
    /// Premultiplied RGBA, suitable for renderer upload without decoding.
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
    pub fn background(self) -> BackgroundImage {
        BackgroundImage {
            image: self,
            fit: ImageFit::Cover,
            position: ImagePosition::Center,
            opacity: 1.,
        }
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageFit {
    #[default]
    Cover,
    Contain,
    Stretch,
    Tile,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImagePosition {
    #[default]
    Center,
    Top,
    Right,
    Bottom,
    Left,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
impl ImagePosition {
    pub fn fractions(self) -> [f32; 2] {
        match self {
            Self::Center => [0.5, 0.5],
            Self::Top => [0.5, 0.],
            Self::Right => [1., 0.5],
            Self::Bottom => [0.5, 1.],
            Self::Left => [0., 0.5],
            Self::TopLeft => [0., 0.],
            Self::TopRight => [1., 0.],
            Self::BottomLeft => [0., 1.],
            Self::BottomRight => [1., 1.],
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct BackgroundImage {
    pub image: Image,
    pub fit: ImageFit,
    pub position: ImagePosition,
    pub opacity: f32,
}
impl From<Image> for BackgroundImage {
    fn from(value: Image) -> Self {
        value.background()
    }
}
impl BackgroundImage {
    pub fn fit(mut self, fit: ImageFit) -> Self {
        self.fit = fit;
        self
    }
    pub fn position(mut self, position: ImagePosition) -> Self {
        self.position = position;
        self
    }
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum PathCommand {
    Move([f32; 2]),
    Line([f32; 2]),
    Quad([f32; 2], [f32; 2]),
    Cubic([f32; 2], [f32; 2], [f32; 2]),
    Close,
}
/// Most commands one [`VectorPath`] may hold.
pub const MAX_PATH_COMMANDS: usize = 1024;
/// Most paths one [`Canvas`] may hold.
pub const MAX_CANVAS_PATHS: usize = 256;

/// Paths use local logical coordinates. Fills/clips use the even-odd rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct VectorPath {
    pub commands: Vec<PathCommand>,
}
impl VectorPath {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Move([x, y]));
        self
    }
    pub fn line_to(mut self, x: f32, y: f32) -> Self {
        self.commands.push(PathCommand::Line([x, y]));
        self
    }
    pub fn quad_to(mut self, control: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Quad(control, end));
        self
    }
    pub fn cubic_to(mut self, a: [f32; 2], b: [f32; 2], end: [f32; 2]) -> Self {
        self.commands.push(PathCommand::Cubic(a, b, end));
        self
    }
    pub fn close(mut self) -> Self {
        self.commands.push(PathCommand::Close);
        self
    }
    pub fn rectangle(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self::new()
            .move_to(x, y)
            .line_to(x + w, y)
            .line_to(x + w, y + h)
            .line_to(x, y + h)
            .close()
    }
    pub fn circle(x: f32, y: f32, r: f32) -> Self {
        Self::ellipse(x, y, r, r)
    }
    pub fn ellipse(x: f32, y: f32, rx: f32, ry: f32) -> Self {
        let k = 0.552_284_8;
        Self::new()
            .move_to(x + rx, y)
            .cubic_to([x + rx, y + k * ry], [x + k * rx, y + ry], [x, y + ry])
            .cubic_to([x - k * rx, y + ry], [x - rx, y + k * ry], [x - rx, y])
            .cubic_to([x - rx, y - k * ry], [x - k * rx, y - ry], [x, y - ry])
            .cubic_to([x + k * rx, y - ry], [x + rx, y - k * ry], [x + rx, y])
            .close()
    }
    /// Parse SVG path data (the `d` attribute): M L H V C S Q T A Z, absolute
    /// and relative, with implicit repeats. Arcs become cubic curves. Use it
    /// to draw icons and logos exported from design tools.
    pub fn from_svg(d: &str) -> Result<Self, String> {
        svg::parse(d)
    }
    /// Affine matrix `[a, b, c, d, tx, ty]`. Can translate/scale/rotate paths in Rust.
    pub fn transformed(mut self, m: [f32; 6]) -> Self {
        let point = |p: &mut [f32; 2]| {
            let [x, y] = *p;
            *p = [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]];
        };
        for c in &mut self.commands {
            match c {
                PathCommand::Move(p) | PathCommand::Line(p) => point(p),
                PathCommand::Quad(a, b) => {
                    point(a);
                    point(b)
                }
                PathCommand::Cubic(a, b, c) => {
                    point(a);
                    point(b);
                    point(c)
                }
                PathCommand::Close => {}
            }
        }
        self
    }
    pub fn is_valid(&self) -> bool {
        let valid = |p: &[f32; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= 65_536.);
        !self.commands.is_empty()
            && self.commands.iter().any(|c| {
                matches!(
                    c,
                    PathCommand::Line(_) | PathCommand::Quad(..) | PathCommand::Cubic(..)
                )
            })
            && self.commands.len() <= MAX_PATH_COMMANDS
            && matches!(self.commands[0], PathCommand::Move(_))
            && self.commands.iter().all(|c| match c {
                PathCommand::Move(p) | PathCommand::Line(p) => valid(p),
                PathCommand::Quad(a, b) => valid(a) && valid(b),
                PathCommand::Cubic(a, b, c) => valid(a) && valid(b) && valid(c),
                PathCommand::Close => true,
            })
    }
    /// Even-odd interior. Curves are flattened at bounded resolution for input.
    pub fn contains(&self, x: f32, y: f32) -> bool {
        let mut inside = false;
        let mut prev = [0.; 2];
        let mut start = prev;
        let mut open = false;
        let edge = |a: [f32; 2], b: [f32; 2], inside: &mut bool| {
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                *inside = !*inside;
            }
        };
        for c in &self.commands {
            match *c {
                PathCommand::Move(p) => {
                    if open {
                        edge(prev, start, &mut inside)
                    };
                    prev = p;
                    start = p;
                    open = true
                }
                PathCommand::Line(p) => {
                    edge(prev, p, &mut inside);
                    prev = p
                }
                PathCommand::Quad(a, b) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * from[0] + 2. * u * t * a[0] + t * t * b[0],
                            u * u * from[1] + 2. * u * t * a[1] + t * t * b[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Cubic(a, b, c) => {
                    let from = prev;
                    for i in 1..=32 {
                        let t = i as f32 / 32.;
                        let u = 1. - t;
                        let p = [
                            u * u * u * from[0]
                                + 3. * u * u * t * a[0]
                                + 3. * u * t * t * b[0]
                                + t * t * t * c[0],
                            u * u * u * from[1]
                                + 3. * u * u * t * a[1]
                                + 3. * u * t * t * b[1]
                                + t * t * t * c[1],
                        ];
                        edge(prev, p, &mut inside);
                        prev = p
                    }
                }
                PathCommand::Close => {
                    edge(prev, start, &mut inside);
                    prev = start;
                    open = false
                }
            }
        }
        if open {
            edge(prev, start, &mut inside)
        };
        inside
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Clip {
    Bounds,
    Rounded,
    Circle,
    Path(VectorPath),
}
/// Resolved physical-pixel clip shared by rendering and interaction.
#[derive(Clone, Debug, PartialEq)]
pub enum ClipRegion {
    Rounded { bounds: [f32; 4], radii: [f32; 4] },
    Circle { bounds: [f32; 4] },
    Path(VectorPath),
}
impl ClipRegion {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        match self {
            Self::Rounded { bounds, radii } => {
                crate::Rect::new(bounds[0], bounds[1], bounds[2], bounds[3])
                    .is_some_and(|r| r.contains_corners(*radii, x, y))
            }
            Self::Circle {
                bounds: [bx, by, w, h],
            } => {
                let r = w.min(*h) * 0.5;
                (x - (bx + w * 0.5)).hypot(y - (by + h * 0.5)) < r
            }
            Self::Path(p) => p.contains(x, y),
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct DrawPath {
    pub path: VectorPath,
    pub color: String,
    pub stroke: Option<f32>,
}
/// A retained list built by an ordinary Rust closure when the view composes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Canvas {
    pub paths: Vec<DrawPath>,
}
impl Canvas {
    pub fn fill(&mut self, path: VectorPath, color: impl Into<String>) {
        self.paths.push(DrawPath {
            path,
            color: color.into(),
            stroke: None,
        });
    }
    pub fn stroke(&mut self, path: VectorPath, color: impl Into<String>, width: f32) {
        self.paths.push(DrawPath {
            path,
            color: color.into(),
            stroke: Some(width),
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_handles_share_storage_and_premultiply_once() {
        let image = Image::from_rgba(1, 1, vec![255, 128, 0, 128]).unwrap();
        assert_eq!(image.pixels(), [128, 64, 0, 128]);
        assert_eq!(image, image.clone());
        assert!(Image::from_rgba(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(Image::from_rgba(2, 2, vec![0; 4]).is_err());
    }
    #[test]
    fn paths_transform_even_odd_holes_and_validate_finite_commands() {
        let mut path = VectorPath::rectangle(0., 0., 100., 100.);
        path.commands
            .extend(VectorPath::rectangle(20., 20., 60., 60.).commands);
        assert!(path.contains(10., 10.));
        assert!(!path.contains(50., 50.));
        let path = path.transformed([2., 0., 0., 2., 10., 20.]);
        assert!(path.contains(30., 40.));
        assert!(!path.contains(110., 120.));
        assert!(!VectorPath::new().move_to(0., 0.).is_valid());
        assert!(!VectorPath::circle(0., 0., f32::NAN).is_valid());
        assert!(VectorPath::circle(50., 50., 25.).contains(50., 50.));
    }
}

mod svg {
    use super::{PathCommand, VectorPath};
    struct Lexer<'a> {
        s: &'a [u8],
        i: usize,
    }
    impl Lexer<'_> {
        fn skip(&mut self) {
            while self.i < self.s.len()
                && matches!(self.s[self.i], b' ' | b',' | b'\t' | b'\n' | b'\r')
            {
                self.i += 1;
            }
        }
        fn command(&mut self) -> Option<u8> {
            self.skip();
            let c = *self.s.get(self.i)?;
            if c.is_ascii_alphabetic() {
                self.i += 1;
                Some(c)
            } else {
                None
            }
        }
        fn at_number(&mut self) -> bool {
            self.skip();
            self.s
                .get(self.i)
                .is_some_and(|c| c.is_ascii_digit() || matches!(c, b'-' | b'+' | b'.'))
        }
        fn number(&mut self) -> Result<f32, String> {
            self.skip();
            let start = self.i;
            let mut seen_dot = false;
            let mut seen_exp = false;
            if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                self.i += 1;
            }
            while let Some(&c) = self.s.get(self.i) {
                match c {
                    b'0'..=b'9' => self.i += 1,
                    b'.' if !seen_dot && !seen_exp => {
                        seen_dot = true;
                        self.i += 1
                    }
                    b'e' | b'E' if !seen_exp => {
                        seen_exp = true;
                        self.i += 1;
                        if matches!(self.s.get(self.i), Some(b'-' | b'+')) {
                            self.i += 1;
                        }
                    }
                    _ => break,
                }
            }
            std::str::from_utf8(&self.s[start..self.i])
                .ok()
                .and_then(|t| t.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .ok_or_else(|| format!("invalid number at byte {start}"))
        }
        /// Arc flags may be written without separators ("a1 1 0 01 5 5").
        fn flag(&mut self) -> Result<bool, String> {
            self.skip();
            match self.s.get(self.i) {
                Some(b'0') => {
                    self.i += 1;
                    Ok(false)
                }
                Some(b'1') => {
                    self.i += 1;
                    Ok(true)
                }
                _ => Err(format!("invalid arc flag at byte {}", self.i)),
            }
        }
    }
    /// SVG arc (endpoint form) to cubic segments of at most 90 degrees.
    #[allow(clippy::too_many_arguments)]
    fn arc(
        out: &mut Vec<PathCommand>,
        from: [f32; 2],
        mut rx: f32,
        mut ry: f32,
        rotation: f32,
        large: bool,
        sweep: bool,
        to: [f32; 2],
    ) {
        if from == to {
            return;
        }
        rx = rx.abs();
        ry = ry.abs();
        if rx == 0. || ry == 0. {
            out.push(PathCommand::Line(to));
            return;
        }
        let (sin, cos) = rotation.to_radians().sin_cos();
        let dx = (from[0] - to[0]) * 0.5;
        let dy = (from[1] - to[1]) * 0.5;
        let x1 = cos * dx + sin * dy;
        let y1 = -sin * dx + cos * dy;
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
        if lambda > 1. {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }
        let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
        let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
        let mut coef = (num / den).max(0.).sqrt();
        if large == sweep {
            coef = -coef;
        }
        let cx1 = coef * rx * y1 / ry;
        let cy1 = -coef * ry * x1 / rx;
        let cx = cos * cx1 - sin * cy1 + (from[0] + to[0]) * 0.5;
        let cy = sin * cx1 + cos * cy1 + (from[1] + to[1]) * 0.5;
        let angle =
            |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
        let start = angle(1., 0., (x1 - cx1) / rx, (y1 - cy1) / ry);
        let mut delta = angle(
            (x1 - cx1) / rx,
            (y1 - cy1) / ry,
            (-x1 - cx1) / rx,
            (-y1 - cy1) / ry,
        );
        if !sweep && delta > 0. {
            delta -= std::f32::consts::TAU;
        } else if sweep && delta < 0. {
            delta += std::f32::consts::TAU;
        }
        let segments = (delta.abs() / std::f32::consts::FRAC_PI_2).ceil().max(1.) as usize;
        let step = delta / segments as f32;
        let k = 4. / 3. * (step / 4.).tan();
        let point = |t: f32| {
            let (st, ct) = t.sin_cos();
            [
                cx + rx * ct * cos - ry * st * sin,
                cy + rx * ct * sin + ry * st * cos,
            ]
        };
        let derivative = |t: f32| {
            let (st, ct) = t.sin_cos();
            [
                -rx * st * cos - ry * ct * sin,
                -rx * st * sin + ry * ct * cos,
            ]
        };
        for i in 0..segments {
            let (t0, t1) = (start + step * i as f32, start + step * (i + 1) as f32);
            let (p0, p1) = (point(t0), point(t1));
            let (d0, d1) = (derivative(t0), derivative(t1));
            let end = if i + 1 == segments { to } else { p1 };
            out.push(PathCommand::Cubic(
                [p0[0] + k * d0[0], p0[1] + k * d0[1]],
                [p1[0] - k * d1[0], p1[1] - k * d1[1]],
                end,
            ));
        }
    }
    pub(super) fn parse(d: &str) -> Result<VectorPath, String> {
        if d.len() > 1 << 20 {
            return Err("path data larger than 1 MiB".into());
        }
        let mut lx = Lexer {
            s: d.as_bytes(),
            i: 0,
        };
        let mut out = Vec::new();
        let (mut cur, mut start) = ([0_f32; 2], [0_f32; 2]);
        // Reflection points for S/s and T/t.
        let (mut last_cubic, mut last_quad): (Option<[f32; 2]>, Option<[f32; 2]>) = (None, None);
        let mut cmd = None;
        loop {
            lx.skip();
            if lx.i >= lx.s.len() {
                break;
            }
            if let Some(c) = lx.command() {
                cmd = Some(c);
            } else if cmd.is_none() {
                return Err("path data must start with a command".into());
            }
            let c = cmd.unwrap();
            let rel = c.is_ascii_lowercase();
            let base = if rel { cur } else { [0.; 2] };
            let pt = |lx: &mut Lexer| -> Result<[f32; 2], String> {
                Ok([lx.number()? + base[0], lx.number()? + base[1]])
            };
            let (mut next_cubic, mut next_quad) = (None, None);
            match c.to_ascii_uppercase() {
                b'M' => {
                    cur = pt(&mut lx)?;
                    start = cur;
                    out.push(PathCommand::Move(cur));
                    // Extra pairs after a move are implicit line-tos.
                    cmd = Some(if rel { b'l' } else { b'L' });
                }
                b'L' => {
                    cur = pt(&mut lx)?;
                    out.push(PathCommand::Line(cur));
                }
                b'H' => {
                    cur[0] = lx.number()? + if rel { cur[0] } else { 0. };
                    out.push(PathCommand::Line(cur));
                }
                b'V' => {
                    cur[1] = lx.number()? + if rel { cur[1] } else { 0. };
                    out.push(PathCommand::Line(cur));
                }
                b'C' => {
                    let (a, b, e) = (pt(&mut lx)?, pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Cubic(a, b, e));
                    next_cubic = Some(b);
                    cur = e;
                }
                b'S' => {
                    let a = last_cubic.map_or(cur, |p| [2. * cur[0] - p[0], 2. * cur[1] - p[1]]);
                    let (b, e) = (pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Cubic(a, b, e));
                    next_cubic = Some(b);
                    cur = e;
                }
                b'Q' => {
                    let (a, e) = (pt(&mut lx)?, pt(&mut lx)?);
                    out.push(PathCommand::Quad(a, e));
                    next_quad = Some(a);
                    cur = e;
                }
                b'T' => {
                    let a = last_quad.map_or(cur, |p| [2. * cur[0] - p[0], 2. * cur[1] - p[1]]);
                    let e = pt(&mut lx)?;
                    out.push(PathCommand::Quad(a, e));
                    next_quad = Some(a);
                    cur = e;
                }
                b'A' => {
                    let (rx, ry, rot) = (lx.number()?, lx.number()?, lx.number()?);
                    let (large, sweep) = (lx.flag()?, lx.flag()?);
                    let e = pt(&mut lx)?;
                    arc(&mut out, cur, rx, ry, rot, large, sweep, e);
                    cur = e;
                }
                b'Z' => {
                    out.push(PathCommand::Close);
                    cur = start;
                    // Z takes no arguments; a following number is an error.
                    if lx.at_number() {
                        return Err("unexpected number after Z".into());
                    }
                    cmd = None;
                }
                other => return Err(format!("unsupported path command '{}'", other as char)),
            }
            last_cubic = next_cubic;
            last_quad = next_quad;
            if out.len() > 100_000 {
                return Err("path has too many segments".into());
            }
            // A command letter must follow unless numbers repeat the command.
            if cmd.is_some() && !lx.at_number() {
                lx.skip();
                if lx.i < lx.s.len() && !lx.s[lx.i].is_ascii_alphabetic() {
                    return Err(format!("unexpected input at byte {}", lx.i));
                }
            }
        }
        let path = VectorPath { commands: out };
        if path.is_valid() {
            Ok(path)
        } else {
            Err("path coordinates out of range".into())
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn parses_commands_relative_forms_repeats_and_arcs() {
            let p = parse("M10 10 h20 v20 H10 Z m5 5 l1 1 2 2").unwrap();
            assert_eq!(p.commands[0], PathCommand::Move([10., 10.]));
            assert_eq!(p.commands[1], PathCommand::Line([30., 10.]));
            assert_eq!(p.commands[2], PathCommand::Line([30., 30.]));
            assert_eq!(p.commands[4], PathCommand::Close);
            // After Z the pen returns to the subpath start; m is relative to it.
            assert_eq!(p.commands[5], PathCommand::Move([15., 15.]));
            assert_eq!(p.commands[7], PathCommand::Line([18., 18.]));
            // Smooth curves reflect the previous control point.
            let s = parse("M0 0 C10 0 20 10 20 20 S30 40 40 40").unwrap();
            assert_eq!(
                s.commands[2],
                PathCommand::Cubic([20., 30.], [30., 40.], [40., 40.])
            );
            // A half-circle arc ends exactly at its endpoint in two quarter cubics.
            let a = parse("M0 10 A10 10 0 0 1 20 10").unwrap();
            assert_eq!(a.commands.len(), 3);
            let PathCommand::Cubic(_, _, end) = a.commands[2] else {
                panic!()
            };
            assert_eq!(end, [20., 10.]);
            // Compact flags and exponents, as minifiers write them.
            assert!(parse("M0 0a5 5 0 1010 0l1e1,2e0z").is_ok());
        }
        #[test]
        fn rejects_malformed_data() {
            for bad in ["10 10", "M10", "M0 0 X1 2", "M0 0 Z 5", "M0 0 L1e999 0"] {
                assert!(parse(bad).is_err(), "{bad}");
            }
        }
    }
}
