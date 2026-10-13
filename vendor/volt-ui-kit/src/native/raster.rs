use crate::graphics::{BackgroundImage, ClipRegion, ImageFit, PathCommand, VectorPath};
use crate::{
    declarative::{DocumentPainter, PaintBatch, TextMetrics, TextStyle},
    Painter, Viewport,
};
use cosmic_text::{
    Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight, Wrap,
};
use std::collections::BTreeMap;
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Stroke, Transform};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, PartialEq)]
pub enum Shape {
    Rectangle([f32; 4], [f32; 4]),
    Triangle([[f32; 2]; 3], [f32; 4]),
    Rounded([f32; 4], f32, [f32; 4]),
    Line([f32; 2], [f32; 2], f32, [f32; 4]),
    Corners([f32; 4], [f32; 4], [f32; 4]),
    Outline([f32; 4], [f32; 4], f32, [f32; 4]),
    Shadow([f32; 4], [f32; 4], f32, f32, [f32; 2], [f32; 4]),
    LayerBegin(f32),
    LayerEnd,
    ClipBegin(ClipRegion),
    ClipEnd,
    Image([f32; 4], BackgroundImage, f32),
    Path(VectorPath, [f32; 4], Option<f32>),
    /// A gradient-filled rounded rectangle: bounds, corner radii, geometry and
    /// resolved (offset, colour) stops, all in physical pixels.
    Gradient([f32; 4], [f32; 4], GradientGeometry, Vec<(f32, [f32; 4])>),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GradientGeometry {
    Linear { start: [f32; 2], end: [f32; 2] },
    Radial { centre: [f32; 2], radius: f32 },
}
impl GradientGeometry {
    /// CSS geometry for a box: a linear line through the centre whose length
    /// lets the end colours reach the corners; a radial circle reaching the
    /// farthest corner.
    pub(crate) fn for_box(
        kind: crate::declarative::extras::GradientKind,
        [x, y, w, h]: [f32; 4],
    ) -> Self {
        use crate::declarative::extras::GradientKind;
        match kind {
            GradientKind::Linear(angle) => {
                let (sin, cos) = angle.to_radians().sin_cos();
                let (dx, dy) = (sin, -cos);
                let half = (w * sin.abs() + h * cos.abs()) * 0.5;
                let (cx, cy) = (x + w * 0.5, y + h * 0.5);
                Self::Linear {
                    start: [cx - dx * half, cy - dy * half],
                    end: [cx + dx * half, cy + dy * half],
                }
            }
            GradientKind::Radial([fx, fy]) => {
                let centre = [x + w * fx, y + h * fy];
                let radius = [[x, y], [x + w, y], [x, y + h], [x + w, y + h]]
                    .iter()
                    .map(|c| (c[0] - centre[0]).hypot(c[1] - centre[1]))
                    .fold(1_f32, f32::max);
                Self::Radial { centre, radius }
            }
        }
    }
}

const EFFECT_CACHE_BYTES: usize = 8 * 1024 * 1024;
const EFFECT_CACHE_ENTRIES: usize = 32;
const EFFECT_TINT_BYTES: usize = 32 * 1024 * 1024;
const MAX_EFFECT_PIXELS: u64 = 4_194_304;
const MAX_FRAME_EFFECT_PIXELS: u64 = 16_777_216;
/// Shaped text layouts kept between frames: comfortably above a dense
/// document page plus navigation, measured at several widths.
const TEXT_CACHE_ENTRIES: usize = 2048;
/// Largest window framebuffer either renderer accepts: a 6K display at 2×
/// is ~20 MP and 8K at 1× ~33 MP. The GPU's own texture-dimension limit and
/// the runtime's 16,384-pixel side limit are enforced separately.
pub(super) const MAX_FRAMEBUFFER_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_LAYER_BYTES: u64 = 64 * 1024 * 1024;
const MAX_LAYER_DEPTH: usize = 24;

/// Validate the group stream before allocating its framebuffers. Large valid
/// groups use small tiles; malformed streams still fail rather than silently
/// losing group opacity or clipping.
fn tiled_opacity_needed(
    batches: &[PaintBatch<Shape, Glyph>],
    width: u32,
    height: u32,
) -> Result<bool, String> {
    let (mut depth, mut peak) = (0usize, 0usize);
    for shape in batches.iter().flat_map(|batch| &batch.shapes) {
        match shape {
            Shape::LayerBegin(opacity) => {
                if !opacity.is_finite() || !(0. ..=1.).contains(opacity) {
                    return Err("invalid layer opacity".into());
                }
                depth += 1;
                if depth > MAX_LAYER_DEPTH {
                    return Err("opacity layer depth exceeds limit".into());
                }
                peak = peak.max(depth);
            }
            Shape::LayerEnd => {
                depth = depth.checked_sub(1).ok_or("unbalanced opacity layer end")?;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err("unbalanced opacity layer begin".into());
    }
    Ok(peak > 0 && u64::from(width) * u64::from(height) * 4 * (peak as u64 + 1) > MAX_LAYER_BYTES)
}

fn translated_batch(
    batch: &PaintBatch<Shape, Glyph>,
    dx: f32,
    dy: f32,
) -> PaintBatch<Shape, Glyph> {
    let bounds = |b: &mut [f32; 4]| {
        b[0] += dx;
        b[1] += dy;
    };
    let point = |p: &mut [f32; 2]| {
        p[0] += dx;
        p[1] += dy;
    };
    let mut result = batch.clone();
    let [x, y, w, h] = batch.clip.bounds();
    result.clip = crate::Rect::new(x + dx, y + dy, w, h).expect("translation preserves dimensions");
    for shape in &mut result.shapes {
        match shape {
            Shape::Rectangle(b, _)
            | Shape::Rounded(b, _, _)
            | Shape::Corners(b, _, _)
            | Shape::Outline(b, _, _, _)
            | Shape::Shadow(b, _, _, _, _, _)
            | Shape::Gradient(b, _, _, _)
            | Shape::Image(b, _, _) => bounds(b),
            Shape::Triangle(points, _) => points.iter_mut().for_each(point),
            Shape::Line(a, b, _, _) => {
                point(a);
                point(b);
            }
            Shape::Path(path, _, _) | Shape::ClipBegin(ClipRegion::Path(path)) => {
                *path = std::mem::take(path).transformed([1., 0., 0., 1., dx, dy]);
            }
            Shape::ClipBegin(ClipRegion::Rounded { bounds: b, .. })
            | Shape::ClipBegin(ClipRegion::Circle { bounds: b }) => bounds(b),
            Shape::LayerBegin(_) | Shape::LayerEnd | Shape::ClipEnd => {}
        }
    }
    for glyph in &mut result.glyphs {
        bounds(&mut glyph.bounds);
    }
    result
}

// Half-pixels round away from zero. Round in frame coordinates first so a
// negative tile-local origin cannot shift text/images by a pixel.
fn pixel_round(value: f32, origin: i32) -> f32 {
    (value + origin as f32).round() - origin as f32
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct EffectKey([u32; 9]);
struct ClipMask {
    x: i32,
    y: i32,
    mask: Mask,
    used: u64,
}
struct EffectMask {
    width: u32,
    height: u32,
    data: Vec<u8>,
    used: u64,
    tint: Option<([u8; 3], Pixmap)>,
}
/// One face/size/style run of positioned glyphs for the GPU vector text path.
#[cfg(feature = "native-gpu")]
pub(super) struct GpuGlyphRun {
    pub font: cosmic_text::fontdb::ID,
    pub size: f32,
    pub fake_italic: bool,
    /// (glyph id, baseline x, baseline y) in physical pixels.
    pub glyphs: Vec<(u16, f32, f32)>,
}
#[derive(Clone, PartialEq)]
pub struct Glyph {
    pub(super) text: String,
    pub(super) bounds: [f32; 4],
    pub(super) style: TextStyle,
    pub(super) color: [f32; 4],
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TextKey {
    text: String,
    family: String,
    weight: u16,
    italic: bool,
    size: u32,
    width: u32,
    tracking: u32,
    align: crate::declarative::TextAlign,
    line_height: u32,
    max_lines: u16,
    no_wrap: bool,
}
fn default_style(size: f32) -> TextStyle {
    TextStyle {
        size,
        ..TextStyle::default()
    }
}
static REGISTERED_FONTS: std::sync::Mutex<Vec<std::sync::Arc<Vec<u8>>>> =
    std::sync::Mutex::new(Vec::new());
const MAX_REGISTERED_FONTS: usize = 64;
const MAX_FONT_BYTES: usize = 64 * 1024 * 1024;
const MAX_REGISTERED_FONT_BYTES: usize = 256 * 1024 * 1024;
fn font_registration_fits(count: usize, bytes: usize, incoming: usize) -> bool {
    count < MAX_REGISTERED_FONTS
        && bytes
            .checked_add(incoming)
            .is_some_and(|sum| sum <= MAX_REGISTERED_FONT_BYTES)
}
/// Bundle a font with the app: TrueType/OpenType (or a collection) bytes,
/// e.g. `include_bytes!("../fonts/Inter.ttf")`. Call before creating windows;
/// families become available by name to `font_family`, alongside system
/// fonts. Returns the family names found. At most 64 files / 256 MiB total;
/// immutable bytes are shared across windows rather than copied per painter.
pub fn register_font(data: Vec<u8>) -> Result<Vec<String>, String> {
    if data.is_empty() || data.len() > MAX_FONT_BYTES {
        return Err("font data must be 1 byte to 64 MiB".into());
    }
    let mut fonts = REGISTERED_FONTS
        .lock()
        .map_err(|_| "font registry poisoned")?;
    let bytes = fonts.iter().map(|f| f.len()).sum();
    if !font_registration_fits(fonts.len(), bytes, data.len()) {
        return Err("at most 64 fonts / 256 MiB total can be registered".into());
    }
    let data = std::sync::Arc::new(data);
    let mut probe = cosmic_text::fontdb::Database::new();
    probe.load_font_source(cosmic_text::fontdb::Source::Binary(data.clone()));
    let mut families: Vec<String> = probe
        .faces()
        .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()))
        .collect();
    families.sort();
    families.dedup();
    if families.is_empty() {
        return Err("not a TrueType/OpenType font".into());
    }
    fonts.push(data);
    Ok(families)
}
fn registered_fonts() -> Vec<std::sync::Arc<Vec<u8>>> {
    REGISTERED_FONTS
        .lock()
        .map(|f| f.clone())
        .unwrap_or_default()
}
/// Shaped text buffers and glyph rasters are reused between event-driven frames.
pub struct RasterPainter {
    viewport: Viewport,
    fonts: FontSystem,
    system_family: String,
    sf_text: String,
    sf_display: String,
    sf_mono: String,
    /// Every installed family name, for CSS-style fallback lists.
    installed: std::collections::HashSet<String>,
    swash: SwashCache,
    /// Normalized outline ink boxes, not rasterized glyphs. Bounded independently
    /// so repeated GPU frames need no font-table parsing to decide clipping.
    #[cfg(feature = "native-gpu")]
    gpu_ink: BTreeMap<(cosmic_text::fontdb::ID, u16), Option<[f32; 4]>>,
    /// Hashed, not ordered: ordering compares whole paragraphs on every lookup.
    text: crate::fasthash::FxHashMap<TextKey, (Buffer, u64)>,
    /// Unwrapped sizes by a hash of text and style, so layout can answer
    /// "does it fit?" without building a key or touching a shaped buffer.
    natural: crate::fasthash::FxHashMap<u64, [f32; 2]>,
    clock: u64,
    glyph_bytes: usize,
    pub text_cache_misses: u64,
    /// Native shape commands after stroke batching (a command may use multiple spans).
    pub shape_draw_calls: u64,
    /// Alpha-mask cache misses. Keys exclude effect color and integer position.
    pub effect_cache_misses: u64,
    /// Shadows skipped because they exceeded the per-effect or per-frame blur
    /// budget; the frame still renders.
    pub effects_skipped: u64,
    /// RGB tint rebuilds; alpha-only animation frames reuse premultiplied pixels.
    pub effect_tint_misses: u64,
    /// Transparent offscreen buffers are reused on subsequent animation frames.
    pub layer_allocations: u64,
    clip_masks: BTreeMap<Vec<u32>, ClipMask>,
    clip_bytes: usize,
    pub clip_cache_misses: u64,
    effect_masks: BTreeMap<EffectKey, EffectMask>,
    effect_bytes: usize,
    effect_tint_bytes: usize,
    layer_pool: Vec<Pixmap>,
    /// Pixel region [x0, y0, x1, y1] being repainted by an incremental frame;
    /// every write is confined to it through the clip masks.
    damage: Option<[usize; 4]>,
    raster_origin: [i32; 2],
    /// The previous incremental frame, diffed against the next one.
    last: Option<LastFrame>,
    scaled_images: BTreeMap<(usize, u32, u32), ScaledImage>,
    scaled_bytes: usize,
}
/// An image resampled once to its drawn physical size. Holding the `Image`
/// keeps its pixel buffer (the key's address) alive while cached.
struct ScaledImage {
    _source: crate::graphics::Image,
    pixmap: Pixmap,
    opaque: bool,
    used: u64,
}
const SCALED_IMAGE_BYTES: usize = 64 * 1024 * 1024;
struct LastFrame {
    batches: Vec<PaintBatch<Shape, Glyph>>,
    pixmap: Pixmap,
    background: [f32; 4],
}
/// What an incremental frame had to repaint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Damage {
    /// Identical to the previous frame; the previous pixels are still valid.
    None,
    /// Only this pixel region `[x0, y0, x1, y1]` (exclusive) changed.
    Rect([u32; 4]),
    /// The whole frame was repainted.
    Full,
}
/// Conservative pixel bounds of a drawing command, including blur/stroke
/// extents. `None` for structural commands (layers and clips).
fn shape_extent(s: &Shape) -> Option<[f32; 4]> {
    let pad = |[x, y, w, h]: [f32; 4], p: f32| [x - p, y - p, w + 2. * p, h + 2. * p];
    let span = |xs: &[f32], ys: &[f32]| {
        let (x0, x1) = xs
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        let (y0, y1) = ys
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(*v), b.max(*v)));
        [x0, y0, x1 - x0, y1 - y0]
    };
    Some(match s {
        Shape::Rectangle(b, _)
        | Shape::Rounded(b, ..)
        | Shape::Corners(b, ..)
        | Shape::Gradient(b, ..)
        | Shape::Image(b, ..) => pad(*b, 1.),
        Shape::Outline(b, _, w, _) => pad(*b, w + 1.),
        Shape::Triangle(p, _) => pad(
            span(&[p[0][0], p[1][0], p[2][0]], &[p[0][1], p[1][1], p[2][1]]),
            1.,
        ),
        // Adjacent lines are stroked as one path. Their miter joins can extend
        // beyond the centerline by miter_limit * half-width, not just width.
        Shape::Line(a, b, w, _) => pad(span(&[a[0], b[0]], &[a[1], b[1]]), stroke_extent(*w)),
        Shape::Path(p, _, w) => {
            let b = native_path(p)?.bounds();
            pad(
                [b.x(), b.y(), b.width(), b.height()],
                stroke_extent(w.unwrap_or(0.)),
            )
        }
        // Three box passes at sigma = blur/2 reach about 1.5 blur; pad more.
        Shape::Shadow(b, _, blur, spread, offset, _) => pad(
            [b[0] + offset[0], b[1] + offset[1], b[2], b[3]],
            spread.max(0.) + blur * 2. + 2.,
        ),
        Shape::ClipBegin(_) | Shape::ClipEnd | Shape::LayerBegin(_) | Shape::LayerEnd => {
            return None
        }
    })
}

pub(super) fn stroke_extent(width: f32) -> f32 {
    width * Stroke::default().miter_limit * 0.5 + 1.
}
fn union(a: Option<[f32; 4]>, b: [f32; 4]) -> Option<[f32; 4]> {
    if !(b[2] > 0. && b[3] > 0.) {
        return a;
    }
    Some(match a {
        None => b,
        Some(a) => {
            let (x0, y0) = (a[0].min(b[0]), a[1].min(b[1]));
            let (x1, y1) = (
                (a[0] + a[2]).max(b[0] + b[2]),
                (a[1] + a[3]).max(b[1] + b[3]),
            );
            [x0, y0, x1 - x0, y1 - y0]
        }
    })
}
fn intersect(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (x0, y0) = (a[0].max(b[0]), a[1].max(b[1]));
    let (x1, y1) = (
        (a[0] + a[2]).min(b[0] + b[2]),
        (a[1] + a[3]).min(b[1] + b[3]),
    );
    [x0, y0, (x1 - x0).max(0.), (y1 - y0).max(0.)]
}
/// Pixels a batch can touch: its commands and text, within its clip.
fn batch_extent(b: &PaintBatch<Shape, Glyph>) -> Result<Option<[f32; 4]>, ()> {
    let clip = b.clip.bounds();
    let mut out = None;
    for s in &b.shapes {
        out = union(out, intersect(shape_extent(s).ok_or(())?, clip));
    }
    for g in &b.glyphs {
        out = union(out, intersect(g.bounds, clip));
    }
    Ok(out)
}
/// Region that differs between two frames, or `Full` when the change is
/// structural (layers/clips) or covers most of the frame. Batches are matched
/// by common prefix and suffix, so an effect that inserts or removes a batch
/// (a hover shadow, a tooltip) only damages its own area.
fn frame_damage(
    old: &[PaintBatch<Shape, Glyph>],
    new: &[PaintBatch<Shape, Glyph>],
    w: u32,
    h: u32,
) -> Damage {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut region = None;
    for batch in old[prefix..old.len() - suffix]
        .iter()
        .chain(&new[prefix..new.len() - suffix])
    {
        match batch_extent(batch) {
            Ok(Some(r)) => region = union(region, r),
            Ok(None) => {}
            Err(()) => return Damage::Full,
        }
    }
    let Some(r) = region else {
        return Damage::None;
    };
    // One extra pixel for antialiased edges, clamped to the frame.
    let x0 = (r[0] - 1.).floor().clamp(0., w as f32) as u32;
    let y0 = (r[1] - 1.).floor().clamp(0., h as f32) as u32;
    let x1 = (r[0] + r[2] + 1.).ceil().clamp(0., w as f32) as u32;
    let y1 = (r[1] + r[3] + 1.).ceil().clamp(0., h as f32) as u32;
    if x0 >= x1 || y0 >= y1 {
        return Damage::None;
    }
    // Repainting most of the frame incrementally saves nothing.
    if u64::from(x1 - x0) * u64::from(y1 - y0) * 2 > u64::from(w) * u64::from(h) {
        return Damage::Full;
    }
    Damage::Rect([x0, y0, x1, y1])
}
impl RasterPainter {
    #[cfg(feature = "native-gpu")]
    pub(super) fn gpu_viewport(&self) -> Viewport {
        self.viewport
    }
    /// Positions come from the same shaped buffers as CPU drawing and editor
    /// hit testing. Only glyph cache misses rasterize small glyph resources.
    #[cfg(feature = "native-gpu")]
    pub(super) fn gpu_glyphs(&mut self, glyph: &Glyph) -> Vec<(cosmic_text::CacheKey, [f64; 2])> {
        let key = self.draw_key(&glyph.text, &glyph.style, glyph.bounds[2]);
        let [gx, gy, gw, gh] = glyph.bounds;
        let mut out = Vec::new();
        for run in self.text[&key].0.layout_runs() {
            if run.line_top >= gh || run.line_top + run.line_height <= 0. {
                continue;
            }
            for shaped in run.glyphs {
                if shaped.x >= gw || shaped.x + shaped.w < 0. {
                    continue;
                }
                let p = shaped.physical((0., 0.), 1.);
                out.push((
                    p.cache_key,
                    [
                        f64::from(p.x + gx.round() as i32),
                        f64::from(p.y + run.line_y as i32 + gy.round() as i32),
                    ],
                ));
            }
        }
        out
    }
    /// Glyphs grouped into runs sharing one face/size, at baseline origins in
    /// physical pixels: the bitmap path's integer origin plus its subpixel bin,
    /// so vector text lands where bitmap text and editor carets do. The flag
    /// reports text that crosses its box and therefore still needs a clip.
    #[cfg(feature = "native-gpu")]
    pub(super) fn gpu_glyph_runs(&mut self, glyph: &Glyph) -> (Vec<GpuGlyphRun>, bool) {
        let key = self.draw_key(&glyph.text, &glyph.style, glyph.bounds[2]);
        let [gx, gy, gw, gh] = glyph.bounds;
        let (ox, oy) = (gx.round(), gy.round());
        let mut runs: Vec<GpuGlyphRun> = Vec::new();
        let mut overflow = false;
        for run in self.text[&key].0.layout_runs() {
            if run.line_top >= gh || run.line_top + run.line_height <= 0. {
                continue;
            }
            overflow |= run.line_top < 0. || run.line_top + run.line_height > gh;
            for shaped in run.glyphs {
                if shaped.x >= gw || shaped.x + shaped.w < 0. {
                    continue;
                }
                overflow |= shaped.x < 0. || shaped.x + shaped.w > gw;
                let p = shaped.physical((0., 0.), 1.);
                let k = p.cache_key;
                let size = f32::from_bits(k.font_size_bits);
                let fake_italic = k.flags.contains(cosmic_text::CacheKeyFlags::FAKE_ITALIC);
                let x = p.x as f32 + k.x_bin.as_float() + ox;
                let y = (p.y + run.line_y as i32) as f32 + k.y_bin.as_float() + oy;
                // Advance widths are not ink bounds (italic f, negative side
                // bearings, accents). Elide the GPU clip only after proving
                // the face's actual ink fits, with a one-pixel AA margin.
                if self.gpu_ink.len() >= 4096
                    && !self.gpu_ink.contains_key(&(k.font_id, k.glyph_id))
                {
                    self.gpu_ink.clear();
                }
                let ink = self
                    .gpu_ink
                    .entry((k.font_id, k.glyph_id))
                    .or_insert_with(|| {
                        let font = self.fonts.get_font(k.font_id)?;
                        let face = font.rustybuzz();
                        let tables = face.tables();
                        if tables.colr.is_some()
                            || tables.sbix.is_some()
                            || tables.cbdt.is_some()
                            || tables.bdat.is_some()
                            || tables.ebdt.is_some()
                            || tables.svg.is_some()
                        {
                            return None;
                        }
                        let b =
                            face.glyph_bounding_box(cosmic_text::ttf_parser::GlyphId(k.glyph_id))?;
                        let em = face.units_per_em() as f32;
                        if em <= 0. {
                            return None;
                        }
                        Some([
                            f32::from(b.x_min) / em,
                            f32::from(b.y_min) / em,
                            f32::from(b.x_max) / em,
                            f32::from(b.y_max) / em,
                        ])
                    });
                let fits = ink.is_some_and(|[xmin, ymin, xmax, ymax]| {
                    let skew = if fake_italic {
                        ymin.abs().max(ymax.abs()) * size * 14f32.to_radians().tan()
                    } else {
                        0.
                    };
                    x + xmin * size - skew - 1. >= gx
                        && x + xmax * size + skew + 1. <= gx + gw
                        && y - ymax * size - 1. >= gy
                        && y - ymin * size + 1. <= gy + gh
                });
                overflow |= !fits;
                match runs.last_mut() {
                    Some(r)
                        if r.font == k.font_id
                            && r.size == size
                            && r.fake_italic == fake_italic =>
                    {
                        r.glyphs.push((k.glyph_id, x, y));
                    }
                    _ => runs.push(GpuGlyphRun {
                        font: k.font_id,
                        size,
                        fake_italic,
                        glyphs: vec![(k.glyph_id, x, y)],
                    }),
                }
            }
        }
        (runs, overflow)
    }
    /// The loaded face (shared, memory-mapped data) and its collection index.
    #[cfg(feature = "native-gpu")]
    pub(super) fn gpu_font(
        &mut self,
        id: cosmic_text::fontdb::ID,
    ) -> Option<(std::sync::Arc<cosmic_text::Font>, u32)> {
        let index = self.fonts.db().face(id)?.index;
        Some((self.fonts.get_font(id)?, index))
    }
    #[cfg(feature = "native-gpu")]
    pub(super) fn gpu_glyph_image(
        &mut self,
        key: cosmic_text::CacheKey,
    ) -> Option<&cosmic_text::SwashImage> {
        if self.swash.image_cache.len() > 4096 || self.glyph_bytes > 16 * 1024 * 1024 {
            self.swash.image_cache.clear();
            self.glyph_bytes = 0;
        }
        if !self.swash.image_cache.contains_key(&key) {
            self.glyph_bytes += self
                .swash
                .get_image(&mut self.fonts, key)
                .as_ref()
                .map_or(0, |i| i.data.len());
        }
        self.swash.get_image(&mut self.fonts, key).as_ref()
    }
    /// Resolve portable SF aliases to installed Apple faces on macOS.
    /// `SF` / `SF Pro` use text faces below 24 logical px, display above.
    /// `size` is physical px, like TextStyle; the viewport supplies DPI.
    /// Other platforms (or missing faces) use sans-serif / monospace. This
    /// reports the requested family, not per-glyph fallback or optical sizing.
    /// Existing `system` and generic family choices remain unchanged.
    /// Resolve a requested family. A comma-separated list (`"New York Extra
    /// Large, Charter, serif"`) picks the first installed family, like CSS;
    /// aliases and generic names in it always resolve.
    pub fn resolved_font_family(&self, requested: &str, size: f32) -> String {
        if requested.contains(',') {
            let generic = |name: &str| {
                matches!(
                    name,
                    "SF" | "SF Pro"
                        | "SF Mono"
                        | "system"
                        | "serif"
                        | "sans-serif"
                        | "sans"
                        | "monospace"
                        | "mono"
                        | "cursive"
                        | "fantasy"
                )
            };
            let mut last = "";
            for name in requested
                .split(',')
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                if generic(name) {
                    return self.resolved_font_family(name, size);
                }
                if self.installed.contains(name) {
                    return name.to_owned();
                }
                last = name;
            }
            return last.to_owned();
        }
        match requested {
            "SF" | "SF Pro" if size >= 24. * self.viewport.scale => self.sf_display.clone(),
            "SF" | "SF Pro" => self.sf_text.clone(),
            "SF Mono" => self.sf_mono.clone(),
            "system" | "" => self.system_family.clone(),
            name => name.to_owned(),
        }
    }
    pub fn new(viewport: Viewport) -> Self {
        let mut fonts = FontSystem::new();
        for data in registered_fonts() {
            fonts
                .db_mut()
                .load_font_source(cosmic_text::fontdb::Source::Binary(data));
        }
        configure_generic_families(&mut fonts);
        let system_family = system_family(&fonts);
        let sf_text = apple_family(
            &fonts,
            &["SF Pro Text", "SF Pro", ".SF NS", "System Font"],
            "sans-serif",
        );
        let sf_display = apple_family(
            &fonts,
            &["SF Pro Display", "SF Pro", ".SF NS", "System Font"],
            "sans-serif",
        );
        let sf_mono = apple_family(&fonts, &["SF Mono", ".SF NS Mono"], "monospace");
        let installed = fonts
            .db()
            .faces()
            .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
            .collect();
        if std::env::var_os("VOLT_UI_TRACE").is_some() {
            eprintln!("VOLT_UI_FONTS system={system_family:?} sf_text={sf_text:?} sf_display={sf_display:?} sf_mono={sf_mono:?}");
        }
        Self {
            viewport,
            fonts,
            system_family,
            sf_text,
            sf_display,
            sf_mono,
            installed,
            swash: SwashCache::new(),
            #[cfg(feature = "native-gpu")]
            gpu_ink: BTreeMap::new(),
            text: Default::default(),
            natural: Default::default(),
            text_cache_misses: 0,
            shape_draw_calls: 0,
            effect_cache_misses: 0,
            effects_skipped: 0,
            effect_tint_misses: 0,
            layer_allocations: 0,
            clip_masks: BTreeMap::new(),
            clip_bytes: 0,
            clip_cache_misses: 0,
            effect_masks: BTreeMap::new(),
            effect_bytes: 0,
            effect_tint_bytes: 0,
            layer_pool: Vec::new(),
            damage: None,
            raster_origin: [0; 2],
            last: None,
            scaled_images: BTreeMap::new(),
            scaled_bytes: 0,
            clock: 0,
            glyph_bytes: 0,
        }
    }
    pub fn set_viewport(&mut self, viewport: Viewport) {
        if self.viewport.width as u32 != viewport.width as u32
            || self.viewport.height as u32 != viewport.height as u32
        {
            self.layer_pool.clear();
            self.clip_masks.clear();
            self.clip_bytes = 0;
        }
        self.viewport = viewport;
    }
    fn key(&mut self, text: &str, style: &TextStyle, width: f32) -> TextKey {
        let size = if style.size.is_finite() {
            style.size.clamp(1., 384.)
        } else {
            14.
        };
        let width = if width.is_finite() {
            width.clamp(0.1, 1_000_000.)
        } else {
            1.
        };
        let family = self.resolved_font_family(&style.family, size);
        let key = TextKey {
            text: text.into(),
            family,
            weight: style.weight.clamp(1, 1000),
            italic: style.italic,
            size: size.to_bits(),
            width: width.to_bits(),
            tracking: if style.tracking.is_finite() {
                style.tracking.clamp(-0.2, 1.).to_bits()
            } else {
                0
            },
            align: style.align,
            line_height: if style.line_height.is_finite() && style.line_height > 0. {
                style.line_height.clamp(0.5, 4.).to_bits()
            } else {
                0
            },
            max_lines: style.max_lines,
            no_wrap: style.no_wrap,
        };
        self.clock += 1;
        if !self.text.contains_key(&key) {
            // A frame touches every visible label at each width it is
            // measured at. LRU below a frame's working set misses on every
            // lookup (cyclic access), reshaping all text each frame.
            if self.text.len() >= TEXT_CACHE_ENTRIES {
                if let Some(oldest) = self
                    .text
                    .iter()
                    .min_by_key(|(_, (_, used))| used)
                    .map(|(key, _)| key.clone())
                {
                    self.text.remove(&oldest);
                }
            }
            let line_height = match f32::from_bits(key.line_height) {
                v if v > 0. => v,
                _ => 1.2,
            };
            let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * line_height));
            buffer.set_size(&mut self.fonts, Some(width.max(0.1)), None);
            buffer.set_wrap(
                &mut self.fonts,
                if key.no_wrap && key.max_lines == 0 {
                    Wrap::None
                } else {
                    Wrap::WordOrGlyph
                },
            );
            let family = match key.family.as_str() {
                "sans" | "sans-serif" => Family::SansSerif,
                "serif" => Family::Serif,
                "mono" | "monospace" => Family::Monospace,
                "cursive" => Family::Cursive,
                "fantasy" => Family::Fantasy,
                name => Family::Name(name),
            };
            let attrs = Attrs::new()
                .family(family)
                .weight(Weight(key.weight))
                .style(if key.italic {
                    Style::Italic
                } else {
                    Style::Normal
                });
            let tracking = f32::from_bits(key.tracking);
            let attrs = if tracking == 0. {
                attrs
            } else {
                attrs.letter_spacing(tracking)
            };
            let align = match key.align {
                crate::declarative::TextAlign::Start => None,
                crate::declarative::TextAlign::Center => Some(cosmic_text::Align::Center),
                crate::declarative::TextAlign::End => Some(cosmic_text::Align::End),
                crate::declarative::TextAlign::Justified => Some(cosmic_text::Align::Justified),
            };
            let fonts = &mut self.fonts;
            let mut shape = |buffer: &mut Buffer, value: &str| {
                buffer.set_text(fonts, value, &attrs, Shaping::Advanced);
                if align.is_some() {
                    for line in &mut buffer.lines {
                        line.set_align(align);
                    }
                }
                buffer.shape_until_scroll(fonts, false);
                buffer.layout_runs().count()
            };
            let lines = shape(&mut buffer, text);
            let limit = usize::from(key.max_lines);
            // Truncate: the longest prefix that, with an ellipsis, fits the
            // line limit. Binary search over character boundaries; only on a
            // cache miss.
            if limit > 0 && lines > limit {
                let cuts: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
                let (mut lo, mut hi) = (0, cuts.len());
                while lo < hi {
                    let mid = (lo + hi).div_ceil(2);
                    let candidate =
                        format!("{}…", text[..cuts[mid.min(cuts.len() - 1)]].trim_end());
                    if mid < cuts.len() && shape(&mut buffer, &candidate) <= limit {
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                let kept = if lo == 0 {
                    ""
                } else {
                    text[..cuts[lo]].trim_end()
                };
                shape(&mut buffer, &format!("{kept}…"));
            }
            self.text.insert(key.clone(), (buffer, self.clock));
            self.text_cache_misses += 1;
        }
        self.text.get_mut(&key).unwrap().1 = self.clock;
        key
    }
    /// Rebuild clip coverage for one batch rectangle plus the clip stack.
    ///
    /// Zero-radius rectangular clips (scroll views, panels) are intersected as
    /// pixel-snapped rectangles into `rect_mask`; that mask only changes when the
    /// rectangle does. Curved or path clips are rasterized into `curve_mask`,
    /// touching only the intersection of the rectangle with their bounds, so a
    /// rounded photo inside a large scroll view costs its own area, not the
    /// viewport's. Returns true when `rect_mask` alone describes coverage.
    #[allow(clippy::too_many_arguments)]
    fn rebuild_mask(
        &mut self,
        rect_mask: &mut Mask,
        curve_mask: &mut Mask,
        rect: [f32; 4],
        clips: &[ClipRegion],
        rect_active: &mut Option<[usize; 4]>,
        curve_active: &mut Option<[usize; 4]>,
    ) -> Result<bool, String> {
        let w = rect_mask.width();
        let h = rect_mask.height();
        let snap = |v: f32, max: u32| v.clamp(0., max as f32) as usize;
        let mut region = [
            snap(rect[0].ceil(), w),
            snap(rect[1].ceil(), h),
            snap((rect[0] + rect[2]).ceil(), w),
            snap((rect[1] + rect[3]).ceil(), h),
        ];
        if let Some(d) = self.damage {
            region = [
                region[0].max(d[0]),
                region[1].max(d[1]),
                region[2].min(d[2]),
                region[3].min(d[3]),
            ];
        }
        let mut curved = Vec::new();
        for clip in clips {
            match clip {
                ClipRegion::Rounded { bounds, radii } if radii.iter().all(|r| *r == 0.) => {
                    region[0] = region[0].max(snap(bounds[0].round(), w));
                    region[1] = region[1].max(snap(bounds[1].round(), h));
                    region[2] = region[2].min(snap((bounds[0] + bounds[2]).round(), w));
                    region[3] = region[3].min(snap((bounds[1] + bounds[3]).round(), h));
                }
                _ => curved.push(clip),
            }
        }
        if region[0] >= region[2] || region[1] >= region[3] {
            region = [0; 4];
        }
        if *rect_active != Some(region) {
            if let Some(old) = rect_active.take() {
                fill_mask_px(rect_mask, old, 0);
            }
            fill_mask_px(rect_mask, region, 255);
            *rect_active = Some(region);
        }
        if curved.is_empty() {
            return Ok(true);
        }
        if let Some(old) = curve_active.take() {
            fill_mask_px(curve_mask, old, 0);
        }
        let mut keys: Vec<Vec<u32>> = Vec::new();
        let mut uncached: Vec<ClipMask> = Vec::new();
        for clip in curved {
            let mut key = clip_key(clip);
            key.extend([w, h]);
            self.clock += 1;
            let bounds = if let Some(local) = self.clip_masks.get_mut(&key) {
                local.used = self.clock;
                [
                    local.x,
                    local.y,
                    local.mask.width() as i32,
                    local.mask.height() as i32,
                ]
            } else {
                let path = clip_path(clip).ok_or("invalid clip geometry")?;
                let bounds = path.bounds();
                let x = bounds.left().floor().max(0.) as i32;
                let y = bounds.top().floor().max(0.) as i32;
                let width = (bounds.right().ceil().min(w as f32) - x as f32).max(0.) as u32;
                let height = (bounds.bottom().ceil().min(h as f32) - y as f32).max(0.) as u32;
                if width == 0 || height == 0 {
                    return Ok(false);
                }
                let bytes = width as usize * height as usize;
                while self.clip_bytes + bytes > 8 * 1024 * 1024 || self.clip_masks.len() >= 32 {
                    let Some(old) = self
                        .clip_masks
                        .iter()
                        .min_by_key(|(_, v)| v.used)
                        .map(|(k, _)| k.clone())
                    else {
                        break;
                    };
                    let old = self.clip_masks.remove(&old).unwrap();
                    self.clip_bytes -= old.mask.data().len();
                }
                let mut local = Mask::new(width, height).ok_or("invalid clip mask allocation")?;
                local.fill_path(
                    &path,
                    FillRule::EvenOdd,
                    true,
                    Transform::from_translate(-x as f32, -y as f32),
                );
                self.clip_cache_misses += 1;
                let entry = ClipMask {
                    x,
                    y,
                    mask: local,
                    used: self.clock,
                };
                if bytes <= 8 * 1024 * 1024 {
                    self.clip_bytes += bytes;
                    self.clip_masks.insert(key.clone(), entry);
                } else {
                    uncached.push(entry);
                }
                [x, y, width as i32, height as i32]
            };
            if self.clip_masks.contains_key(&key) && !keys.contains(&key) {
                keys.push(key);
            }
            // Pixels outside any clip's bounds have zero coverage.
            region[0] = region[0].max(bounds[0].max(0) as usize);
            region[1] = region[1].max(bounds[1].max(0) as usize);
            region[2] = region[2].min((bounds[0] + bounds[2]).max(0) as usize);
            region[3] = region[3].min((bounds[1] + bounds[3]).max(0) as usize);
        }
        if region[0] >= region[2] || region[1] >= region[3] {
            return Ok(false);
        }
        fill_mask_px(curve_mask, region, 255);
        *curve_active = Some(region);
        for key in &keys {
            if let Some(local) = self.clip_masks.get(key) {
                apply_clip_mask(curve_mask, local, region);
            }
        }
        for local in &uncached {
            apply_clip_mask(curve_mask, local, region);
        }
        Ok(false)
    }
    /// Opaque, untiled images are resampled once per drawn size and then copied
    /// row by row through the coverage mask; translucent or tiled images use the
    /// general pattern path. Positions snap to whole physical pixels.
    fn draw_image_cached(
        &mut self,
        pixmap: &mut Pixmap,
        mask: &Mask,
        [x, y, w, h]: [f32; 4],
        image: &BackgroundImage,
        scale: f32,
    ) -> Result<(), String> {
        if image.fit == ImageFit::Tile || image.opacity != 1. || !scale.is_finite() || scale <= 0. {
            return draw_image(pixmap, mask, [x, y, w, h], image, scale);
        }
        let iw = image.image.width() as f32;
        let ih = image.image.height() as f32;
        let (dw, dh) = match image.fit {
            ImageFit::Cover => {
                let f = (w / iw).max(h / ih);
                (iw * f, ih * f)
            }
            ImageFit::Contain => {
                let f = (w / iw).min(h / ih);
                (iw * f, ih * f)
            }
            _ => (w, h),
        };
        if !dw.is_finite() || !dh.is_finite() {
            return draw_image(pixmap, mask, [x, y, w, h], image, scale);
        }
        let (sw, sh) = (dw.round() as u32, dh.round() as u32);
        if sw == 0 || sh == 0 || u64::from(sw) * u64::from(sh) > 16_777_216 {
            return draw_image(pixmap, mask, [x, y, w, h], image, scale);
        }
        let pos = image.position.fractions();
        let [ox, oy] = self.raster_origin;
        let ix = pixel_round(x + (w - dw) * pos[0], ox) as i64;
        let iy = pixel_round(y + (h - dh) * pos[1], oy) as i64;
        let key = (image.image.pixels().as_ptr() as usize, sw, sh);
        self.clock += 1;
        if !self.scaled_images.contains_key(&key) {
            let source = tiny_skia::PixmapRef::from_bytes(
                image.image.pixels(),
                image.image.width(),
                image.image.height(),
            )
            .ok_or("invalid RGBA image")?;
            let mut scaled = Pixmap::new(sw, sh).ok_or("invalid scaled image")?;
            scaled.draw_pixmap(
                0,
                0,
                source,
                &tiny_skia::PixmapPaint {
                    quality: tiny_skia::FilterQuality::Bicubic,
                    ..tiny_skia::PixmapPaint::default()
                },
                Transform::from_scale(sw as f32 / iw, sh as f32 / ih),
                None,
            );
            let bytes = scaled.data().len();
            if bytes > SCALED_IMAGE_BYTES {
                return draw_image(pixmap, mask, [x, y, w, h], image, scale);
            }
            while self.scaled_bytes + bytes > SCALED_IMAGE_BYTES {
                let Some(oldest) = self
                    .scaled_images
                    .iter()
                    .min_by_key(|(_, v)| v.used)
                    .map(|(k, _)| *k)
                else {
                    break;
                };
                let old = self.scaled_images.remove(&oldest).unwrap();
                self.scaled_bytes -= old.pixmap.data().len();
            }
            self.scaled_bytes += bytes;
            let opaque = scaled.pixels().iter().all(|p| p.alpha() == 255);
            self.scaled_images.insert(
                key,
                ScaledImage {
                    _source: image.image.clone(),
                    pixmap: scaled,
                    opaque,
                    used: 0,
                },
            );
        }
        let entry = self.scaled_images.get_mut(&key).unwrap();
        entry.used = self.clock;
        // Contain shows the whole image; other fits fill (and crop to) the box.
        let area = if image.fit == ImageFit::Contain {
            [ix as f32, iy as f32, sw as f32, sh as f32]
        } else {
            [x, y, w, h]
        };
        let (pw, ph) = (i64::from(pixmap.width()), i64::from(pixmap.height()));
        let l = (pixel_round(area[0], ox) as i64).max(ix).max(0);
        let t = (pixel_round(area[1], oy) as i64).max(iy).max(0);
        let r = (pixel_round(area[0] + area[2], ox) as i64)
            .min(ix + i64::from(sw))
            .min(pw);
        let b = (pixel_round(area[1] + area[3], oy) as i64)
            .min(iy + i64::from(sh))
            .min(ph);
        if l >= r || t >= b {
            return Ok(());
        }
        let opaque = entry.opaque;
        let source = entry.pixmap.data();
        let coverage = mask.data();
        let dest = pixmap.data_mut();
        let (l, r) = (l as usize, r as usize);
        for row in t..b {
            let base = (row * pw) as usize;
            let src_start = (((row - iy) * i64::from(sw) + (l as i64 - ix)) as usize) * 4;
            let src = &source[src_start..src_start + (r - l) * 4];
            let dst = &mut dest[(base + l) * 4..(base + r) * 4];
            let cov = &coverage[base + l..base + r];
            // Interior rows of an opaque photo are a straight copy.
            if opaque && cov.iter().all(|c| *c == 255) {
                dst.copy_from_slice(src);
                continue;
            }
            for ((d, s), c) in dst
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .zip(src.as_chunks::<4>().0.iter())
                .zip(cov)
            {
                if *c == 0 {
                    continue;
                }
                if *c == 255 && s[3] == 255 {
                    d.copy_from_slice(s);
                    continue;
                }
                // Premultiplied source-over, with the source scaled by coverage.
                let c = u32::from(*c);
                let inverse = 255 - (u32::from(s[3]) * c + 127) / 255;
                for k in 0..4 {
                    let sv = (u32::from(s[k]) * c + 127) / 255;
                    let dv = (u32::from(d[k]) * inverse + 127) / 255;
                    d[k] = (sv + dv).min(255) as u8;
                }
            }
        }
        Ok(())
    }
    /// Blur uses three separable box passes approximating a Gaussian with
    /// sigma=blur/2. Coverage masks are color independent and bounded by an LRU.
    #[allow(clippy::too_many_arguments)]
    fn draw_shadow(
        &mut self,
        pixmap: &mut Pixmap,
        clip_mask: &Mask,
        clip: [f32; 4],
        bounds: [f32; 4],
        radii: [f32; 4],
        blur: f32,
        spread: f32,
        offset: [f32; 2],
        color: [f32; 4],
        frame_pixels: &mut u64,
    ) -> Result<(), String> {
        if !bounds.into_iter().all(f32::is_finite)
            || bounds[2] <= 0.
            || bounds[3] <= 0.
            || radii.into_iter().any(|r| !r.is_finite() || r < 0.)
            || ![blur, spread, offset[0], offset[1]]
                .into_iter()
                .all(f32::is_finite)
            || !(0. ..=384.).contains(&blur)
            || spread.abs() > 384.
            || offset.into_iter().any(|v| v.abs() > 65_536.)
            || color.into_iter().any(|v| !v.is_finite())
        {
            return Err("invalid or over-limit shadow geometry".into());
        }
        if color[3] <= 0. {
            return Ok(());
        }
        let ew = bounds[2] + spread * 2.;
        let eh = bounds[3] + spread * 2.;
        if ew <= 0. || eh <= 0. {
            return Ok(());
        }
        let radii = radii.map(|r| {
            (r.min(bounds[2].min(bounds[3]) * 0.5) + spread)
                .max(0.)
                .min(ew.min(eh) * 0.5)
        });
        let passes = gaussian_box_radii(blur * 0.5);
        let pad = passes.iter().sum::<u32>() + 2;
        let x = bounds[0] + offset[0] - spread;
        let y = bounds[1] + offset[1] - spread;
        let left = x.floor() - pad as f32;
        let top = y.floor() - pad as f32;
        let mask_width = (ew + (x - x.floor()) + 1.).ceil() + (pad * 2) as f32;
        let mask_height = (eh + (y - y.floor()) + 1.).ceil() + (pad * 2) as f32;
        if !mask_width.is_finite()
            || !mask_height.is_finite()
            || mask_width > MAX_EFFECT_PIXELS as f32
            || mask_height > MAX_EFFECT_PIXELS as f32
        {
            return Err("shadow mask dimensions exceed budget".into());
        }
        let mw = mask_width as u32;
        let mh = mask_height as u32;
        // A fully clipped effect performs no allocations or blur work.
        if left >= clip[0] + clip[2]
            || top >= clip[1] + clip[3]
            || left + mw as f32 <= clip[0]
            || top + mh as f32 <= clip[1]
        {
            return Ok(());
        }
        let area = u64::from(mw) * u64::from(mh);
        if area > MAX_EFFECT_PIXELS || frame_pixels.saturating_add(area) > MAX_FRAME_EFFECT_PIXELS {
            // Shadows are decoration: past the blur-work budget (a window-sized
            // shadow on a 6K Retina display) skip this one rather than fail
            // the whole frame.
            self.effects_skipped += 1;
            return Ok(());
        }
        *frame_pixels += area;
        let local = [x - left, y - top, ew, eh];
        let key = EffectKey([
            local[0].to_bits(),
            local[1].to_bits(),
            ew.to_bits(),
            eh.to_bits(),
            radii[0].to_bits(),
            radii[1].to_bits(),
            radii[2].to_bits(),
            radii[3].to_bits(),
            blur.to_bits(),
        ]);
        self.clock += 1;
        if !self.effect_masks.contains_key(&key) {
            let mut mask = Mask::new(mw, mh).ok_or("invalid effect mask allocation")?;
            let path = rounded_corners(local, radii).ok_or("invalid effect corner path")?;
            mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
            let mut data = mask.data().to_vec();
            drop(mask);
            let mut scratch = vec![0_u8; area as usize];
            for radius in passes {
                if radius > 0 {
                    box_blur(
                        &data,
                        &mut scratch,
                        mw as usize,
                        mh as usize,
                        radius as usize,
                        true,
                    );
                    box_blur(
                        &scratch,
                        &mut data,
                        mw as usize,
                        mh as usize,
                        radius as usize,
                        false,
                    );
                }
            }
            while self.effect_masks.len() >= EFFECT_CACHE_ENTRIES
                || self.effect_bytes + data.len() > EFFECT_CACHE_BYTES
            {
                let Some(oldest) = self
                    .effect_masks
                    .iter()
                    .min_by_key(|(_, mask)| mask.used)
                    .map(|(key, _)| key.clone())
                else {
                    break;
                };
                let removed = self.effect_masks.remove(&oldest).unwrap();
                self.effect_bytes -= removed.data.len();
                if let Some((_, tint)) = removed.tint {
                    self.effect_tint_bytes -= tint.data().len();
                }
            }
            self.effect_bytes += data.len();
            self.effect_masks.insert(
                key.clone(),
                EffectMask {
                    width: mw,
                    height: mh,
                    data,
                    used: self.clock,
                    tint: None,
                },
            );
            self.effect_cache_misses += 1;
        }
        let source = [byte(color[0]), byte(color[1]), byte(color[2])];
        if self.effect_masks[&key]
            .tint
            .as_ref()
            .is_none_or(|(rgb, _)| *rgb != source)
        {
            if let Some((_, previous)) = self.effect_masks.get_mut(&key).unwrap().tint.take() {
                self.effect_tint_bytes -= previous.data().len();
            }
            let tint_bytes = area as usize * 4;
            while self.effect_tint_bytes + tint_bytes > EFFECT_TINT_BYTES {
                let Some(oldest) = self
                    .effect_masks
                    .iter()
                    .filter(|(_, m)| m.tint.is_some())
                    .min_by_key(|(_, m)| m.used)
                    .map(|(key, _)| key.clone())
                else {
                    return Err("effect tint cache budget exceeded".into());
                };
                if let Some((_, evicted)) = self.effect_masks.get_mut(&oldest).unwrap().tint.take()
                {
                    self.effect_tint_bytes -= evicted.data().len();
                }
            }
            let effect = &self.effect_masks[&key];
            let mut tint =
                Pixmap::new(effect.width, effect.height).ok_or("invalid effect tint allocation")?;
            let table: [_; 256] = std::array::from_fn(|coverage| {
                tiny_skia::ColorU8::from_rgba(source[0], source[1], source[2], coverage as u8)
                    .premultiply()
            });
            for (pixel, coverage) in tint.pixels_mut().iter_mut().zip(&effect.data) {
                *pixel = table[*coverage as usize];
            }
            self.effect_tint_bytes += tint.data().len();
            self.effect_masks.get_mut(&key).unwrap().tint = Some((source, tint));
            self.effect_tint_misses += 1;
        }
        let effect = self.effect_masks.get_mut(&key).unwrap();
        effect.used = self.clock;
        blit_tint(
            pixmap,
            &effect.tint.as_ref().unwrap().1,
            clip_mask,
            [left as i32, top as i32],
            clip,
            byte(color[3]),
        );
        Ok(())
    }
    /// Draw a complete frame into a new pixmap.
    pub fn rasterize(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<Pixmap, String> {
        self.rasterize_into(batches, background, None)
    }
    /// Draw a frame, repainting only what changed since the previous call.
    /// The returned pixels are the full, current frame; `Damage` says which
    /// part changed, so a host can upload and present just that region.
    pub fn rasterize_incremental(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<(&Pixmap, Damage), String> {
        let (w, h) = (self.viewport.width as u32, self.viewport.height as u32);
        let damage = match &self.last {
            Some(last)
                if last.background == background
                    && last.pixmap.width() == w
                    && last.pixmap.height() == h =>
            {
                frame_damage(&last.batches, batches, w, h)
            }
            _ => Damage::Full,
        };
        let pixmap = match damage {
            Damage::None => None,
            Damage::Rect([x0, y0, x1, y1]) => {
                // Taken first: on error the next frame starts from scratch.
                let base = self
                    .last
                    .take()
                    .expect("damage implies a previous frame")
                    .pixmap;
                let region = [x0 as usize, y0 as usize, x1 as usize, y1 as usize];
                Some(self.rasterize_into(batches, background, Some((base, region)))?)
            }
            Damage::Full => {
                self.last = None;
                Some(self.rasterize_into(batches, background, None)?)
            }
        };
        if let Some(pixmap) = pixmap {
            self.last = Some(LastFrame {
                batches: batches.to_vec(),
                pixmap,
                background,
            });
        }
        Ok((&self.last.as_ref().expect("frame stored").pixmap, damage))
    }
    fn rasterize_into(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
        base: Option<(Pixmap, [usize; 4])>,
    ) -> Result<Pixmap, String> {
        let w = self.viewport.width as u32;
        let h = self.viewport.height as u32;
        if u64::from(w) * u64::from(h) > MAX_FRAMEBUFFER_PIXELS {
            return Err("framebuffer exceeds 64 megapixel limit".into());
        }
        if tiled_opacity_needed(batches, w, h)? {
            // Repaint the complete valid scene if an incremental region needs
            // tiled groups. Discard the old buffer before allocating the new
            // one; the caller's damage region remains valid for presentation.
            drop(base);
            return self.rasterize_tiled(batches, background);
        }
        let clear =
            tiny_skia::Color::from_rgba(background[0], background[1], background[2], background[3])
                .ok_or("invalid background color")?;
        self.damage = base.as_ref().map(|(_, region)| *region);
        let mut pixmap = match base {
            Some((mut pixmap, [x0, y0, x1, y1])) => {
                // Replace (not blend) the damaged pixels with the background.
                let paint = Paint {
                    shader: tiny_skia::Shader::SolidColor(clear),
                    blend_mode: tiny_skia::BlendMode::Source,
                    ..Paint::default()
                };
                if let Some(r) =
                    tiny_skia::Rect::from_ltrb(x0 as f32, y0 as f32, x1 as f32, y1 as f32)
                {
                    pixmap.fill_rect(r, &paint, Transform::identity(), None);
                }
                pixmap
            }
            None => {
                let mut pixmap = Pixmap::new(w, h).ok_or("invalid framebuffer")?;
                pixmap.fill(clear);
                pixmap
            }
        };
        // Bounds that any write must stay inside (the damage region, if any).
        let limit = self
            .damage
            .map_or([0., 0., w as f32, h as f32], |[x0, y0, x1, y1]| {
                [x0 as f32, y0 as f32, (x1 - x0) as f32, (y1 - y0) as f32]
            });
        let mut rect_mask = Mask::new(w, h).ok_or("invalid clip mask")?;
        // Allocated on first use: most frames have no curved clip at all.
        let mut curve_mask: Option<Mask> = None;
        let mut last_clip = None;
        let (mut rect_active, mut curve_active) = (None, None);
        let mut plain_mask = true;
        let mut clips = Vec::<ClipRegion>::new();

        let mut layers: Vec<(Pixmap, f32)> = Vec::new();
        let mut effect_pixels = 0_u64;
        for batch in batches {
            let [x, y, cw, ch] = batch.clip.bounds();
            if last_clip != Some([x, y, cw, ch]) {
                plain_mask = self.rebuild_mask(
                    &mut rect_mask,
                    curve_mask.get_or_insert_with(|| Mask::new(w, h).expect("checked dimensions")),
                    [x, y, cw, ch],
                    &clips,
                    &mut rect_active,
                    &mut curve_active,
                )?;
                last_clip = Some([x, y, cw, ch]);
            }
            let mut shapes = batch.shapes.iter().peekable();
            while let Some(shape) = shapes.next() {
                // Incremental frames skip drawing that cannot reach the damage.
                if self.damage.is_some() {
                    if let Some(extent) = shape_extent(shape) {
                        let visible = intersect(extent, limit);
                        if visible[2] <= 0. || visible[3] <= 0. {
                            continue;
                        }
                    }
                }
                let mask = match (&curve_mask, plain_mask) {
                    (Some(curve), false) => curve,
                    _ => &rect_mask,
                };
                match shape {
                    Shape::ClipBegin(region) => {
                        if clips.len() >= 48 {
                            return Err("clip depth exceeds limit".into());
                        }
                        clips.push(region.clone());
                        plain_mask = self.rebuild_mask(
                            &mut rect_mask,
                            curve_mask.get_or_insert_with(|| {
                                Mask::new(w, h).expect("checked dimensions")
                            }),
                            [x, y, cw, ch],
                            &clips,
                            &mut rect_active,
                            &mut curve_active,
                        )?;
                        continue;
                    }
                    Shape::ClipEnd => {
                        clips.pop().ok_or("unbalanced clip end")?;
                        plain_mask = self.rebuild_mask(
                            &mut rect_mask,
                            curve_mask.get_or_insert_with(|| {
                                Mask::new(w, h).expect("checked dimensions")
                            }),
                            [x, y, cw, ch],
                            &clips,
                            &mut rect_active,
                            &mut curve_active,
                        )?;
                        continue;
                    }
                    Shape::Image(bounds, image, scale) => {
                        self.draw_image_cached(&mut pixmap, mask, *bounds, image, *scale)?;
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    Shape::Path(path, color, stroke) => {
                        if let Some(path) = native_path(path) {
                            if let Some(width) = stroke {
                                pixmap.stroke_path(
                                    &path,
                                    &shape_paint(*color),
                                    &Stroke {
                                        width: *width,
                                        ..Stroke::default()
                                    },
                                    Transform::identity(),
                                    Some(mask),
                                );
                            } else {
                                pixmap.fill_path(
                                    &path,
                                    &shape_paint(*color),
                                    FillRule::EvenOdd,
                                    Transform::identity(),
                                    Some(mask),
                                );
                            }
                            self.shape_draw_calls += 1;
                        }
                        continue;
                    }
                    Shape::LayerBegin(opacity) => {
                        if !opacity.is_finite() || !(0. ..=1.).contains(opacity) {
                            return Err("invalid layer opacity".into());
                        }
                        if layers.len() >= MAX_LAYER_DEPTH
                            || u64::from(w) * u64::from(h) * 4 * (layers.len() as u64 + 2)
                                > MAX_LAYER_BYTES
                        {
                            return Err("opacity layer depth/memory budget exceeded".into());
                        }
                        let next = if let Some(mut reusable) = self.layer_pool.pop() {
                            reusable.data_mut().fill(0);
                            reusable
                        } else {
                            self.layer_allocations += 1;
                            Pixmap::new(w, h).ok_or("invalid opacity layer")?
                        };
                        layers.push((std::mem::replace(&mut pixmap, next), *opacity));
                        continue;
                    }
                    Shape::LayerEnd => {
                        let (mut parent, opacity) =
                            layers.pop().ok_or("unbalanced opacity layer end")?;
                        composite_layer(&mut parent, &pixmap, byte(opacity));
                        self.layer_pool.push(std::mem::replace(&mut pixmap, parent));
                        continue;
                    }
                    Shape::Shadow(bounds, radii, blur, spread, offset, color) => {
                        self.draw_shadow(
                            &mut pixmap,
                            mask,
                            [x, y, cw, ch],
                            *bounds,
                            *radii,
                            *blur,
                            *spread,
                            *offset,
                            *color,
                            &mut effect_pixels,
                        )?;
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    Shape::Gradient(bounds, radii, geometry, stops) => {
                        let Some(path) = rounded_corners(*bounds, *radii) else {
                            continue;
                        };
                        let stops: Vec<_> = stops
                            .iter()
                            .filter_map(|(at, c)| {
                                tiny_skia::Color::from_rgba(c[0], c[1], c[2], c[3])
                                    .map(|c| tiny_skia::GradientStop::new(*at, c))
                            })
                            .collect();
                        let shader = match *geometry {
                            GradientGeometry::Linear { start, end } => {
                                tiny_skia::LinearGradient::new(
                                    tiny_skia::Point::from_xy(start[0], start[1]),
                                    tiny_skia::Point::from_xy(end[0], end[1]),
                                    stops,
                                    tiny_skia::SpreadMode::Pad,
                                    Transform::identity(),
                                )
                            }
                            GradientGeometry::Radial { centre, radius } => {
                                let c = tiny_skia::Point::from_xy(centre[0], centre[1]);
                                tiny_skia::RadialGradient::new(
                                    c,
                                    c,
                                    radius,
                                    stops,
                                    tiny_skia::SpreadMode::Pad,
                                    Transform::identity(),
                                )
                            }
                        };
                        if let Some(shader) = shader {
                            let paint = Paint {
                                shader,
                                anti_alias: true,
                                ..Paint::default()
                            };
                            pixmap.fill_path(
                                &path,
                                &paint,
                                FillRule::Winding,
                                Transform::identity(),
                                Some(mask),
                            );
                        }
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    Shape::Outline(bounds, radii, width, color) => {
                        if !width.is_finite() || *width <= 0. {
                            continue;
                        }
                        let Some(path) = outline(*bounds, *radii, *width) else {
                            continue;
                        };
                        pixmap.fill_path(
                            &path,
                            &shape_paint(*color),
                            FillRule::EvenOdd,
                            Transform::identity(),
                            Some(mask),
                        );
                        self.shape_draw_calls += 1;
                        continue;
                    }
                    _ => {}
                }
                // Legacy panels emit short line segments around their border. Batch
                // same-style consecutive segments into one native stroked path.
                if let Shape::Line(a, b, width, color) = shape {
                    let mut builder = PathBuilder::new();
                    builder.move_to(a[0], a[1]);
                    builder.line_to(b[0], b[1]);
                    let mut last_point = *b;
                    while let Some(Shape::Line(a, b, next_width, next_color)) = shapes.peek() {
                        if next_width != width || next_color != color {
                            break;
                        }
                        if *a != last_point {
                            builder.move_to(a[0], a[1]);
                        }
                        builder.line_to(b[0], b[1]);
                        last_point = *b;
                        shapes.next();
                    }
                    if let Some(path) = builder.finish() {
                        let paint = shape_paint(*color);
                        let stroke = Stroke {
                            width: *width,
                            ..Stroke::default()
                        };
                        pixmap.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            Transform::identity(),
                            Some(mask),
                        );
                        self.shape_draw_calls += 1;
                    }
                    continue;
                }
                let (path, color) = match shape {
                    Shape::Rectangle([x, y, w, h], c) => {
                        let Some(r) = tiny_skia::Rect::from_xywh(*x, *y, *w, *h) else {
                            continue;
                        };
                        (PathBuilder::from_rect(r), *c)
                    }
                    Shape::Rounded(bounds, radius, c) => {
                        if plain_mask
                            && fill_rounded_fast(
                                &mut pixmap,
                                mask,
                                intersect([x, y, cw, ch], limit),
                                *bounds,
                                *radius,
                                *c,
                            )
                        {
                            self.shape_draw_calls += 1;
                            continue;
                        }
                        let Some(path) = rounded(*bounds, *radius) else {
                            continue;
                        };
                        (path, *c)
                    }
                    Shape::Line(..) => unreachable!("lines handled above"),
                    Shape::Corners(bounds, radii, c) => {
                        let Some(path) = rounded_corners(*bounds, *radii) else {
                            continue;
                        };
                        (path, *c)
                    }
                    Shape::Outline(..)
                    | Shape::Gradient(..)
                    | Shape::Shadow(..)
                    | Shape::LayerBegin(..)
                    | Shape::LayerEnd
                    | Shape::ClipBegin(..)
                    | Shape::ClipEnd
                    | Shape::Image(..)
                    | Shape::Path(..) => unreachable!("effect commands handled above"),
                    Shape::Triangle(points, c) => {
                        let mut p = PathBuilder::new();
                        p.move_to(points[0][0], points[0][1]);
                        p.line_to(points[1][0], points[1][1]);
                        p.line_to(points[2][0], points[2][1]);
                        p.close();
                        let Some(path) = p.finish() else { continue };
                        (path, *c)
                    }
                };
                let paint = shape_paint(color);
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    Some(mask),
                );
                self.shape_draw_calls += 1;
            }
            let mask = match (&curve_mask, plain_mask) {
                (Some(curve), false) => curve,
                _ => &rect_mask,
            };
            for glyph in &batch.glyphs {
                let visible = intersect(glyph.bounds, limit);
                if self.damage.is_some() && (visible[2] <= 0. || visible[3] <= 0.) {
                    continue;
                }
                let key = self.draw_key(&glyph.text, &glyph.style, glyph.bounds[2]);
                let buffer = &self.text[&key].0;
                let [gx, gy, gw, gh] = glyph.bounds;
                let [ox, oy] = self.raster_origin;
                let (gx_pixel, gy_pixel) = (pixel_round(gx, ox) as i32, pixel_round(gy, oy) as i32);
                let [cx, cy, cw, ch] = batch.clip.bounds();
                let c = glyph.color;
                for run in buffer.layout_runs() {
                    if run.line_top >= gh || run.line_top + run.line_height <= 0. {
                        continue;
                    }
                    for shaped in run.glyphs {
                        if shaped.x >= gw || shaped.x + shaped.w < 0. {
                            continue;
                        }
                        let physical = shaped.physical((0., 0.), 1.);
                        if self.glyph_bytes > 16 * 1024 * 1024
                            || self.swash.image_cache.len() > 4096
                        {
                            self.swash.image_cache.clear();
                            self.swash.outline_command_cache.clear();
                            self.glyph_bytes = 0;
                        }
                        if !self.swash.image_cache.contains_key(&physical.cache_key) {
                            self.glyph_bytes += self
                                .swash
                                .get_image(&mut self.fonts, physical.cache_key)
                                .as_ref()
                                .map_or(0, |image| image.data.len());
                        }
                        self.swash.with_pixels(
                            &mut self.fonts,
                            physical.cache_key,
                            Color::rgba(byte(c[0]), byte(c[1]), byte(c[2]), 255),
                            |x, y, color| {
                                let px = x + physical.x + gx_pixel;
                                let py = y + physical.y + run.line_y as i32 + gy_pixel;
                                if px < 0 || py < 0 || px >= w as i32 || py >= h as i32 {
                                    return;
                                }
                                let fx = px as f32;
                                let fy = py as f32;
                                if fx < cx
                                    || fy < cy
                                    || fx >= cx + cw
                                    || fy >= cy + ch
                                    || fx < gx
                                    || fy < gy
                                    || fx >= gx + gw
                                    || fy >= gy + gh
                                {
                                    return;
                                }
                                let [r, g, b, a] = color.as_rgba();
                                let alpha = a as f32 / 255.
                                    * c[3]
                                    * mask.data()[py as usize * w as usize + px as usize] as f32
                                    / 255.;
                                let pixel = &mut pixmap.pixels_mut()
                                    [py as usize * w as usize + px as usize];
                                blend_pixel(pixel, [r, g, b], alpha);
                            },
                        );
                    }
                }
            }
        }
        if !clips.is_empty() {
            return Err("unbalanced clip begin".into());
        }
        if !layers.is_empty() {
            return Err("unbalanced opacity layer begin".into());
        }
        Ok(pixmap)
    }

    /// Overflow fallback only: replay the same scene in integer-aligned tiles.
    /// Each tile has at most 26 MiB of group buffers (24 nested groups + root),
    /// independent of window size. Keep the full output and all alpha semantics;
    /// never flatten opacity into per-object alpha or drop a group to fit.
    fn rasterize_tiled(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<Pixmap, String> {
        const SIDE: u32 = 512;
        // Rasterizer stroke/AA coverage must not be truncated at tile edges.
        // Discard the overlap after compositing the same full group stream.
        const HALO: u32 = 4;
        let viewport = self.viewport;
        let origin = self.raster_origin;
        let (w, h) = (viewport.width as u32, viewport.height as u32);
        self.layer_pool.clear();
        let result = (|| {
            let mut output = Pixmap::new(w, h).ok_or("invalid framebuffer")?;
            for y in (0..h).step_by(SIDE as usize) {
                for x in (0..w).step_by(SIDE as usize) {
                    let (tw, th) = (SIDE.min(w - x), SIDE.min(h - y));
                    let (left, top) = (x.saturating_sub(HALO), y.saturating_sub(HALO));
                    let (rw, rh) = ((x + tw + HALO).min(w) - left, (y + th + HALO).min(h) - top);
                    // Direct assignment is deliberate: commands are already in
                    // physical pixels; shaping/asset caches stay reusable.
                    self.viewport.width = rw as f32;
                    self.viewport.height = rh as f32;
                    self.raster_origin = [left as i32, top as i32];
                    // Edge tiles have different sizes; pooled layers must match.
                    self.layer_pool
                        .retain(|p| p.width() == rw && p.height() == rh);
                    let translated = batches
                        .iter()
                        .map(|b| translated_batch(b, -(left as f32), -(top as f32)))
                        .collect::<Vec<_>>();
                    let tile = self.rasterize_into(&translated, background, None)?;
                    for row in 0..th as usize {
                        let start = ((y as usize + row) * w as usize + x as usize) * 4;
                        let source =
                            ((row + (y - top) as usize) * rw as usize + (x - left) as usize) * 4;
                        output.data_mut()[start..start + tw as usize * 4]
                            .copy_from_slice(&tile.data()[source..source + tw as usize * 4]);
                    }
                }
            }
            Ok(output)
        })();
        self.viewport = viewport;
        self.raster_origin = origin;
        self.damage = None;
        self.layer_pool.clear();
        result
    }
}
// cosmic-text's default generic families may not be installed (e.g. DejaVu
// Serif on macOS). Select available faces instead of silently using sans fallback.
fn configure_generic_families(fonts: &mut FontSystem) {
    fn installed(fonts: &FontSystem, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find(|candidate| {
                fonts
                    .db()
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == *candidate))
            })
            .map(|name| (*name).to_owned())
    }
    for (family, candidates) in [
        (
            "serif",
            &["Georgia", "Times New Roman", "DejaVu Serif", "Noto Serif"][..],
        ),
        (
            "sans",
            &[
                "Avenir Next",
                "DejaVu Sans",
                "Arial",
                "Noto Sans",
                "Open Sans",
            ][..],
        ),
        (
            "mono",
            &[
                "Menlo",
                "Consolas",
                "DejaVu Sans Mono",
                "Noto Sans Mono",
                "Courier New",
            ][..],
        ),
    ] {
        if let Some(name) = installed(fonts, candidates) {
            match family {
                "serif" => fonts.db_mut().set_serif_family(name),
                "sans" => fonts.db_mut().set_sans_serif_family(name),
                _ => fonts.db_mut().set_monospace_family(name),
            }
        }
    }
}
// Only fonts already discovered on the machine are used. No downloads,
// private font paths, font bundling, global registration or font mutation.
fn apple_family(fonts: &FontSystem, candidates: &[&str], fallback: &str) -> String {
    if cfg!(target_os = "macos") {
        for &candidate in candidates {
            if fonts
                .db()
                .faces()
                .any(|f| f.families.iter().any(|(n, _)| n == candidate))
            {
                return candidate.into();
            }
        }
    }
    fallback.into()
}
fn system_family(fonts: &FontSystem) -> String {
    if cfg!(target_os = "macos")
        && fonts
            .db()
            .faces()
            .any(|face| face.families.iter().any(|(name, _)| name == "Avenir Next"))
    {
        "Avenir Next".into()
    } else {
        "sans-serif".into()
    }
}
fn shape_paint(color: [f32; 4]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        byte(color[0]),
        byte(color[1]),
        byte(color[2]),
        byte(color[3]),
    );
    paint
}
fn byte(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255.).round() as u8
}
impl RasterPainter {
    /// The shaped buffer to draw with. Start-aligned text that fits unwrapped
    /// draws identically from its natural-width buffer, which measurement
    /// already shaped, so a changing box width does not reshape it.
    fn draw_key(&mut self, text: &str, style: &TextStyle, width: f32) -> TextKey {
        if style.align == crate::declarative::TextAlign::Start
            && width.is_finite()
            && width < 1_000_000.
            && self.measure_unbounded(text, style)[0] <= width
        {
            return self.key(text, style, 1_000_000.);
        }
        self.key(text, style, width)
    }
    fn measure_unbounded(&mut self, text: &str, style: &TextStyle) -> [f32; 2] {
        use std::hash::{Hash, Hasher};
        let mut h = crate::fasthash::FxHasher::default();
        text.hash(&mut h);
        style.family.hash(&mut h);
        style.weight.hash(&mut h);
        style.italic.hash(&mut h);
        style.size.to_bits().hash(&mut h);
        style.tracking.to_bits().hash(&mut h);
        style.line_height.to_bits().hash(&mut h);
        style.max_lines.hash(&mut h);
        style.no_wrap.hash(&mut h);
        let id = h.finish();
        if let Some(size) = self.natural.get(&id) {
            return *size;
        }
        if self.natural.len() >= 8 * TEXT_CACHE_ENTRIES {
            self.natural.clear();
        }
        let key = self.key(text, style, 1_000_000.);
        let buffer = &self.text[&key].0;
        let mut width = 0_f32;
        let mut height = f32::from_bits(key.size) * 1.2;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        self.natural.insert(id, [width, height]);
        [width, height]
    }
}
impl TextMetrics for RasterPainter {
    fn line_advances(&mut self, text: &str, style: &TextStyle, byte_offsets: &[usize]) -> Vec<f32> {
        // Share the same shaped buffer as painting: prefix measurement would
        // lose kerning/ligatures. Interpolate grapheme stops within a cluster,
        // as cosmic-text's editor does; complex bidi editing is not promised.
        let key = self.key(text, style, 1_000_000.);
        let buffer = &self.text[&key].0;
        let Some(run) = buffer.layout_runs().next() else {
            return vec![0.; byte_offsets.len()];
        };
        byte_offsets
            .iter()
            .map(|&at| {
                if !text.is_char_boundary(at) {
                    return 0.;
                }
                for glyph in run.glyphs {
                    if at >= glyph.start && at < glyph.end {
                        let cluster = &text[glyph.start..glyph.end];
                        let count = cluster.graphemes(true).count().max(1);
                        let before = cluster
                            .grapheme_indices(true)
                            .take_while(|(offset, _)| glyph.start + offset < at)
                            .count();
                        let advance = glyph.w * before as f32 / count as f32;
                        return if glyph.level.is_rtl() {
                            glyph.x + glyph.w - advance
                        } else {
                            glyph.x + advance
                        };
                    }
                }
                run.glyphs.iter().find(|g| g.end == at).map_or(0., |g| {
                    if g.level.is_rtl() {
                        g.x
                    } else {
                        g.x + g.w
                    }
                })
            })
            .collect()
    }
    fn measure(&mut self, text: &str, size: f32, max_width: f32) -> [f32; 2] {
        self.measure_styled(text, &default_style(size), max_width)
    }
    fn measure_styled(&mut self, text: &str, style: &TextStyle, max_width: f32) -> [f32; 2] {
        // Layout measures the same text at many widths (natural width, then
        // shares of a row). Whenever the unwrapped text already fits, the
        // result is its natural size: reuse that one shaped buffer instead of
        // shaping again at every new width (divider drags, window resizes).
        if max_width.is_finite() && max_width < 1_000_000. {
            let natural = self.measure_unbounded(text, style);
            if natural[0] <= max_width {
                return natural;
            }
        }
        let key = self.key(text, style, max_width);
        let buffer = &self.text[&key].0;
        let mut width = 0_f32;
        let mut height = f32::from_bits(key.size) * 1.2;
        for run in buffer.layout_runs() {
            width = width.max(run.line_w);
            height = height.max(run.line_top + run.line_height);
        }
        [width, height]
    }
}
impl Painter for RasterPainter {
    type Shape = Shape;
    type Glyph = Glyph;
    fn viewport(&self) -> Viewport {
        self.viewport
    }
    fn draw_rect(&self, out: &mut Vec<Shape>, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        out.push(Shape::Rectangle([x, y, w, h], color));
    }
    fn card_triangle(&self, out: &mut Vec<Shape>, points: [[f32; 2]; 3], color: [f32; 4]) {
        out.push(Shape::Triangle(points, color));
    }
    fn card_round(&self, out: &mut Vec<Shape>, rect: [f32; 4], radius: f32, color: [f32; 4]) {
        out.push(Shape::Rounded(rect, radius, color));
    }
    fn card_corners(&self, out: &mut Vec<Shape>, rect: [f32; 4], radii: [f32; 4], color: [f32; 4]) {
        out.push(Shape::Corners(rect, radii, color));
    }
    fn card_outline(
        &self,
        out: &mut Vec<Shape>,
        rect: [f32; 4],
        radii: [f32; 4],
        width: f32,
        color: [f32; 4],
    ) {
        out.push(Shape::Outline(rect, radii, width, color));
    }
    fn card_shadow(
        &self,
        out: &mut Vec<Shape>,
        rect: [f32; 4],
        radii: [f32; 4],
        blur: f32,
        spread: f32,
        offset: [f32; 2],
        color: [f32; 4],
    ) {
        out.push(Shape::Shadow(rect, radii, blur, spread, offset, color));
    }
    fn card_line(
        &self,
        out: &mut Vec<Shape>,
        a: [f32; 2],
        b: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        if width.is_finite() && width > 0. && a != b {
            out.push(Shape::Line(a, b, width, color));
        }
    }
    fn card_text(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: [f32; 4],
    ) {
        if !text.is_empty() {
            out.push(Glyph {
                text: text.into(),
                bounds: [x, y, self.viewport.width - x, size * 1.2],
                style: default_style(size),
                color,
            });
        }
    }
    fn draw_text_colored(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        _line_height: f32,
        color_at: &dyn Fn(usize) -> [f32; 4],
    ) {
        let mut cursor = x;
        for (i, ch) in text.char_indices() {
            let text = ch.to_string();
            let [w, _] = self.measure(&text, size, 1_000_000.);
            self.card_text(out, &text, cursor, y, size, color_at(i));
            cursor += w;
        }
    }
}
impl DocumentPainter for RasterPainter {
    fn ui_clip_begin(&mut self, out: &mut Vec<Shape>, clip: ClipRegion) {
        out.push(Shape::ClipBegin(clip));
    }
    fn ui_clip_end(&mut self, out: &mut Vec<Shape>) {
        out.push(Shape::ClipEnd);
    }
    fn ui_image(
        &mut self,
        out: &mut Vec<Shape>,
        bounds: [f32; 4],
        image: &BackgroundImage,
        scale: f32,
    ) {
        out.push(Shape::Image(bounds, image.clone(), scale));
    }
    fn ui_path(
        &mut self,
        out: &mut Vec<Shape>,
        path: &VectorPath,
        color: [f32; 4],
        stroke: Option<f32>,
    ) {
        out.push(Shape::Path(path.clone(), color, stroke));
    }

    fn ui_layer_begin(&mut self, out: &mut Vec<Shape>, opacity: f32) {
        out.push(Shape::LayerBegin(opacity));
    }
    fn ui_layer_end(&mut self, out: &mut Vec<Shape>) {
        out.push(Shape::LayerEnd);
    }
    fn ui_text(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        bounds: [f32; 4],
        size: f32,
        color: [f32; 4],
    ) {
        self.ui_text_styled(out, text, bounds, &default_style(size), color);
    }
    fn ui_gradient(
        &mut self,
        out: &mut Vec<Shape>,
        bounds: [f32; 4],
        radii: [f32; 4],
        gradient: &crate::declarative::Gradient,
        stops: &[(f32, [f32; 4])],
    ) {
        if bounds[2] > 0. && bounds[3] > 0. {
            out.push(Shape::Gradient(
                bounds,
                radii,
                GradientGeometry::for_box(gradient.kind, bounds),
                stops.to_vec(),
            ));
        }
    }
    fn ui_text_lines(&mut self, text: &str, bounds: [f32; 4], style: &TextStyle) -> Vec<[f32; 3]> {
        let key = self.draw_key(text, style, bounds[2]);
        let (ox, oy) = (bounds[0].round(), bounds[1].round());
        self.text[&key]
            .0
            .layout_runs()
            .filter(|run| run.line_top < bounds[3])
            .filter_map(|run| {
                let first = run.glyphs.first()?;
                let last = run.glyphs.last()?;
                let start = first.x.min(last.x);
                let end = (first.x + first.w).max(last.x + last.w);
                Some([ox + start, oy + run.line_y, (end - start).min(bounds[2])])
            })
            .collect()
    }
    fn ui_text_styled(
        &mut self,
        out: &mut Vec<Glyph>,
        text: &str,
        bounds: [f32; 4],
        style: &TextStyle,
        color: [f32; 4],
    ) {
        if bounds[2] > 0. && bounds[3] > 0. && !text.is_empty() {
            out.push(Glyph {
                text: text.into(),
                bounds,
                style: style.clone(),
                color,
            });
        }
    }
}

/// Split on integer scanlines: AA caps/edges retain tiny-skia's original cubic
/// geometry; fully covered middle pixels use a direct integer source-over span.
/// Disjoint regions guarantee translucent fills never overlap at split seams.
fn fill_rounded_fast(
    pixmap: &mut Pixmap,
    mask: &Mask,
    clip: [f32; 4],
    bounds: [f32; 4],
    radius: f32,
    color: [f32; 4],
) -> bool {
    let [x, y, w, h] = bounds;
    if !bounds.into_iter().all(f32::is_finite) || !radius.is_finite() || w <= 0. || h <= 0. {
        return false;
    }
    let r = radius.max(0.).min(w * 0.5).min(h * 0.5);
    let top = (y + r).ceil();
    let bottom = (y + h - r).floor();
    let left = x.ceil();
    let right = (x + w).floor();
    if top >= bottom || left >= right {
        return false;
    }
    let paint = shape_paint(color);
    for (is_top, cut) in [(true, top), (false, bottom)] {
        if let Some(path) = rounded_cap(bounds, r, cut, is_top) {
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                Some(mask),
            );
        }
    }
    // Preserve fractional side-edge AA exactly using narrow rectangular paths.
    for (sx, sw) in [(x, left - x), (right, x + w - right)] {
        if let Some(rect) = tiny_skia::Rect::from_xywh(sx, top, sw, bottom - top) {
            pixmap.fill_path(
                &PathBuilder::from_rect(rect),
                &paint,
                FillRule::Winding,
                Transform::identity(),
                Some(mask),
            );
        }
    }
    let fw = pixmap.width();
    let fh = pixmap.height();
    let l = left.max(clip[0].ceil()).clamp(0., fw as f32) as usize;
    let t = top.max(clip[1].ceil()).clamp(0., fh as f32) as usize;
    let rr = right
        .min((clip[0] + clip[2]).ceil())
        .clamp(l as f32, fw as f32) as usize;
    let bb = bottom
        .min((clip[1] + clip[3]).ceil())
        .clamp(t as f32, fh as f32) as usize;
    let source = tiny_skia::ColorU8::from_rgba(
        byte(color[0]),
        byte(color[1]),
        byte(color[2]),
        byte(color[3]),
    )
    .premultiply();
    let pixels = pixmap.pixels_mut();
    for row in t..bb {
        let span = &mut pixels[row * fw as usize + l..row * fw as usize + rr];
        if source.alpha() == 255 {
            span.fill(source);
        } else if source.alpha() > 0 {
            let inverse = 255_u32 - u32::from(source.alpha());
            for pixel in span {
                let blend = |s: u8, d: u8| -> u8 {
                    // Match tiny-skia 0.11's low-precision source-over:
                    // premultiplied source plus ceil(dst * inverse / 256).
                    let product = u32::from(d) * inverse;
                    (u32::from(s) + ((product + 255) >> 8)).min(255) as u8
                };
                *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
                    blend(source.red(), pixel.red()),
                    blend(source.green(), pixel.green()),
                    blend(source.blue(), pixel.blue()),
                    // Same formula for alpha: an opaque destination stays exactly
                    // 255, and transparent layers/windows keep correct coverage.
                    blend(source.alpha(), pixel.alpha()),
                )
                .expect("source-over preserves premultiplied channels");
            }
        }
    }
    true
}
fn clip_key(region: &ClipRegion) -> Vec<u32> {
    let mut key = vec![];
    match region {
        ClipRegion::Rounded { bounds, radii } => {
            key.push(0);
            key.extend(bounds.iter().chain(radii).map(|v| v.to_bits()));
        }
        ClipRegion::Circle { bounds } => {
            key.push(1);
            key.extend(bounds.map(f32::to_bits));
        }
        ClipRegion::Path(p) => {
            key.push(2);
            for c in &p.commands {
                match c {
                    PathCommand::Move(a) => {
                        key.push(0);
                        key.extend(a.map(f32::to_bits));
                    }
                    PathCommand::Line(a) => {
                        key.push(1);
                        key.extend(a.map(f32::to_bits));
                    }
                    PathCommand::Quad(a, b) => {
                        key.push(2);
                        key.extend(a.iter().chain(b).map(|v| v.to_bits()));
                    }
                    PathCommand::Cubic(a, b, c) => {
                        key.push(3);
                        key.extend(a.iter().chain(b).chain(c).map(|v| v.to_bits()));
                    }
                    PathCommand::Close => key.push(4),
                }
            }
        }
    }
    key
}
fn apply_clip_mask(mask: &mut Mask, local: &ClipMask, [l, t, r, b]: [usize; 4]) {
    let w = mask.width() as usize;
    let lw = local.mask.width() as usize;
    let lh = local.mask.height() as i32;
    let data = mask.data_mut();
    for y in t..b {
        let row = &mut data[y * w + l..y * w + r];
        let ly = y as i32 - local.y;
        if ly < 0 || ly >= lh {
            row.fill(0);
            continue;
        }
        let source = &local.mask.data()[ly as usize * lw..(ly as usize + 1) * lw];
        for (i, dest) in row.iter_mut().enumerate() {
            let lx = (l + i) as i32 - local.x;
            let coverage = if lx >= 0 && (lx as usize) < lw {
                source[lx as usize]
            } else {
                0
            };
            if coverage != 255 {
                *dest = ((u16::from(*dest) * u16::from(coverage) + 127) / 255) as u8;
            }
        }
    }
}
#[cfg(test)]
fn fill_mask(mask: &mut Mask, w: u32, h: u32, r: [f32; 4], value: u8) {
    let left = r[0].ceil().clamp(0., w as f32) as usize;
    let top = r[1].ceil().clamp(0., h as f32) as usize;
    let right = (r[0] + r[2]).ceil().clamp(left as f32, w as f32) as usize;
    let bottom = (r[1] + r[3]).ceil().clamp(top as f32, h as f32) as usize;
    fill_mask_px(mask, [left, top, right, bottom], value);
}
fn fill_mask_px(mask: &mut Mask, [l, t, r, b]: [usize; 4], value: u8) {
    let w = mask.width() as usize;
    let data = mask.data_mut();
    for row in t..b {
        data[row * w + l..row * w + r].fill(value);
    }
}
pub(super) fn native_path(path: &VectorPath) -> Option<tiny_skia::Path> {
    if path.commands.is_empty() || path.commands.len() > 1024 {
        return None;
    }
    let mut b = PathBuilder::new();
    for cmd in &path.commands {
        match *cmd {
            PathCommand::Move([x, y]) => b.move_to(x, y),
            PathCommand::Line([x, y]) => b.line_to(x, y),
            PathCommand::Quad([x, y], [a, c]) => b.quad_to(x, y, a, c),
            PathCommand::Cubic([x, y], [a, c], [d, e]) => b.cubic_to(x, y, a, c, d, e),
            PathCommand::Close => b.close(),
        }
    }
    b.finish()
}
fn clip_path(clip: &ClipRegion) -> Option<tiny_skia::Path> {
    match clip {
        ClipRegion::Rounded { bounds, radii } => rounded_corners(*bounds, *radii),
        ClipRegion::Circle {
            bounds: [x, y, w, h],
        } => native_path(&VectorPath::circle(
            x + w * 0.5,
            y + h * 0.5,
            w.min(*h) * 0.5,
        )),
        ClipRegion::Path(path) => native_path(path),
    }
}
fn draw_image(
    pixmap: &mut Pixmap,
    mask: &Mask,
    [x, y, w, h]: [f32; 4],
    image: &BackgroundImage,
    scale: f32,
) -> Result<(), String> {
    if !image.opacity.is_finite()
        || !(0. ..=1.).contains(&image.opacity)
        || !scale.is_finite()
        || scale <= 0.
    {
        return Err("invalid image opacity/scale".into());
    }
    let iw = image.image.width() as f32;
    let ih = image.image.height() as f32;
    let (dw, dh) = match image.fit {
        ImageFit::Cover => {
            let f = (w / iw).max(h / ih);
            (iw * f, ih * f)
        }
        ImageFit::Contain => {
            let f = (w / iw).min(h / ih);
            (iw * f, ih * f)
        }
        ImageFit::Stretch => (w, h),
        ImageFit::Tile => (iw * scale, ih * scale),
    };
    let pos = image.position.fractions();
    let ox = x + (w - dw) * pos[0];
    let oy = y + (h - dh) * pos[1];
    // A pattern handles tiling in a single bounded command (not thousands of quads).
    let source = tiny_skia::PixmapRef::from_bytes(
        image.image.pixels(),
        image.image.width(),
        image.image.height(),
    )
    .ok_or("invalid RGBA image")?;
    let transform = Transform::from_row(dw / iw, 0., 0., dh / ih, ox, oy);
    let paint = Paint {
        shader: tiny_skia::Pattern::new(
            source,
            if image.fit == ImageFit::Tile {
                tiny_skia::SpreadMode::Repeat
            } else {
                tiny_skia::SpreadMode::Pad
            },
            tiny_skia::FilterQuality::Bilinear,
            image.opacity,
            transform,
        ),
        ..Paint::default()
    };
    let bounds = if image.fit == ImageFit::Contain {
        [ox, oy, dw, dh]
    } else {
        [x, y, w, h]
    };
    if let Some(r) = tiny_skia::Rect::from_xywh(bounds[0], bounds[1], bounds[2], bounds[3]) {
        pixmap.fill_rect(r, &paint, Transform::identity(), Some(mask));
    }
    Ok(())
}
fn rounded_cap([x, y, w, h]: [f32; 4], r: f32, cut: f32, top: bool) -> Option<tiny_skia::Path> {
    let k = r * 0.5522848;
    let mut p = PathBuilder::new();
    if top {
        p.move_to(x, cut);
        p.line_to(x, y + r);
        p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
        p.line_to(x + w - r, y);
        p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
        p.line_to(x + w, cut);
    } else {
        p.move_to(x, cut);
        p.line_to(x + w, cut);
        p.line_to(x + w, y + h - r);
        p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
        p.line_to(x + r, y + h);
        p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    }
    p.close();
    p.finish()
}

fn rounded([x, y, w, h]: [f32; 4], r: f32) -> Option<tiny_skia::Path> {
    if w <= 0. || h <= 0. {
        return None;
    }
    let r = r.max(0.).min(w * 0.5).min(h * 0.5);
    let k = r * 0.5522848;
    let mut p = PathBuilder::new();
    p.move_to(x + r, y);
    p.line_to(x + w - r, y);
    p.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    p.line_to(x + w, y + h - r);
    p.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    p.line_to(x + r, y + h);
    p.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    p.line_to(x, y + r);
    p.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    p.close();
    p.finish()
}

pub(super) fn rounded_corners([x, y, w, h]: [f32; 4], radii: [f32; 4]) -> Option<tiny_skia::Path> {
    if ![x, y, w, h, x + w, y + h].into_iter().all(f32::is_finite)
        || w <= 0.
        || h <= 0.
        || radii.into_iter().any(|r| !r.is_finite() || r < 0.)
    {
        return None;
    }
    let [tl, tr, br, bl] = radii.map(|r| r.min(w.min(h) * 0.5));
    let k = 0.5522848;
    let mut p = PathBuilder::new();
    p.move_to(x + tl, y);
    p.line_to(x + w - tr, y);
    p.cubic_to(
        x + w - tr + tr * k,
        y,
        x + w,
        y + tr - tr * k,
        x + w,
        y + tr,
    );
    p.line_to(x + w, y + h - br);
    p.cubic_to(
        x + w,
        y + h - br + br * k,
        x + w - br + br * k,
        y + h,
        x + w - br,
        y + h,
    );
    p.line_to(x + bl, y + h);
    p.cubic_to(
        x + bl - bl * k,
        y + h,
        x,
        y + h - bl + bl * k,
        x,
        y + h - bl,
    );
    p.line_to(x, y + tl);
    p.cubic_to(x, y + tl - tl * k, x + tl - tl * k, y, x + tl, y);
    p.close();
    p.finish()
}

pub(super) fn outline(bounds: [f32; 4], radii: [f32; 4], width: f32) -> Option<tiny_skia::Path> {
    let outer = rounded_corners(bounds, radii)?;
    if width * 2. >= bounds[2].min(bounds[3]) {
        return Some(outer);
    }
    let inner_bounds = [
        bounds[0] + width,
        bounds[1] + width,
        bounds[2] - width * 2.,
        bounds[3] - width * 2.,
    ];
    let limit = bounds[2].min(bounds[3]) * 0.5;
    let inner = rounded_corners(inner_bounds, radii.map(|r| (r.min(limit) - width).max(0.)))?;
    let mut builder = PathBuilder::new();
    builder.push_path(&outer);
    builder.push_path(&inner);
    builder.finish()
}

fn blend_pixel(pixel: &mut tiny_skia::PremultipliedColorU8, rgb: [u8; 3], alpha: f32) {
    let alpha = alpha.clamp(0., 1.);
    let inverse = 1. - alpha;
    let a = (alpha * 255. + pixel.alpha() as f32 * inverse).round() as u8;
    let blend = |source: u8, old: u8| {
        (source as f32 * alpha + old as f32 * inverse)
            .round()
            .min(a as f32) as u8
    };
    *pixel = tiny_skia::PremultipliedColorU8::from_rgba(
        blend(rgb[0], pixel.red()),
        blend(rgb[1], pixel.green()),
        blend(rgb[2], pixel.blue()),
        a,
    )
    .expect("source-over preserves premultiplied channels");
}

/// Integer clipped source-over spans. Coverage/RGB pixels stay cached; changing
/// effect opacity never walks the mask to rebuild tints or performs float math
/// per pixel. This avoids tiny-skia's masked pattern pipeline overhead here.
fn blit_tint(
    dest: &mut Pixmap,
    source: &Pixmap,
    mask: &Mask,
    position: [i32; 2],
    clip: [f32; 4],
    opacity: u8,
) {
    if opacity == 0 {
        return;
    }
    let [x, y] = position;
    let fw = dest.width() as usize;
    let left = x.max(clip[0].ceil() as i32).max(0).min(dest.width() as i32);
    let top = y
        .max(clip[1].ceil() as i32)
        .max(0)
        .min(dest.height() as i32);
    let right = x
        .saturating_add(source.width() as i32)
        .min((clip[0] + clip[2]).ceil() as i32)
        .min(dest.width() as i32)
        .max(left);
    let bottom = y
        .saturating_add(source.height() as i32)
        .min((clip[1] + clip[3]).ceil() as i32)
        .min(dest.height() as i32)
        .max(top);
    if left >= right || top >= bottom {
        return;
    }
    let opacity = u32::from(opacity);
    let scale = |value: u8| ((u32::from(value) * opacity + 127) / 255) as u8;
    let width = (right - left) as usize;
    for row in top..bottom {
        let di = row as usize * fw + left as usize;
        let si = (row - y) as usize * source.width() as usize + (left - x) as usize;
        let target = &mut dest.data_mut()[di * 4..(di + width) * 4];
        let input = &source.data()[si * 4..(si + width) * 4];
        let coverage = &mask.data()[di..di + width];
        for ((target, input), clip_alpha) in target
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(input.as_chunks::<4>().0.iter())
            .zip(coverage)
        {
            if *clip_alpha == 0 || input[3] == 0 {
                continue;
            }
            blend_premultiplied(target, input, &scale);
        }
    }
}

/// Group buffers share the framebuffer size. Skip transparent pixels instead
/// of running a pattern shader over the entire otherwise-empty framebuffer.
fn composite_layer(dest: &mut Pixmap, source: &Pixmap, opacity: u8) {
    if opacity == 0 {
        return;
    }
    debug_assert_eq!(dest.data().len(), source.data().len());
    let opacity = u32::from(opacity);
    let scale = |value: u8| ((u32::from(value) * opacity + 127) / 255) as u8;
    for (target, input) in dest
        .data_mut()
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(source.data().as_chunks::<4>().0.iter())
    {
        if input[3] != 0 {
            blend_premultiplied(target, input, &scale);
        }
    }
}

#[inline]
fn blend_premultiplied(target: &mut [u8], input: &[u8], scale: &impl Fn(u8) -> u8) {
    let a = scale(input[3]);
    let inverse = 255 - u32::from(a);
    for channel in 0..4 {
        let source = if channel == 3 {
            a
        } else {
            scale(input[channel])
        };
        let product = u32::from(target[channel]) * inverse;
        target[channel] = (u32::from(source) + ((product + 255) >> 8)).min(255) as u8;
    }
}

fn gaussian_box_radii(sigma: f32) -> [u32; 3] {
    if sigma <= 0. {
        return [0; 3];
    }
    let mut lower = (4. * sigma * sigma + 1.).sqrt().floor() as u32;
    if lower.is_multiple_of(2) {
        lower = lower.saturating_sub(1);
    }
    lower = lower.max(1);
    let lf = lower as f32;
    let count = ((12. * sigma * sigma - 3. * lf * lf - 12. * lf - 9.) / (-4. * lf - 4.))
        .round()
        .clamp(0., 3.) as usize;
    std::array::from_fn(|i| {
        if i < count {
            (lower - 1) / 2
        } else {
            lower.div_ceil(2)
        }
    })
}

fn box_blur(source: &[u8], out: &mut [u8], w: usize, h: usize, radius: usize, horizontal: bool) {
    let (lines, length) = if horizontal { (h, w) } else { (w, h) };
    let kernel = (radius * 2 + 1) as u32;
    for line in 0..lines {
        let index = |position: usize| {
            if horizontal {
                line * w + position
            } else {
                position * w + line
            }
        };
        let mut sum = (0..=radius.min(length - 1))
            .map(|i| u32::from(source[index(i)]))
            .sum::<u32>();
        for position in 0..length {
            out[index(position)] = ((sum + kernel / 2) / kernel) as u8;
            if position >= radius {
                sum -= u32::from(source[index(position - radius)]);
            }
            if position + radius + 1 < length {
                sum += u32::from(source[index(position + radius + 1)]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;
    #[test]
    fn cached_shaping_and_nonuniform_text_metrics() {
        let mut p = RasterPainter::new(Viewport::new(400., 300., 1.).unwrap());
        let wide = p.measure("WWW", 20., 400.);
        let narrow = p.measure("iii", 20., 400.);
        assert!(wide[0] > narrow[0]);
        let misses = p.text_cache_misses;
        p.measure("WWW", 20., 400.);
        assert_eq!(p.text_cache_misses, misses);
        for i in 0..TEXT_CACHE_ENTRIES + 50 {
            p.measure(&format!("entry{i}"), 12., 400.);
        }
        assert_eq!(p.text.len(), TEXT_CACHE_ENTRIES, "bounded");
    }
    #[test]
    fn register_font_validates_data_and_reports_families() {
        assert!(font_registration_fits(63, MAX_REGISTERED_FONT_BYTES - 1, 1));
        assert!(!font_registration_fits(64, 0, 1));
        assert!(!font_registration_fits(1, MAX_REGISTERED_FONT_BYTES, 1));
        assert!(!font_registration_fits(1, usize::MAX, 1));
        assert!(register_font(vec![]).is_err());
        assert!(register_font(b"definitely not a font".to_vec()).is_err());
        // A real system font registers and reports its family names.
        let path = [
            "/System/Library/Fonts/Supplemental/Arial.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists());
        if let Some(path) = path {
            let families = register_font(std::fs::read(path).unwrap()).unwrap();
            assert!(!families.is_empty());
        }
    }
    #[test]
    fn truncation_alignment_line_height_and_no_wrap() {
        use crate::declarative::{DocumentPainter, TextAlign};
        let mut p = RasterPainter::new(Viewport::new(400., 400., 1.).unwrap());
        let long = "The quick brown fox jumps over the lazy dog again and again";
        let base = TextStyle {
            family: "SF".into(),
            size: 16.,
            ..Default::default()
        };
        let wrapped = p.measure_styled(long, &base, 160.);
        let one = p.measure_styled(
            long,
            &TextStyle {
                max_lines: 1,
                ..base.clone()
            },
            160.,
        );
        let two = p.measure_styled(
            long,
            &TextStyle {
                max_lines: 2,
                ..base.clone()
            },
            160.,
        );
        assert!(
            wrapped[1] > two[1] && two[1] > one[1],
            "{wrapped:?} {two:?} {one:?}"
        );
        assert!(one[0] <= 160.);
        // The truncated layout really ends in an ellipsis.
        let key = p.key(
            long,
            &TextStyle {
                max_lines: 1,
                ..base.clone()
            },
            160.,
        );
        let shown: String = p.text[&key].0.lines.iter().map(|l| l.text()).collect();
        assert!(shown.ends_with('…') && shown.len() < long.len(), "{shown}");
        // Short text is untouched by max_lines.
        let key = p.key(
            "Short",
            &TextStyle {
                max_lines: 1,
                ..base.clone()
            },
            160.,
        );
        assert_eq!(p.text[&key].0.lines[0].text(), "Short");
        // Centre and end alignment move the line right within the box.
        let x = |p: &mut RasterPainter, align| {
            p.ui_text_lines(
                "Hi",
                [0., 0., 200., 40.],
                &TextStyle {
                    align,
                    ..base.clone()
                },
            )[0][0]
        };
        let (start, centre, end) = (
            x(&mut p, TextAlign::Start),
            x(&mut p, TextAlign::Center),
            x(&mut p, TextAlign::End),
        );
        assert!(
            start < 2. && centre > 80. && end > centre,
            "{start} {centre} {end}"
        );
        // Line height scales the height of wrapped text.
        let loose = p.measure_styled(
            long,
            &TextStyle {
                line_height: 2.,
                ..base.clone()
            },
            160.,
        );
        assert!(loose[1] > wrapped[1] * 1.5);
        // No-wrap keeps one line however narrow the box.
        let single = p.measure_styled(
            long,
            &TextStyle {
                no_wrap: true,
                ..base.clone()
            },
            60.,
        );
        assert!(single[1] < 25., "{single:?}");
    }
    #[test]
    fn letter_spacing_widens_text_by_its_em_fraction() {
        let mut p = RasterPainter::new(Viewport::new(400., 100., 1.).unwrap());
        let style = TextStyle {
            family: "SF Mono".into(),
            size: 20.,
            ..Default::default()
        };
        let plain = p.measure_styled("ABCDEFGH", &style, 400.)[0];
        let tracked = p.measure_styled(
            "ABCDEFGH",
            &TextStyle {
                tracking: 0.2,
                ..style.clone()
            },
            400.,
        )[0];
        // Eight glyphs, each 0.2 × 20px wider.
        assert!((tracked - plain - 32.).abs() < 1.5, "{plain} → {tracked}");
    }
    #[test]
    fn family_lists_pick_the_first_installed_family() {
        let p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        // Missing families are skipped; generic names always resolve.
        assert_eq!(
            p.resolved_font_family("Nonexistent Display, serif", 14.),
            p.resolved_font_family("serif", 14.)
        );
        assert_eq!(
            p.resolved_font_family("Nope A, Nope B, SF Mono", 14.),
            p.resolved_font_family("SF Mono", 14.)
        );
        // An installed family wins over later candidates.
        if let Some(real) = p.installed.iter().find(|n| !n.starts_with('.')).cloned() {
            assert_eq!(
                p.resolved_font_family(&format!("Nope, {real}, serif"), 14.),
                real
            );
        }
        // Nothing installed and no generic: the last name, for the default fallback.
        assert_eq!(p.resolved_font_family("Nope A, Nope B", 14.), "Nope B");
    }
    #[test]
    fn a_page_of_labels_is_shaped_once_not_every_frame() {
        // ~600 layouts per frame (a dense page plus navigation, measured at
        // two widths) previously exceeded a 256-entry LRU, which then missed
        // on every lookup and reshaped every label every frame.
        let mut p = RasterPainter::new(Viewport::new(400., 300., 1.).unwrap());
        let frame = |p: &mut RasterPainter| {
            for i in 0..300 {
                p.measure(&format!("label {i}"), 14., 400.);
                p.measure(&format!("label {i}"), 14., 120.);
            }
        };
        frame(&mut p);
        let misses = p.text_cache_misses;
        frame(&mut p);
        frame(&mut p);
        assert_eq!(p.text_cache_misses, misses, "later frames reuse layouts");
    }
    #[test]
    fn clipping_and_rounded_rasterization() {
        let mut p = RasterPainter::new(Viewport::new(80., 80., 1.).unwrap());
        let mut shapes = vec![];
        p.card_round(&mut shapes, [10., 10., 50., 50.], 10., [1., 0., 0., 1.]);
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 35., 80.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        let pixel = |x: usize, y: usize| frame.pixels()[y * 80 + x];
        assert_eq!(pixel(10, 10).red(), 0);
        assert_eq!(pixel(25, 25).red(), 255);
        assert_eq!(pixel(40, 25).red(), 0);
    }
    #[test]
    fn selected_corners_fill_outline_and_blurred_shadow() {
        let mut p = RasterPainter::new(Viewport::new(80., 80., 1.).unwrap());
        let mut shapes = vec![];
        p.card_shadow(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            4.,
            0.,
            [0., 0.],
            [1., 0., 0., 1.],
        );
        p.card_corners(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            [0., 1., 0., 1.],
        );
        p.card_outline(
            &mut shapes,
            [20., 20., 40., 40.],
            [12., 0., 0., 12.],
            2.,
            [0., 0., 1., 1.],
        );
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 80., 80.).unwrap(),
                    shapes: shapes.clone(),
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(
            frame.pixels()[40 * 80 + 18].red() > 0,
            "blur extends beyond silhouette"
        );
        assert_eq!(frame.pixels()[40 * 80 + 40].green(), 255);
        assert!(frame.pixels()[20 * 80 + 58].blue() > 0);
        let misses = p.effect_cache_misses;
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 80., 80.).unwrap(),
                shapes,
                glyphs: vec![],
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(p.effect_cache_misses, misses);
    }
    #[test]
    fn window_sized_shadows_on_huge_displays_are_skipped_not_fatal() {
        // A 6K display at 2x: one sheet shadow alone exceeds the blur budget.
        let (w, h) = (6016., 3384.);
        let mut p = RasterPainter::new(Viewport::new(w, h, 2.).unwrap());
        let mut shapes = vec![];
        p.card_shadow(
            &mut shapes,
            [400., 0., w - 400., h],
            [24.; 4],
            32.,
            0.,
            [0., 0.],
            [0., 0., 0., 0.2],
        );
        p.card_round(&mut shapes, [400., 0., w - 400., h], 24., [1.; 4]);
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., w, h).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert_eq!(p.effects_skipped, 1);
        assert_eq!(frame.pixels()[1000 * w as usize + 3000].red(), 255);
    }
    #[test]
    fn nested_layer_opacity_is_grouped_and_budget_is_explicit() {
        let mut p = RasterPainter::new(Viewport::new(40., 40., 1.).unwrap());
        let batch = |shapes| PaintBatch {
            clip: Rect::new(0., 0., 40., 40.).unwrap(),
            shapes,
            glyphs: vec![],
        };
        let frame = p
            .rasterize(
                &[batch(vec![
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([0., 0., 30., 30.], [1., 0., 0., 1.]),
                    Shape::Rectangle([10., 0., 30., 30.], [0., 1., 0., 1.]),
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([10., 10., 10., 10.], [0., 0., 1., 1.]),
                    Shape::LayerEnd,
                    Shape::LayerEnd,
                ])],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(frame.pixels()[5 * 40 + 15].green().abs_diff(128) <= 1);
        assert!(frame.pixels()[15 * 40 + 15].blue().abs_diff(64) <= 1);
        assert!(p
            .rasterize(&[batch(vec![Shape::LayerEnd])], [0., 0., 0., 1.])
            .is_err());
        assert!(p
            .rasterize(&[batch(vec![Shape::LayerBegin(0.5); 25])], [0., 0., 0., 1.])
            .is_err());
    }
    #[test]
    fn shadow_cache_ignores_color_and_integer_translation_but_is_bounded() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let make = |bounds, color| PaintBatch {
            clip: Rect::new(0., 0., 100., 100.).unwrap(),
            shapes: vec![Shape::Shadow(bounds, [4.; 4], 8., 1., [-2., 3.], color)],
            glyphs: vec![],
        };
        p.rasterize(
            &[make([20.25, 20.5, 30., 30.], [1., 0., 0., 0.5])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        let misses = p.effect_cache_misses;
        p.rasterize(
            &[make([21.25, 22.5, 30., 30.], [0., 1., 0., 0.8])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.effect_cache_misses, misses,
            "color and integer offsets reuse coverage"
        );
        p.rasterize(
            &[make([21.5, 22.5, 30., 30.], [0., 1., 0., 0.8])],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.effect_cache_misses,
            misses + 1,
            "fractional origin changes AA coverage"
        );
        for width in 20..65 {
            p.rasterize(
                &[make([10., 10., width as f32, 30.], [1.; 4])],
                [0., 0., 0., 1.],
            )
            .unwrap();
        }
        assert_eq!(p.effect_masks.len(), EFFECT_CACHE_ENTRIES);
        assert!(p.effect_bytes <= EFFECT_CACHE_BYTES);
        assert!(p.effect_tint_bytes <= EFFECT_TINT_BYTES);
        assert_eq!(
            p.effect_tint_bytes,
            p.effect_masks
                .values()
                .filter_map(|m| m.tint.as_ref())
                .map(|(_, t)| t.data().len())
                .sum::<usize>(),
            "mask LRU eviction also releases tinted RGBA bytes"
        );
    }
    #[test]
    fn shadow_tint_cache_reuses_rgb_during_alpha_only_animation() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let batch = |alpha: f32, rgb: [f32; 3]| PaintBatch {
            clip: Rect::new(0., 0., 100., 100.).unwrap(),
            shapes: vec![Shape::Shadow(
                [20., 20., 50., 50.],
                [8.; 4],
                8.,
                0.,
                [0., 0.],
                [rgb[0], rgb[1], rgb[2], alpha],
            )],
            glyphs: vec![],
        };
        for alpha in [0.1, 0.2, 0.5, 1., 0.4] {
            p.rasterize(&[batch(alpha, [0.4, 0.7, 1.])], [0., 0., 0., 1.])
                .unwrap();
        }
        assert_eq!(p.effect_cache_misses, 1);
        assert_eq!(
            p.effect_tint_misses, 1,
            "alpha-only frames must not rebuild coverage or RGB pixels"
        );
        p.rasterize(&[batch(0.4, [1., 0.7, 0.4])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(p.effect_cache_misses, 1);
        assert_eq!(
            p.effect_tint_misses, 2,
            "RGB change rebuilds tint, not blur geometry"
        );
        assert!(p.effect_tint_bytes <= EFFECT_TINT_BYTES);
    }
    #[test]
    fn tint_spans_match_source_over_with_fractional_clip_and_transparent_groups() {
        let mut source = Pixmap::new(8, 7).unwrap();
        for (i, pixel) in source.pixels_mut().iter_mut().enumerate() {
            *pixel =
                tiny_skia::ColorU8::from_rgba(91, 173, 237, (i * 37 % 256) as u8).premultiply();
        }
        let clip = [2.25, 1.5, 9.5, 8.2];
        let mut mask = Mask::new(17, 13).unwrap();
        fill_mask(&mut mask, 17, 13, clip, 255);
        for background in [[0., 0., 0., 0.], [0.4, 0.2, 0.7, 0.3], [0.7, 0.5, 0.1, 1.]] {
            for position in [[0, 0], [3, 2], [-3, -2], [14, 10], [-30, -20]] {
                for opacity in [0, 1, 64, 128, 254, 255] {
                    let mut actual = Pixmap::new(17, 13).unwrap();
                    actual.fill(
                        tiny_skia::Color::from_rgba(
                            background[0],
                            background[1],
                            background[2],
                            background[3],
                        )
                        .unwrap(),
                    );
                    let mut oracle = actual.clone();
                    let mut grouped = actual.clone();
                    blit_tint(&mut actual, &source, &mask, position, clip, opacity);
                    let mut group = Pixmap::new(17, 13).unwrap();
                    blit_tint(&mut group, &source, &mask, position, clip, 255);
                    composite_layer(&mut grouped, &group, opacity);
                    // Bound the pattern oracle to the source bitmap. Its Pad
                    // shader can extend a negative-position edge by one pixel;
                    // a shadow blit must not draw outside the coverage image.
                    let mut bounded_mask = mask.clone();
                    for (i, value) in bounded_mask.data_mut().iter_mut().enumerate() {
                        let x = (i % 17) as i32;
                        let y = (i / 17) as i32;
                        if x < position[0]
                            || x >= position[0] + source.width() as i32
                            || y < position[1]
                            || y >= position[1] + source.height() as i32
                        {
                            *value = 0;
                        }
                    }
                    oracle.draw_pixmap(
                        position[0],
                        position[1],
                        source.as_ref(),
                        &tiny_skia::PixmapPaint {
                            opacity: f32::from(opacity) / 255.,
                            ..Default::default()
                        },
                        Transform::identity(),
                        Some(&bounded_mask),
                    );
                    assert!(actual.data().iter().zip(oracle.data()).all(|(a, b)| a.abs_diff(*b) <= 1),
                        "source-over mismatch at {position:?}, opacity={opacity}, background={background:?}: {:?}",
                        actual.pixels().iter().zip(oracle.pixels()).enumerate().filter(|(_, (a,b))|
                            [a.red().abs_diff(b.red()),a.green().abs_diff(b.green()),a.blue().abs_diff(b.blue()),a.alpha().abs_diff(b.alpha())].into_iter().any(|d|d>1)).take(5).collect::<Vec<_>>());
                    assert!(actual.pixels().iter().all(|p| p.red() <= p.alpha()
                        && p.green() <= p.alpha()
                        && p.blue() <= p.alpha()));
                    assert!(grouped.data().iter().zip(oracle.data()).all(|(a,b)|a.abs_diff(*b)<=1),
                        "group composite mismatch at {position:?}, opacity={opacity}, background={background:?}");
                }
            }
        }
    }
    #[test]
    fn effects_respect_ancestor_clip_and_report_oversized_work() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        let batch = |shape| PaintBatch {
            clip: Rect::new(20., 0., 60., 100.).unwrap(),
            shapes: vec![shape],
            glyphs: vec![],
        };
        let image = p
            .rasterize(
                &[batch(Shape::Shadow(
                    [25., 25., 50., 50.],
                    [0.; 4],
                    12.,
                    0.,
                    [0., 0.],
                    [1., 0., 0., 1.],
                ))],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(
            image.pixels()[50 * 100 + 21].red() > 0,
            "shadow may extend beyond node bounds"
        );
        assert_eq!(
            image.pixels()[50 * 100 + 19].red(),
            0,
            "ancestor clip still limits effects"
        );
        // Valid but oversized work is skipped and reported, not fatal.
        let skipped = p.effects_skipped;
        p.rasterize(
            &[batch(Shape::Shadow(
                [0., 0., 3000., 3000.],
                [0.; 4],
                1.,
                0.,
                [0., 0.],
                [1.; 4],
            ))],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(p.effects_skipped, skipped + 1);
        // Malformed geometry is still rejected.
        for shape in [
            Shape::Shadow([0., 0., f32::MAX, 30.], [0.; 4], 1., 0., [0., 0.], [1.; 4]),
            Shape::Shadow([0., 0., 30., 30.], [0.; 4], f32::NAN, 0., [0., 0.], [1.; 4]),
        ] {
            assert!(p.rasterize(&[batch(shape)], [0., 0., 0., 1.]).is_err());
        }
        p.set_viewport(Viewport::new(2000., 2000., 1.).unwrap());
        let layer_batch = PaintBatch {
            clip: Rect::new(0., 0., 2000., 2000.).unwrap(),
            shapes: vec![Shape::LayerBegin(0.5); 4],
            glyphs: vec![],
        };
        assert!(p
            .rasterize(&[layer_batch], [0., 0., 0., 1.])
            .unwrap_err()
            .contains("unbalanced"));
    }
    #[test]
    fn retina_nested_opacity_fallback_is_bounded_and_preserves_alpha() {
        let make_batches = |w: f32, h: f32| {
            vec![PaintBatch {
                clip: crate::Rect::new(0., 0., w, h).unwrap(),
                shapes: vec![
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([0., 0., w, h], [1.; 4]),
                    Shape::LayerBegin(0.5),
                    Shape::Rectangle([0., 0., w, h], [0., 0., 1., 1.]),
                    Shape::LayerEnd,
                    Shape::LayerEnd,
                ],
                glyphs: vec![],
            }]
        };
        let background = [0.1, 0.2, 0.3, 0.42];
        let mut small = RasterPainter::new(Viewport::new(16., 16., 1.).unwrap());
        let expected = small
            .rasterize(&make_batches(16., 16.), background)
            .unwrap();
        let viewport = Viewport::new(3028., 2074., 2.).unwrap();
        let mut painter = RasterPainter::new(viewport);
        let actual = painter
            .rasterize(&make_batches(3028., 2074.), background)
            .expect("valid opacity groups must render at Retina window sizes");
        assert_eq!(painter.viewport(), viewport, "restore the native viewport");
        for (x, y) in [(0, 0), (511, 511), (512, 512), (3027, 2073)] {
            assert_eq!(
                actual.pixel(x, y),
                expected.pixel(8, 8),
                "tile seam at {x},{y}"
            );
        }
    }

    #[test]
    fn tiled_opacity_matches_clips_paths_images_shadows_and_text_without_seams() {
        let viewport = Viewport::new(770., 641., 1.).unwrap();
        let mut p = RasterPainter::new(viewport);
        let mut glyphs = vec![];
        p.ui_text_styled(
            &mut glyphs,
            "Text spanning the tile boundary",
            [460.25, 502.5, 300., 80.],
            &TextStyle::default(),
            [0.2, 0.4, 0.8, 0.8],
        );
        let image = crate::graphics::Image::from_rgba(8, 8, [40, 150, 90, 180].repeat(64)).unwrap();
        let clip = crate::Rect::new(0., 0., 770., 641.).unwrap();
        let batches = vec![
            PaintBatch {
                clip,
                shapes: vec![
                    Shape::ClipBegin(ClipRegion::Rounded {
                        bounds: [10.25, 11.5, 750., 610.],
                        radii: [20.; 4],
                    }),
                    Shape::LayerBegin(0.65),
                    Shape::Shadow(
                        [490.25, 480.5, 180., 100.],
                        [12.; 4],
                        8.,
                        2.,
                        [2., 3.],
                        [0., 0., 0., 0.4],
                    ),
                    Shape::Rounded([490.25, 480.5, 180., 100.], 12., [1., 0.5, 0.2, 0.8]),
                    Shape::LayerBegin(0.55),
                    Shape::Path(
                        VectorPath::ellipse(511.25, 510.5, 150., 90.),
                        [0.1, 0.2, 0.7, 0.9],
                        Some(3.),
                    ),
                    Shape::Image([500.25, 488.5, 40., 90.], image.background(), 1.),
                ],
                glyphs,
            },
            PaintBatch {
                clip,
                shapes: vec![Shape::LayerEnd, Shape::LayerEnd, Shape::ClipEnd],
                glyphs: vec![],
            },
        ];
        let background = [0.3, 0.2, 0.5, 0.42];
        let expected = p.rasterize(&batches, background).unwrap();
        let actual = p.rasterize_tiled(&batches, background).unwrap();
        let max_delta = expected
            .data()
            .iter()
            .zip(actual.data())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        let (index, _) = expected
            .data()
            .iter()
            .zip(actual.data())
            .enumerate()
            .max_by_key(|(_, (a, b))| a.abs_diff(**b))
            .unwrap();
        // tiny-skia may tessellate a clipped curved stroke slightly differently;
        // permit bounded edge coverage variation, not shifted text/images.
        assert!(
            max_delta <= 8,
            "tile translation changed pixels by {max_delta} at {},{} expected {:?} actual {:?}",
            index / 4 % 770,
            index / 4 / 770,
            expected.pixel((index / 4 % 770) as u32, (index / 4 / 770) as u32),
            actual.pixel((index / 4 % 770) as u32, (index / 4 / 770) as u32)
        );
        // Isolate the curved-stroke AA tolerance from image/text positioning:
        // those must still match to one byte, including negative half-pixels.
        let mut without_stroke = batches.clone();
        without_stroke[0]
            .shapes
            .retain(|s| !matches!(s, Shape::Path(..)));
        let expected = p.rasterize(&without_stroke, background).unwrap();
        let actual = p.rasterize_tiled(&without_stroke, background).unwrap();
        let delta = expected
            .data()
            .iter()
            .zip(actual.data())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap();
        assert!(
            delta <= 1,
            "text/image translation changed pixels by {delta}"
        );
        assert_eq!(p.viewport(), viewport);
        assert!(
            p.layer_pool.is_empty(),
            "do not retain tile buffers at native size"
        );
        for shapes in [
            vec![Shape::LayerBegin(f32::NAN), Shape::LayerEnd],
            vec![Shape::LayerEnd],
            vec![Shape::LayerBegin(0.5); MAX_LAYER_DEPTH + 1],
        ] {
            let invalid = [PaintBatch {
                clip,
                shapes,
                glyphs: vec![],
            }];
            assert!(p.rasterize(&invalid, background).is_err());
            assert!(p.rasterize_tiled(&invalid, background).is_err());
        }
        assert_eq!(
            p.viewport(),
            viewport,
            "error must not leave a tile viewport"
        );
        assert_eq!(p.raster_origin, [0; 2]);
    }

    #[test]
    fn layer_opacity_applies_to_glyphs_without_opaque_pixel_artifacts() {
        let mut p = RasterPainter::new(Viewport::new(100., 60., 1.).unwrap());
        let clip = Rect::new(0., 0., 100., 60.).unwrap();
        let mut glyphs = vec![];
        p.ui_text(&mut glyphs, "MMMM", [10., 5., 90., 55.], 30., [1.; 4]);
        let plain = p
            .rasterize(
                &[PaintBatch {
                    clip,
                    shapes: vec![],
                    glyphs: glyphs.clone(),
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        let layered = p
            .rasterize(
                &[
                    PaintBatch {
                        clip,
                        shapes: vec![Shape::LayerBegin(0.5)],
                        glyphs,
                    },
                    PaintBatch {
                        clip,
                        shapes: vec![Shape::LayerEnd],
                        glyphs: vec![],
                    },
                ],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert!(plain.pixels().iter().any(|p| p.red() > 200));
        for (before, after) in plain.pixels().iter().zip(layered.pixels()) {
            assert!(
                after
                    .red()
                    .abs_diff((before.red() as f32 * 0.5).round() as u8)
                    <= 1
            );
        }
    }
    #[test]
    fn reusable_opacity_layers_clear_pixels_and_resize_clears_pool() {
        let mut p = RasterPainter::new(Viewport::new(60., 60., 1.).unwrap());
        let batch = |rect| PaintBatch {
            clip: Rect::new(0., 0., 60., 60.).unwrap(),
            shapes: vec![
                Shape::LayerBegin(0.5),
                Shape::Rounded(rect, 8., [1., 0., 0., 1.]),
                Shape::LayerEnd,
            ],
            glyphs: vec![],
        };
        p.rasterize(&[batch([0., 0., 30., 30.])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(p.layer_allocations, 1);
        let next = p
            .rasterize(&[batch([30., 30., 30., 30.])], [0., 0., 0., 1.])
            .unwrap();
        assert_eq!(
            p.layer_allocations, 1,
            "animation frames reuse transparent buffers"
        );
        assert_eq!(
            next.pixels()[15 * 60 + 15].red(),
            0,
            "previous frame pixels must be cleared"
        );
        assert!(next.pixels()[45 * 60 + 45].red().abs_diff(128) <= 1);
        p.set_viewport(Viewport::new(80., 60., 1.).unwrap());
        assert!(p.layer_pool.is_empty());
    }
    #[test]
    fn equal_corner_paths_preserve_uniform_geometry() {
        let a = rounded([5.25, 8.5, 70.75, 40.5], 12.).unwrap();
        let b = rounded_corners([5.25, 8.5, 70.75, 40.5], [12.; 4]).unwrap();
        assert_eq!(a, b);
    }
    #[test]
    fn fast_rounded_fill_matches_original_path_exactly() {
        for scale in [1., 1.5, 2.] {
            for alpha in [0., 0.18, 0.5, 1.] {
                for fractional in [0., 0.25, 0.7] {
                    let mut p = RasterPainter::new(Viewport::new(280., 220., scale).unwrap());
                    let bounds = [
                        (10. + fractional) * scale,
                        (12. + fractional) * scale,
                        112.3 * scale,
                        84.7 * scale,
                    ];
                    let radius = 12.4 * scale;
                    let color = [0.13, 0.75, 0.38, alpha];
                    let clip =
                        Rect::new(9.3 * scale, 13.2 * scale, 96.8 * scale, 74.3 * scale).unwrap();
                    let fast = p
                        .rasterize(
                            &[PaintBatch {
                                clip,
                                shapes: vec![Shape::Rounded(bounds, radius, color)],
                                glyphs: vec![],
                            }],
                            [0.09, 0.1, 0.11, 1.],
                        )
                        .unwrap();
                    let mut oracle = Pixmap::new(280, 220).unwrap();
                    oracle.fill(tiny_skia::Color::from_rgba(0.09, 0.1, 0.11, 1.).unwrap());
                    let mut mask = Mask::new(280, 220).unwrap();
                    fill_mask(&mut mask, 280, 220, clip.bounds(), 255);
                    oracle.fill_path(
                        &rounded(bounds, radius).unwrap(),
                        &shape_paint(color),
                        FillRule::Winding,
                        Transform::identity(),
                        Some(&mask),
                    );
                    let first = fast
                        .pixels()
                        .iter()
                        .zip(oracle.pixels())
                        .position(|(a, b)| a != b);
                    assert!(
                        first.is_none(),
                        "scale={scale} alpha={alpha} fractional={fractional} first={first:?} pixels={:?}", first.map(|i| (fast.pixels()[i], oracle.pixels()[i]))
                    );
                }
            }
        }
    }
    #[test]
    fn font_attributes_are_cached_and_match_drawing() {
        let mut p = RasterPainter::new(Viewport::new(300., 100., 1.).unwrap());
        let base = TextStyle {
            family: "sans-serif".into(),
            size: 16.,
            ..TextStyle::default()
        };
        let bold = TextStyle {
            weight: 700,
            ..base.clone()
        };
        let italic = TextStyle {
            italic: true,
            ..base.clone()
        };
        let mono = TextStyle {
            family: "monospace".into(),
            ..base.clone()
        };
        for style in [&base, &bold, &italic, &mono] {
            let measured = p.measure_styled("Window typography", style, 280.);
            assert!(measured[0] > 0. && measured[1] > 0.);
        }
        assert_eq!(
            p.text.len(),
            4,
            "family/weight/italic need independent shaping keys"
        );
        let misses = p.text_cache_misses;
        let mut glyphs = vec![];
        p.ui_text_styled(
            &mut glyphs,
            "Window typography",
            [0., 0., 280., 100.],
            &bold,
            [1.; 4],
        );
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 300., 100.).unwrap(),
                shapes: vec![],
                glyphs,
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert_eq!(
            p.text_cache_misses, misses,
            "draw must use the measured font attributes"
        );
    }
    #[test]
    fn native_panel_border_is_one_stroke_not_seventy_two_triangles() {
        let mut p = RasterPainter::new(Viewport::new(120., 100., 1.).unwrap());
        let mut shapes = vec![];
        let panel = crate::Panel::new(
            Rect::new(10., 10., 100., 70.).unwrap(),
            12.,
            1.,
            crate::SurfaceStyle::from_theme(&crate::Theme::dark()),
        )
        .unwrap();
        panel.paint(&p, &mut shapes);
        assert!(shapes.iter().all(|s| !matches!(s, Shape::Triangle(..))));
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 120., 100.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert_eq!(
            p.shape_draw_calls, 4,
            "shadow, fill, batched border, highlight"
        );
        assert!(frame.pixels()[10 * 120 + 60].red() > 0);
        assert!(frame.pixels()[50 * 120 + 60].red() > 0);
    }
    #[test]
    fn stroke_batching_respects_order_style_and_clip() {
        let mut p = RasterPainter::new(Viewport::new(50., 50., 1.).unwrap());
        let shapes = vec![
            Shape::Line([5., 10.], [45., 10.], 4., [1., 0., 0., 1.]),
            Shape::Line([5., 20.], [45., 20.], 4., [1., 0., 0., 1.]),
            Shape::Rectangle([10., 8., 10., 5.], [0., 1., 0., 1.]),
            Shape::Line([5., 30.], [45., 30.], 4., [0., 0., 1., 1.]),
        ];
        let frame = p
            .rasterize(
                &[PaintBatch {
                    clip: Rect::new(0., 0., 35., 50.).unwrap(),
                    shapes,
                    glyphs: vec![],
                }],
                [0., 0., 0., 1.],
            )
            .unwrap();
        assert_eq!(p.shape_draw_calls, 3);
        assert_eq!(
            frame.pixels()[10 * 50 + 15].green(),
            255,
            "rectangle after stroke stays above it"
        );
        assert_eq!(frame.pixels()[20 * 50 + 25].red(), 255);
        assert_eq!(frame.pixels()[30 * 50 + 25].blue(), 255);
        assert_eq!(
            frame.pixels()[20 * 50 + 40].red(),
            0,
            "batched stroke stays clipped"
        );
    }
    #[test]
    fn effective_text_size_and_glyph_cache_budget() {
        let mut p = RasterPainter::new(Viewport::new(100., 100., 1.).unwrap());
        p.measure("x", f32::MIN_POSITIVE, 100.);
        p.measure("x", f32::MAX, 100.);
        assert!(p
            .text
            .keys()
            .all(|key| (1. ..=384.).contains(&f32::from_bits(key.size))));
        p.glyph_bytes = 17 * 1024 * 1024;
        let glyph = Glyph {
            text: "x".into(),
            bounds: [0., 0., 100., 100.],
            style: default_style(12.),
            color: [1.; 4],
        };
        p.rasterize(
            &[PaintBatch {
                clip: Rect::new(0., 0., 100., 100.).unwrap(),
                shapes: vec![],
                glyphs: vec![glyph],
            }],
            [0., 0., 0., 1.],
        )
        .unwrap();
        assert!(p.glyph_bytes < 1024 * 1024);
    }
}

#[cfg(test)]
mod generic_font_tests {
    use super::*;
    #[test]
    fn generic_family_selection_uses_installed_fonts_when_available() {
        let mut fonts = FontSystem::new();
        configure_generic_families(&mut fonts);
        for (family, candidates) in [
            (
                Family::Serif,
                &["Georgia", "Times New Roman", "DejaVu Serif", "Noto Serif"][..],
            ),
            (
                Family::SansSerif,
                &[
                    "Avenir Next",
                    "DejaVu Sans",
                    "Arial",
                    "Noto Sans",
                    "Open Sans",
                ][..],
            ),
            (
                Family::Monospace,
                &[
                    "Menlo",
                    "Consolas",
                    "DejaVu Sans Mono",
                    "Noto Sans Mono",
                    "Courier New",
                ][..],
            ),
        ] {
            if let Some(expected) = candidates.iter().find(|candidate| {
                fonts
                    .db()
                    .faces()
                    .any(|face| face.families.iter().any(|(name, _)| name == *candidate))
            }) {
                let selected = fonts
                    .db()
                    .query(&cosmic_text::fontdb::Query {
                        families: &[family],
                        ..Default::default()
                    })
                    .unwrap();
                let face = fonts.db().face(selected).unwrap();
                assert!(face.families.iter().any(|(name, _)| name == expected));
            }
        }
    }
}

#[cfg(test)]
mod sf_alias_tests {
    use super::*;
    #[test]
    fn sf_requested_weights_shape_with_the_matching_installed_face() {
        for scale in [1., 2.] {
            let mut p =
                RasterPainter::new(Viewport::new(600. * scale, 120. * scale, scale).unwrap());
            for (alias, logical_size) in [("SF", 13.), ("SF", 44.), ("SF Mono", 13.)] {
                for weight in [400, 500, 600, 700] {
                    let size = logical_size * scale;
                    let family = p.resolved_font_family(alias, size);
                    if matches!(family.as_str(), "sans-serif" | "monospace") {
                        continue;
                    }
                    let expected = p
                        .fonts
                        .db()
                        .query(&cosmic_text::fontdb::Query {
                            families: &[Family::Name(&family)],
                            weight: Weight(weight),
                            ..Default::default()
                        })
                        .expect("resolved family must be queryable");
                    let style = TextStyle {
                        family: alias.into(),
                        size,
                        weight,
                        ..Default::default()
                    };
                    let key = p.key("Native Rust 012345", &style, 590. * scale);
                    let buffer = &p.text[&key].0;
                    let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs).collect();
                    assert!(!glyphs.is_empty());
                    assert!(
                        glyphs.iter().all(|g| g.font_id == expected),
                        "{alias}/{weight}/{scale}x selected a different face or fell back"
                    );
                }
            }
        }
    }
    #[test]
    fn sf_aliases_resolve_installed_faces_without_changing_existing_defaults() {
        let mut p = RasterPainter::new(Viewport::new(400., 120., 1.).unwrap());
        assert_eq!(
            p.resolved_font_family("system", 14.),
            system_family(&p.fonts)
        );
        assert_eq!(p.resolved_font_family("monospace", 14.), "monospace");
        for (alias, size, candidates, fallback) in [
            (
                "SF",
                14.,
                &["SF Pro Text", "SF Pro", ".SF NS", "System Font"][..],
                "sans-serif",
            ),
            (
                "SF",
                36.,
                &["SF Pro Display", "SF Pro", ".SF NS", "System Font"][..],
                "sans-serif",
            ),
            ("SF Mono", 14., &["SF Mono", ".SF NS Mono"][..], "monospace"),
        ] {
            let expected = apple_family(&p.fonts, candidates, fallback);
            let resolved = p.resolved_font_family(alias, size);
            assert_eq!(resolved, expected);
            let style = TextStyle {
                family: alias.into(),
                size,
                ..TextStyle::default()
            };
            let key = p.key("Native Rust 012345", &style, 380.);
            let buffer = &p.text[&key].0;
            let glyphs: Vec<_> = buffer.layout_runs().flat_map(|r| r.glyphs).collect();
            assert!(!glyphs.is_empty());
            if resolved != fallback {
                assert!(
                    glyphs.iter().all(|g| p
                        .fonts
                        .db()
                        .face(g.font_id)
                        .unwrap()
                        .families
                        .iter()
                        .any(|(name, _)| name == &resolved)),
                    "alias must shape with the resolved SF face"
                );
            }
        }
        let text_family = p.resolved_font_family("SF", 14.);
        let display_family = p.resolved_font_family("SF", 36.);
        p.set_viewport(Viewport::new(800., 240., 2.).unwrap());
        assert_eq!(p.resolved_font_family("SF", 28.), text_family);
        assert_eq!(p.resolved_font_family("SF", 72.), display_family);
        assert_eq!(
            p.resolved_font_family("SF Pro", 36.),
            p.resolved_font_family("SF", 36.)
        );
    }
}
#[cfg(test)]
mod scaled_image_tests {
    use super::*;
    use crate::declarative::PaintBatch;
    use crate::graphics::Image;
    fn frame(shapes: Vec<Shape>) -> Pixmap {
        let mut p = RasterPainter::new(Viewport::new(40., 40., 1.).unwrap());
        let batch = PaintBatch {
            clip: crate::geometry::Rect::new(0., 0., 40., 40.).unwrap(),
            shapes,
            glyphs: vec![],
        };
        p.rasterize(&[batch], [0., 0., 1., 1.]).unwrap()
    }
    #[test]
    fn opaque_images_blit_through_offscreen_origins_and_rounded_clips() {
        let red = Image::from_rgba(4, 4, [255, 0, 0, 255].repeat(16)).unwrap();
        // Starts above/left of the framebuffer, so source offsets must not wrap.
        let pixmap = frame(vec![
            Shape::ClipBegin(ClipRegion::Rounded {
                bounds: [0., 0., 30., 30.],
                radii: [10.; 4],
            }),
            Shape::Image([-10., -10., 40., 40.], red.background(), 1.),
            Shape::ClipEnd,
        ]);
        let px = |x: u32, y: u32| pixmap.pixel(x, y).unwrap();
        assert_eq!((px(15, 15).red(), px(15, 15).blue()), (255, 0));
        // Outside the rounded corner and outside the clip: background shows.
        assert_eq!((px(0, 0).red(), px(0, 0).blue()), (0, 255));
        assert_eq!((px(35, 35).red(), px(35, 35).blue()), (0, 255));
    }
    #[test]
    fn translucent_images_keep_the_pattern_path_and_blend() {
        let half = Image::from_rgba(2, 2, [255, 0, 0, 128].repeat(4)).unwrap();
        let pixmap = frame(vec![Shape::Image(
            [0., 0., 40., 40.],
            half.background(),
            1.,
        )]);
        let p = pixmap.pixel(20, 20).unwrap();
        assert!(
            p.red() > 100 && p.blue() > 100,
            "blended {:?}",
            (p.red(), p.blue())
        );
    }
}

#[cfg(test)]
mod editor_metric_tests {
    use super::*;
    #[test]
    fn editor_advances_share_shaped_clusters_without_prefix_buffers() {
        for scale in [1., 2.] {
            let mut painter =
                RasterPainter::new(Viewport::new(400. * scale, 160. * scale, scale).unwrap());
            let style = TextStyle {
                family: "SF".into(),
                ..default_style(14. * scale)
            };
            let value = "AV office e\u{301} 👨‍👩‍👧‍👦";
            let offsets: Vec<_> = value
                .grapheme_indices(true)
                .map(|(i, _)| i)
                .chain(std::iter::once(value.len()))
                .collect();
            let advances = painter.line_advances(value, &style, &offsets);
            let width = painter.measure_styled(value, &style, 1_000_000.)[0];
            assert_eq!(advances.len(), offsets.len());
            assert!(advances.iter().all(|x| x.is_finite() && *x >= 0.));
            assert!((advances.last().unwrap() - width).abs() < 0.1);
            assert_eq!(
                painter.text_cache_misses, 1,
                "one shaped line, not one prefix buffer per grapheme"
            );
            assert_eq!(painter.line_advances("", &style, &[0]), vec![0.]);
        }
    }
}

#[cfg(test)]
mod damage_regression_tests {
    use super::*;
    use crate::Rect;
    #[test]
    fn acute_miter_repaint_matches_full_frame() {
        let vp = Viewport::new(300., 300., 1.).unwrap();
        let mut p = RasterPainter::new(vp);
        let path = crate::graphics::VectorPath::new()
            .move_to(140., 160.)
            .line_to(150., 130.)
            .line_to(160., 160.);
        for lines in [false, true] {
            let frame = |c| PaintBatch {
                clip: Rect::new(0., 0., 300., 300.).unwrap(),
                shapes: if lines {
                    vec![
                        Shape::Line([140., 160.], [150., 130.], 12., c),
                        Shape::Line([150., 130.], [160., 160.], 12., c),
                    ]
                } else {
                    vec![Shape::Path(path.clone(), c, Some(12.))]
                },
                glyphs: vec![],
            };
            p.rasterize_incremental(&[frame([1., 0., 0., 1.])], [0., 0., 0., 1.])
                .unwrap();
            let b = frame([0., 1., 0., 1.]);
            let (actual, damage) = p
                .rasterize_incremental(std::slice::from_ref(&b), [0., 0., 0., 1.])
                .unwrap();
            let actual = actual.clone();
            assert!(
                matches!(damage, Damage::Rect(_)),
                "retain partial repaint, not just full fallback"
            );
            let full = p.rasterize(&[b], [0., 0., 0., 1.]).unwrap();
            assert!(
                actual.data() == full.data(),
                "miter damage must include stroke joins (lines={lines})"
            );
        }
    }
}
