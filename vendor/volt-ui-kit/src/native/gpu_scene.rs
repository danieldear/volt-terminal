//! Encode retained native commands into GPU-rendered Vello scenes. No full-frame
//! CPU raster is used. Text shares CPU shaping with the software adapter (same
//! fonts, positions and editor geometry) but is drawn as Vello vector glyphs, so
//! colour changes and animations need no new bitmaps.
//!
//! Vello 0.6 renders into fixed-size GPU buffers and silently drops a frame that
//! overflows any of them, leaving the previous image on screen. The encoder
//! therefore estimates every buffer while encoding; the host draws an
//! over-budget frame in software instead (see `SceneStats::over_budget`).
use super::raster::{self, Glyph, Shape};
use super::RasterPainter;
use crate::{
    declarative::PaintBatch,
    graphics::{BackgroundImage, ClipRegion, Image, ImageFit},
};
use cosmic_text::{CacheKey, SwashContent};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};
use vello::{
    kurbo::{Affine, BezPath, Circle, Rect, Shape as _, Stroke},
    peniko::{
        Blob, Color, Compose, Fill, FontData, ImageAlphaType, ImageBrush, ImageData, ImageFormat,
        ImageQuality, ImageSampler, Mix,
    },
    Scene,
};

#[derive(Default, Debug, Clone, Copy)]
pub(super) struct SceneStats {
    /// Conservative estimate of Vello blend-spill slots this scene needs.
    pub spill_slots: u32,
    /// Name of the first Vello buffer this frame would overflow, if any.
    pub over_budget: Option<&'static str>,
    pub shapes: usize,
    pub glyphs: usize,
    pub layers: usize,
    pub resource_bytes: usize,
    /// Horizontal strips the frame is rendered in (1 = one pass). A frame too
    /// large for Vello's fixed per-frame buffers, but whose strips each fit,
    /// stays on the GPU instead of falling back to software.
    pub bands: usize,
    /// Always zero: scene resources are not a CPU-rendered window framebuffer.
    pub framebuffer_upload_bytes: usize,
}
struct CachedImage {
    source: Image,
    data: ImageData,
    used: u64,
}
struct CachedGlyph {
    data: ImageData,
    offset: [f64; 2],
    used: u64,
}
#[derive(Default)]
pub(super) struct GpuSceneEncoder {
    pub scene: Scene,
    stats: SceneStats,
    images: BTreeMap<usize, CachedImage>,
    glyphs: HashMap<(CacheKey, [u8; 3]), CachedGlyph>,
    image_bytes: usize,
    glyph_bytes: usize,
    clock: u64,
    /// Per-tile current and peak layer depth for this frame (16x16 tiles).
    tiles: [usize; 2],
    depth: Vec<u8>,
    peak: Vec<u8>,
    /// Estimated command words per tile, including draws and clip scopes.
    commands: Vec<u32>,
    open_tiles: Vec<[usize; 4]>,
    /// Zero-copy handles to loaded faces; stable ids keep Vello's outline cache warm.
    fonts: HashMap<cosmic_text::fontdb::ID, Option<FontData>>,
    budget: Budget,
    frame_images: HashSet<usize>,
    /// Physical `[y, height]` strips for a banded frame; empty for one pass.
    pub bands: Vec<[u32; 2]>,
    viewport_height: u32,
}
/// Strips are whole 16px tile rows. A strip that cuts a 256px bin row counts
/// that row's bins in full, which over-estimates binning (the safe side).
const MAX_BANDS: usize = 16;
/// Per-frame usage of the Vello buffers its own estimator does not cover.
#[derive(Default)]
struct Budget {
    /// Tile footprint per 16px tile row; bins per 256px bin row.
    row_tiles: Vec<u64>,
    bin_rows: Vec<u64>,
    glyph_lines: u64,
    atlas_area: u64,
    oversize_image: bool,
}
/// Vello 0.6 fixed buffer sizes (vello_encoding `BufferSizes::new`), in elements.
const TILE_CAPACITY: u64 = 1 << 21;
const PTCL_CAPACITY: u64 = 1 << 23;
// Pinned Vello 0.6 shared/ptcl.wgsl and coarse.wgsl: the initial allocation
// exists even for empty viewport tiles; spill allocations are 256-word chunks.
const PTCL_INITIAL_ALLOC: u64 = 64;
const PTCL_INCREMENT: u64 = 256;
// Leave room for the blend offset, END/JUMP and command-boundary headroom.
const PTCL_INITIAL_USABLE: u32 = 48;
const PTCL_INCREMENT_USABLE: u32 = 240;
const BIN_CAPACITY: u64 = 1 << 18;
const LINE_CAPACITY: u64 = 1 << 21;
const ATLAS_SIDE: u64 = 8192;
/// Use at most this fraction of any buffer: estimates are approximations.
fn over(used: u64, capacity: u64) -> bool {
    used * 5 > capacity * 4
}
/// `cosmic_text::Font` owns (usually memory-mapped) font bytes; sharing the Arc
/// avoids copying large faces such as colour emoji into Vello.
struct FontBytes(Arc<cosmic_text::Font>);
impl AsRef<[u8]> for FontBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.data()
    }
}
/// Supplies GPU-resident images. Without one, images are uploaded from CPU
/// memory every frame (Vello rebuilds its atlas per frame).
pub(super) type ImageResolver<'a, 'b> =
    &'a mut (dyn FnMut(&Image) -> Result<ImageData, String> + 'b);
/// Vello keeps this many nested layers on chip; deeper tiles spill.
const BLEND_STACK_SPLIT: u8 = 4;
/// Vello 0.6's fixed blend spill buffer holds 4096 tile-layer slots per frame;
/// exceeding it silently drops the whole frame. Keep a safety margin.
pub(super) const SPILL_SLOT_LIMIT: u32 = 3584;
const IMAGE_BUDGET: usize = 64 * 1024 * 1024;
const GLYPH_BUDGET: usize = 16 * 1024 * 1024;
fn byte(v: f32) -> u8 {
    (v.clamp(0., 1.) * 255.).round() as u8
}
pub(super) fn color(c: [f32; 4]) -> Color {
    Color::from_rgba8(byte(c[0]), byte(c[1]), byte(c[2]), byte(c[3]))
}
fn rect([x, y, w, h]: [f32; 4]) -> Rect {
    Rect::new(x as f64, y as f64, (x + w) as f64, (y + h) as f64)
}
fn path(p: &tiny_skia::Path) -> BezPath {
    let mut out = BezPath::new();
    for segment in p.segments() {
        use tiny_skia::PathSegment::*;
        let pt = |p: tiny_skia::Point| (p.x as f64, p.y as f64);
        match segment {
            MoveTo(p) => out.move_to(pt(p)),
            LineTo(p) => out.line_to(pt(p)),
            QuadTo(c, p) => out.quad_to(pt(c), pt(p)),
            CubicTo(a, b, p) => out.curve_to(pt(a), pt(b), pt(p)),
            Close => out.close_path(),
        }
    }
    out
}
fn corners(b: [f32; 4], r: [f32; 4]) -> Result<BezPath, String> {
    raster::rounded_corners(b, r)
        .map(|p| path(&p))
        .ok_or_else(|| "invalid GPU corner geometry".into())
}
fn image_data(w: u32, h: u32, pixels: Vec<u8>) -> ImageData {
    ImageData {
        data: Blob::new(Arc::new(pixels)),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
        width: w,
        height: h,
    }
}
/// Conservative drawing bounds of one paint command (before batch clipping).
fn shape_bounds(s: &Shape) -> Option<Rect> {
    let pad = |r: Rect, p: f32| r.inflate(p as f64, p as f64);
    Some(match s {
        Shape::Rectangle(b, _)
        | Shape::Rounded(b, ..)
        | Shape::Corners(b, ..)
        | Shape::Gradient(b, ..)
        | Shape::Image(b, ..) => rect(*b),
        Shape::Outline(b, _, w, _) => pad(rect(*b), *w),
        Shape::Triangle(p, _) => {
            let xs = p.iter().map(|q| q[0]);
            let ys = p.iter().map(|q| q[1]);
            Rect::new(
                xs.clone().fold(f32::INFINITY, f32::min) as f64,
                ys.clone().fold(f32::INFINITY, f32::min) as f64,
                xs.fold(f32::NEG_INFINITY, f32::max) as f64,
                ys.fold(f32::NEG_INFINITY, f32::max) as f64,
            )
        }
        Shape::Line(a, b, w, _) => pad(
            Rect::from_points((a[0] as f64, a[1] as f64), (b[0] as f64, b[1] as f64)),
            raster::stroke_extent(*w),
        ),
        Shape::Path(p, _, w) => pad(
            path(&raster::native_path(p)?).bounding_box(),
            raster::stroke_extent(w.unwrap_or(0.)),
        ),
        Shape::Shadow(b, _, blur, spread, offset, _) => pad(
            rect([b[0] + offset[0], b[1] + offset[1], b[2], b[3]]),
            spread.max(0.) + blur * 1.5,
        ),
        _ => return None,
    })
}
/// Bounds of what each opacity layer actually draws, in stream order. The
/// stream gives opacity layers no geometry; sizing them to the enclosing clip
/// made a pressed button deepen every tile of the window.
fn opacity_layer_bounds(batches: &[PaintBatch<Shape, Glyph>]) -> Vec<Rect> {
    let mut out = Vec::new();
    let mut open: Vec<(usize, Option<Rect>)> = Vec::new();
    let add = |open: &mut Vec<(usize, Option<Rect>)>, r: Rect| {
        for (_, acc) in open.iter_mut() {
            *acc = Some(acc.map_or(r, |a| a.union(r)));
        }
    };
    for batch in batches {
        let clip = rect(batch.clip.bounds());
        for s in &batch.shapes {
            match s {
                Shape::LayerBegin(_) => {
                    open.push((out.len(), None));
                    out.push(Rect::ZERO);
                }
                Shape::LayerEnd => {
                    if let Some((i, acc)) = open.pop() {
                        let r = acc.unwrap_or(Rect::ZERO);
                        out[i] = r;
                        // The finished layer is itself content of its parents.
                        add(&mut open, r);
                    }
                }
                s => {
                    if let Some(b) = shape_bounds(s) {
                        add(&mut open, b.intersect(clip));
                    }
                }
            }
        }
        for g in &batch.glyphs {
            add(&mut open, rect(g.bounds).intersect(clip));
        }
    }
    out
}
impl GpuSceneEncoder {
    pub(super) fn reject_frame(&mut self, reason: &'static str) {
        // Do not retain the previous frame's resources/statistics on fallback.
        self.scene.reset();
        self.bands.clear();
        self.stats = SceneStats {
            over_budget: Some(reason),
            ..Default::default()
        };
    }
    pub fn stats(&self) -> SceneStats {
        self.stats
    }
    /// Record a draw object's footprint in tiles, per-tile commands and bins.
    fn note(&mut self, r: Rect) {
        let r = r.intersect(Rect::new(
            0.,
            0.,
            self.tiles[0] as f64 * 16.,
            self.tiles[1] as f64 * 16.,
        ));
        if r.is_zero_area() {
            return;
        }
        let span =
            |a: f64, b: f64, size: f64| ((b / size).ceil() - (a / size).floor()).max(1.) as u64;
        // Fill + brush use at most eight words in pinned coarse.wgsl. Count
        // per tile: a dense tile spills in 256-word chunks, not by exact usage.
        let [tw, th] = self.tiles;
        let x0 = ((r.x0 / 16.).floor() as usize).min(tw);
        let x1 = ((r.x1 / 16.).ceil() as usize).min(tw);
        let y0 = ((r.y0 / 16.).floor() as usize).min(th);
        let y1 = ((r.y1 / 16.).ceil() as usize).min(th);
        for y in y0..y1 {
            self.budget.row_tiles[y] += (x1 - x0).max(1) as u64;
            for words in &mut self.commands[y * tw + x0..y * tw + x1] {
                *words = words.saturating_add(8);
            }
        }
        let bins = span(r.x0, r.x1, 256.);
        let by0 = (r.y0 / 256.).floor() as usize;
        let by1 = ((r.y1 / 256.).ceil() as usize).max(by0 + 1);
        for by in by0..by1.min(self.budget.bin_rows.len()) {
            self.budget.bin_rows[by] += bins;
        }
    }
    /// Area-dependent Vello buffers (blend spill, tiles, tile commands,
    /// bins) for the tile rows `rows`, as one render pass would use them.
    fn area_budget(&self, rows: std::ops::Range<usize>) -> Option<&'static str> {
        let tw = self.tiles[0];
        let cells = rows.start * tw..rows.end * tw;
        let spill: u32 = self.peak[cells.clone()]
            .iter()
            .map(|d| u32::from(d.saturating_sub(BLEND_STACK_SPLIT)))
            .sum();
        let ptcl = cells.len() as u64 * PTCL_INITIAL_ALLOC
            + self.commands[cells]
                .iter()
                .map(|words| {
                    u64::from(
                        words
                            .saturating_sub(PTCL_INITIAL_USABLE)
                            .div_ceil(PTCL_INCREMENT_USABLE),
                    ) * PTCL_INCREMENT
                })
                .sum::<u64>();
        let tiles: u64 = self.budget.row_tiles[rows.clone()].iter().sum();
        let bins: u64 = self.budget.bin_rows
            [rows.start / 16..rows.end.div_ceil(16).min(self.budget.bin_rows.len())]
            .iter()
            .sum();
        if spill > SPILL_SLOT_LIMIT {
            Some("blend spill")
        } else if over(tiles, TILE_CAPACITY) {
            Some("tiles")
        } else if over(ptcl, PTCL_CAPACITY) {
            Some("tile commands")
        } else if over(bins, BIN_CAPACITY) {
            Some("binning")
        } else {
            None
        }
    }
    fn finish_budget(&mut self) {
        let th = self.tiles[1];
        self.stats.spill_slots = self
            .peak
            .iter()
            .map(|d| u32::from(d.saturating_sub(BLEND_STACK_SPLIT)))
            .sum();
        // Vello estimates path flattening itself, but not glyph outlines, which
        // it resolves later; those are added from our own per-glyph estimate.
        // Flattening covers the whole scene in every pass, so strips do not
        // reduce these.
        let vello = self.scene.bump_estimate(None);
        let lines = u64::from(vello.lines.len()) + self.budget.glyph_lines;
        let segments = u64::from(vello.segments.len()) + self.budget.glyph_lines * 3 / 2;
        let b = &self.budget;
        let global = if over(lines, LINE_CAPACITY) {
            Some("lines")
        } else if over(segments, LINE_CAPACITY) {
            Some("segments")
        } else if b.oversize_image || over(b.atlas_area, ATLAS_SIDE * ATLAS_SIDE) {
            Some("image atlas")
        } else {
            None
        };
        self.bands.clear();
        self.stats.bands = 1;
        self.stats.over_budget = global.or_else(|| self.area_budget(0..th));
        if global.is_some() || self.stats.over_budget.is_none() {
            return;
        }
        // Too big for one pass: find the fewest equal strips that each fit.
        // Large displays (5K/6K Retina) exceed the fixed tile-command buffer
        // on its per-tile reservation alone, before any content is drawn.
        for n in 2..=MAX_BANDS {
            let rows = th.div_ceil(n);
            if rows == 0 {
                break;
            }
            let strips: Vec<_> = (0..th)
                .step_by(rows)
                .map(|y| y..(y + rows).min(th))
                .collect();
            if strips.iter().all(|r| self.area_budget(r.clone()).is_none()) {
                let height = self.viewport_height;
                self.bands = strips
                    .iter()
                    .map(|r| {
                        let y = (r.start * 16) as u32;
                        [y, ((r.end * 16) as u32).min(height) - y]
                    })
                    .collect();
                self.stats.bands = self.bands.len();
                self.stats.over_budget = None;
                return;
            }
        }
    }
    fn layer(
        &mut self,
        blend: impl Into<vello::peniko::BlendMode>,
        alpha: f32,
        shape: &impl vello::kurbo::Shape,
    ) {
        let b = shape.bounding_box();
        let [tw, th] = self.tiles;
        let tile = |v: f64, max: usize| ((v / 16.).floor().max(0.) as usize).min(max);
        let r = [
            tile(b.x0, tw),
            tile(b.y0, th),
            tile((b.x1 / 16.).ceil() * 16., tw),
            tile((b.y1 / 16.).ceil() * 16., th),
        ];
        for y in r[1]..r[3] {
            for i in y * tw + r[0]..y * tw + r[2] {
                self.depth[i] = self.depth[i].saturating_add(1);
                self.peak[i] = self.peak[i].max(self.depth[i]);
            }
        }
        self.open_tiles.push(r);
        // A layer is two draw objects (begin/end) over its clip path's tiles.
        self.note(b);
        self.note(b);
        self.scene.push_layer(blend, alpha, Affine::IDENTITY, shape);
    }
    fn pop_layer(&mut self) {
        if let Some(r) = self.open_tiles.pop() {
            let tw = self.tiles[0];
            for y in r[1]..r[3] {
                for i in y * tw + r[0]..y * tw + r[2] {
                    self.depth[i] = self.depth[i].saturating_sub(1);
                }
            }
        }
        self.scene.pop_layer();
    }
    /// Encode with CPU-memory images (headless verification only).
    #[cfg(test)]
    pub fn encode(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
    ) -> Result<(), String> {
        self.encode_with(batches, painter, None)
    }
    /// Encode a frame. `stats().over_budget` names the first Vello buffer it
    /// would overflow; the host must then draw it another way, not present a
    /// stale or blank window.
    pub fn encode_with(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
        mut images: Option<ImageResolver<'_, '_>>,
    ) -> Result<(), String> {
        self.scene.reset();
        self.bands.clear();
        self.stats = SceneStats::default();
        self.budget = Budget::default();
        self.frame_images.clear();
        self.clock += 1;
        let vp = painter.gpu_viewport();
        if vp.width <= 0.
            || vp.height <= 0.
            || vp.width as f64 * vp.height as f64 > raster::MAX_FRAMEBUFFER_PIXELS as f64
        {
            return Err("GPU framebuffer exceeds 64 megapixel limit".into());
        }
        let viewport = Rect::new(0., 0., vp.width as f64, vp.height as f64);
        self.tiles = [
            (vp.width / 16.).ceil() as usize,
            (vp.height / 16.).ceil() as usize,
        ];
        let count = self.tiles[0] * self.tiles[1];
        self.depth.clear();
        self.depth.resize(count, 0);
        self.peak.clear();
        self.peak.resize(count, 0);
        self.commands.clear();
        self.commands.resize(count, 0);
        self.budget.row_tiles = vec![0; self.tiles[1]];
        self.budget.bin_rows = vec![0; self.tiles[1].div_ceil(16)];
        self.viewport_height = vp.height as u32;
        self.open_tiles.clear();
        let mut stack = Vec::<bool>::new();
        // Pixel bounds of the innermost structural clip. Opacity layers and
        // path-clip layers use it instead of the whole window: Vello spills
        // every tile of a layer nested deeper than its on-chip blend stack into
        // a fixed buffer, and an overflow drops the entire frame.
        let mut bounds = vec![viewport];
        let mut layer_areas = opacity_layer_bounds(batches).into_iter();
        let mut masks = Vec::<Option<BezPath>>::new();
        let (mut layers, mut clips) = (0, 0);
        // The innermost open batch clip. Consecutive batches usually share one
        // clip (often the whole scroll viewport), so they reuse a single layer;
        // full-window clips need no layer. Each Vello layer reserves tiles and
        // blend memory over its bounds, and its fixed buffers drop the whole
        // frame on overflow, so per-batch layers blanked large Retina scenes.
        // (clip, whether a layer was actually pushed for it)
        let mut open: Option<([f32; 4], bool)> = None;
        for batch in batches {
            let clip = batch.clip.bounds();
            for s in &batch.shapes {
                if matches!(
                    s,
                    Shape::LayerBegin(_) | Shape::LayerEnd | Shape::ClipBegin(_) | Shape::ClipEnd
                ) {
                    if open.take().is_some_and(|(_, pushed)| pushed) {
                        self.pop_layer();
                    }
                    match s {
                        Shape::LayerBegin(alpha) => {
                            if !alpha.is_finite() || !(0. ..=1.).contains(alpha) || layers >= 24 {
                                return Err("invalid GPU layer opacity/depth".into());
                            }
                            // Pixel-aligned so edge antialiasing is never cut.
                            let content = layer_areas.next().unwrap_or(viewport).expand();
                            let area = bounds.last().unwrap().intersect(content);
                            self.layer(Mix::Normal, *alpha, &area);
                            bounds.push(area);
                            stack.push(true);
                            layers += 1;
                            self.stats.layers += 1;
                        }
                        Shape::ClipBegin(region) => {
                            if clips >= 48 {
                                return Err("GPU clip depth exceeds limit".into());
                            }
                            match region {
                                ClipRegion::Rounded { bounds, radii } => {
                                    self.layer(Mix::Normal, 1., &corners(*bounds, *radii)?)
                                }
                                ClipRegion::Circle {
                                    bounds: [x, y, w, h],
                                } => self.layer(
                                    Mix::Normal,
                                    1.,
                                    &Circle::new(
                                        ((x + w * 0.5) as f64, (y + h * 0.5) as f64),
                                        w.min(*h) as f64 * 0.5,
                                    ),
                                ),
                                ClipRegion::Path(p) => {
                                    if !p.is_valid() {
                                        return Err("invalid GPU clip path".into());
                                    }
                                    // The public path contract is even-odd; using an ordinary
                                    // blend layer also preserves opacity nested within clips.
                                    let area = bounds.last().unwrap().intersect(
                                        path(
                                            &raster::native_path(p)
                                                .ok_or("invalid GPU clip path")?,
                                        )
                                        .bounding_box(),
                                    );
                                    self.layer(Mix::Normal, 1., &area);
                                }
                            }
                            let region_bounds = match region {
                                ClipRegion::Rounded { bounds: b, .. }
                                | ClipRegion::Circle { bounds: b } => rect(*b),
                                ClipRegion::Path(p) => {
                                    path(&raster::native_path(p).ok_or("invalid GPU clip path")?)
                                        .bounding_box()
                                }
                            };
                            let area = bounds.last().unwrap().intersect(region_bounds);
                            bounds.push(if area.is_zero_area() {
                                Rect::ZERO
                            } else {
                                area
                            });
                            masks.push(if let ClipRegion::Path(p) = region {
                                Some(path(
                                    &raster::native_path(p).ok_or("invalid GPU clip path")?,
                                ))
                            } else {
                                None
                            });
                            stack.push(false);
                            clips += 1;
                        }
                        Shape::LayerEnd | Shape::ClipEnd => {
                            let layer = matches!(s, Shape::LayerEnd);
                            if stack.pop() != Some(layer) {
                                return Err("unbalanced GPU clip/opacity stream".into());
                            }
                            bounds.pop();
                            if layer {
                                layers -= 1;
                            } else {
                                clips -= 1;
                                if let Some(mask) = masks.pop().ok_or("unbalanced GPU mask")? {
                                    let area = mask.bounding_box();
                                    self.layer(Compose::DestIn, 1., &area);
                                    self.scene.fill(
                                        Fill::EvenOdd,
                                        Affine::IDENTITY,
                                        Color::WHITE,
                                        None,
                                        &mask,
                                    );
                                    self.pop_layer();
                                }
                            }
                            self.pop_layer();
                        }
                        _ => unreachable!(),
                    }
                    continue;
                }
                self.open_batch_clip(&mut open, clip, *bounds.last().unwrap());
                if let Some(b) = shape_bounds(s) {
                    self.note(b.intersect(rect(clip)));
                }
                self.shape(s, images.as_deref_mut())?;
                self.stats.shapes += 1;
            }
            if !batch.glyphs.is_empty() {
                self.open_batch_clip(&mut open, clip, *bounds.last().unwrap());
            }
            for g in &batch.glyphs {
                self.text(g, painter)?;
            }
        }
        if open.take().is_some_and(|(_, pushed)| pushed) {
            self.pop_layer();
        }
        if !stack.is_empty() {
            return Err("unclosed GPU clip/opacity stream".into());
        }
        self.finish_budget();
        Ok(())
    }
    /// A batch clip needs a layer only if it is tighter than the clip already
    /// in force (a structural clip or the window). Batches inside a scroll view
    /// usually repeat that view's own clip, so they cost nothing.
    fn open_batch_clip(
        &mut self,
        open: &mut Option<([f32; 4], bool)>,
        clip: [f32; 4],
        enclosing: Rect,
    ) {
        if open.is_some_and(|(c, _)| c == clip) {
            return;
        }
        if open.take().is_some_and(|(_, pushed)| pushed) {
            self.pop_layer();
        }
        let r = rect(clip);
        let redundant = enclosing.x0 >= r.x0 - 0.5
            && enclosing.y0 >= r.y0 - 0.5
            && enclosing.x1 <= r.x1 + 0.5
            && enclosing.y1 <= r.y1 + 0.5;
        if !redundant {
            self.layer(Mix::Normal, 1., &r);
        }
        *open = Some((clip, !redundant));
    }
    fn shape(&mut self, s: &Shape, images: Option<ImageResolver<'_, '_>>) -> Result<(), String> {
        let scene = &mut self.scene;
        match s {
            Shape::Rectangle(b, c) => {
                scene.fill(Fill::NonZero, Affine::IDENTITY, color(*c), None, &rect(*b))
            }
            Shape::Rounded(b, r, c) => scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                color(*c),
                None,
                &corners(*b, [*r; 4])?,
            ),
            Shape::Corners(b, r, c) => scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                color(*c),
                None,
                &corners(*b, *r)?,
            ),
            Shape::Outline(b, r, w, c) => {
                if *w > 0. {
                    let p = raster::outline(*b, *r, *w).ok_or("invalid GPU outline")?;
                    scene.fill(Fill::EvenOdd, Affine::IDENTITY, color(*c), None, &path(&p));
                }
            }
            Shape::Triangle(p, c) => {
                let mut b = BezPath::new();
                b.move_to((p[0][0] as f64, p[0][1] as f64));
                for a in &p[1..] {
                    b.line_to((a[0] as f64, a[1] as f64));
                }
                b.close_path();
                scene.fill(Fill::NonZero, Affine::IDENTITY, color(*c), None, &b);
            }
            // Axis-aligned butt-capped lines (borders, dividers) are exact
            // rectangles. Stroked on Vello 0.6, a long horizontal line lying
            // on the surface's top edge filled its whole first tile row.
            Shape::Line(a, b, w, c) if a[0] == b[0] || a[1] == b[1] => {
                let half = w * 0.5;
                let r = if a[1] == b[1] {
                    [a[0].min(b[0]), a[1] - half, (a[0] - b[0]).abs(), *w]
                } else {
                    [a[0] - half, a[1].min(b[1]), *w, (a[1] - b[1]).abs()]
                };
                if r[2] > 0. && r[3] > 0. {
                    scene.fill(Fill::NonZero, Affine::IDENTITY, color(*c), None, &rect(r));
                }
            }
            Shape::Line(a, b, w, c) => scene.stroke(
                &Stroke::new(*w as f64),
                Affine::IDENTITY,
                color(*c),
                None,
                &vello::kurbo::Line::new((a[0] as f64, a[1] as f64), (b[0] as f64, b[1] as f64)),
            ),
            Shape::Path(p, c, w) => {
                if let Some(p) = raster::native_path(p) {
                    let p = path(&p);
                    if let Some(w) = w {
                        scene.stroke(
                            &Stroke::new(*w as f64),
                            Affine::IDENTITY,
                            color(*c),
                            None,
                            &p,
                        );
                    } else {
                        scene.fill(Fill::EvenOdd, Affine::IDENTITY, color(*c), None, &p);
                    }
                }
            }
            Shape::Shadow(b, r, blur, spread, offset, c) => {
                if ![blur, spread, &offset[0], &offset[1]]
                    .into_iter()
                    .all(|v| v.is_finite())
                    || !(0. ..=384.).contains(blur)
                    || spread.abs() > 384.
                    || r.iter().any(|v| !v.is_finite() || *v < 0.)
                {
                    return Err("invalid GPU shadow".into());
                }
                let b = [
                    b[0] + offset[0] - spread,
                    b[1] + offset[1] - spread,
                    b[2] + 2. * spread,
                    b[3] + 2. * spread,
                ];
                if b[2] <= 0. || b[3] <= 0. {
                    return Ok(());
                }
                let r = r.map(|v| (v + spread).max(0.).min(b[2].min(b[3]) * 0.5));
                if *blur == 0. {
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        color(*c),
                        None,
                        &corners(b, r)?,
                    );
                } else if r.iter().all(|v| *v == r[0]) {
                    scene.draw_blurred_rounded_rect(
                        // Vello's analytic blur evaluates integer pixel positions;
                        // our physical geometry uses pixel centres, like tiny-skia.
                        Affine::translate((-0.5, -0.5)),
                        rect(b),
                        color(*c),
                        r[0] as f64,
                        *blur as f64 * 0.5,
                    );
                } else {
                    // Analytic GPU blur per corner quadrant, not a CPU effect bitmap.
                    let outer = rect([
                        b[0] - blur * 3.,
                        b[1] - blur * 3.,
                        b[2] + blur * 6.,
                        b[3] + blur * 6.,
                    ]);
                    let mx = (b[0] + b[2] * 0.5) as f64;
                    let my = (b[1] + b[3] * 0.5) as f64;
                    for (i, q) in [
                        Rect::new(outer.x0, outer.y0, mx, my),
                        Rect::new(mx, outer.y0, outer.x1, my),
                        Rect::new(mx, my, outer.x1, outer.y1),
                        Rect::new(outer.x0, my, mx, outer.y1),
                    ]
                    .iter()
                    .enumerate()
                    {
                        scene.draw_blurred_rounded_rect_in(
                            q,
                            Affine::translate((-0.5, -0.5)),
                            rect(b),
                            color(*c),
                            r[i] as f64,
                            *blur as f64 * 0.5,
                        );
                    }
                }
            }
            Shape::Image(b, image, scale) => self.image(*b, image, *scale, images)?,
            Shape::Gradient(b, r, geometry, stops) => {
                let stops: Vec<(f32, Color)> =
                    stops.iter().map(|(at, c)| (*at, color(*c))).collect();
                let gradient = match *geometry {
                    raster::GradientGeometry::Linear { start, end } => {
                        vello::peniko::Gradient::new_linear(
                            (f64::from(start[0]), f64::from(start[1])),
                            (f64::from(end[0]), f64::from(end[1])),
                        )
                    }
                    raster::GradientGeometry::Radial { centre, radius } => {
                        vello::peniko::Gradient::new_radial(
                            (f64::from(centre[0]), f64::from(centre[1])),
                            radius,
                        )
                    }
                }
                .with_stops(stops.as_slice());
                self.scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &gradient,
                    None,
                    &corners(*b, *r)?,
                );
            }
            _ => unreachable!("structural commands handled by encoder"),
        }
        Ok(())
    }
    fn image(
        &mut self,
        b: [f32; 4],
        image: &BackgroundImage,
        scale: f32,
        images: Option<ImageResolver<'_, '_>>,
    ) -> Result<(), String> {
        if !scale.is_finite()
            || scale <= 0.
            || !image.opacity.is_finite()
            || !(0. ..=1.).contains(&image.opacity)
            || b.iter().any(|v| !v.is_finite())
            || b[2] <= 0.
            || b[3] <= 0.
        {
            return Err("invalid GPU image geometry/opacity".into());
        }
        let id = image.image.pixels().as_ptr() as usize;
        let (w, h) = (image.image.width(), image.image.height());
        if self.frame_images.insert(id) {
            // Vello packs every image used this frame into one atlas (max 8192²).
            self.budget.atlas_area += u64::from(w + 2) * u64::from(h + 2);
            self.budget.oversize_image |= u64::from(w.max(h)) > ATLAS_SIDE;
        }
        let data = match images {
            Some(resolve) => resolve(&image.image)?,
            None => self.cpu_image(image)?,
        };
        self.image_brush(b, image, scale, data);
        Ok(())
    }
    /// CPU-memory image data, cached by pixel buffer and uploaded by Vello each
    /// frame. Used for headless verification without a texture registry.
    fn cpu_image(&mut self, image: &BackgroundImage) -> Result<ImageData, String> {
        let id = image.image.pixels().as_ptr() as usize;
        if !self.images.contains_key(&id) {
            let bytes = image.image.pixels().len();
            while self.image_bytes + bytes > IMAGE_BUDGET {
                let Some(old) = self
                    .images
                    .iter()
                    .min_by_key(|(_, v)| v.used)
                    .map(|(k, _)| *k)
                else {
                    return Err("GPU image cache budget exceeded".into());
                };
                self.image_bytes -= self.images.remove(&old).unwrap().data.data.len();
            }
            let data = image_data(
                image.image.width(),
                image.image.height(),
                image.image.pixels().to_vec(),
            );
            self.stats.resource_bytes += bytes;
            self.image_bytes += bytes;
            self.images.insert(
                id,
                CachedImage {
                    source: image.image.clone(),
                    data,
                    used: self.clock,
                },
            );
        }
        let cached = self.images.get_mut(&id).unwrap();
        cached.used = self.clock;
        debug_assert_eq!(cached.source, image.image);
        Ok(cached.data.clone())
    }
    fn image_brush(&mut self, b: [f32; 4], image: &BackgroundImage, scale: f32, data: ImageData) {
        let (iw, ih) = (image.image.width() as f32, image.image.height() as f32);
        let (sx, sy) = match image.fit {
            ImageFit::Cover => {
                let s = (b[2] / iw).max(b[3] / ih);
                (s, s)
            }
            ImageFit::Contain => {
                let s = (b[2] / iw).min(b[3] / ih);
                (s, s)
            }
            ImageFit::Stretch => (b[2] / iw, b[3] / ih),
            ImageFit::Tile => (scale, scale),
        };
        let pos = image.position.fractions();
        let origin = [
            b[0] + (b[2] - iw * sx) * pos[0],
            b[1] + (b[3] - ih * sy) * pos[1],
        ];
        let brush = ImageBrush {
            image: data,
            sampler: ImageSampler::default(),
        }
        .with_alpha(image.opacity)
        .with_quality(ImageQuality::High);
        let t = Affine::new([
            sx as f64,
            0.,
            0.,
            sy as f64,
            origin[0] as f64,
            origin[1] as f64,
        ]);
        // Fill the visible rectangle with an image brush rather than drawing the
        // image inside a clip layer: one less nesting level per image.
        if image.fit == ImageFit::Tile {
            let brush = brush
                .with_extend(vello::peniko::Extend::Repeat)
                .with_quality(if scale == 1. {
                    ImageQuality::Low
                } else {
                    ImageQuality::Medium
                });
            self.scene
                .fill(Fill::NonZero, Affine::IDENTITY, &brush, Some(t), &rect(b));
        } else {
            let area = rect(b).intersect(rect([origin[0], origin[1], iw * sx, ih * sy]));
            if !area.is_zero_area() {
                self.scene
                    .fill(Fill::NonZero, Affine::IDENTITY, &brush, Some(t), &area);
            }
        }
    }
    fn font(
        &mut self,
        id: cosmic_text::fontdb::ID,
        painter: &mut RasterPainter,
    ) -> Option<FontData> {
        self.fonts
            .entry(id)
            .or_insert_with(|| {
                let (font, index) = painter.gpu_font(id)?;
                Some(FontData::new(Blob::new(Arc::new(FontBytes(font))), index))
            })
            .clone()
    }
    /// Vector text: one `draw_glyphs` run per face/size. The colour is a brush,
    /// so hover/fade colour animation costs nothing extra. A face Vello cannot
    /// be given falls back to cached bitmaps for that text.
    fn text(&mut self, glyph: &Glyph, painter: &mut RasterPainter) -> Result<(), String> {
        let (runs, overflow) = painter.gpu_glyph_runs(glyph);
        if runs.is_empty() {
            return Ok(());
        }
        let mut fonts = Vec::with_capacity(runs.len());
        for run in &runs {
            match self.font(run.font, painter) {
                Some(font) => fonts.push(font),
                None => return self.text_bitmaps(glyph, painter),
            }
        }
        let bounds = rect(glyph.bounds);
        if overflow {
            self.layer(Mix::Normal, 1., &bounds);
        }
        for (run, font) in runs.iter().zip(&fonts) {
            for &(_, x, y) in &run.glyphs {
                let s = f64::from(run.size);
                self.note(Rect::new(
                    x as f64,
                    y as f64 - s,
                    x as f64 + s,
                    y as f64 + s * 0.35,
                ));
                // Flattened outline lines grow with size (0.25px tolerance).
                self.budget.glyph_lines += 48 + (run.size * 1.5) as u64;
            }
            let mut draw = self
                .scene
                .draw_glyphs(font)
                .font_size(run.size)
                // Bitmap/COLR glyphs use brush_alpha independently of the
                // foreground brush. Keep the brush opaque to avoid applying
                // alpha twice to outline glyphs.
                .brush(color([glyph.color[0], glyph.color[1], glyph.color[2], 1.]))
                .brush_alpha(glyph.color[3]);
            if run.fake_italic {
                // cosmic-text synthesises italic with a 14 degree skew.
                draw = draw.glyph_transform(Some(Affine::skew(-(14f64.to_radians().tan()), 0.)));
            }
            draw.draw(
                Fill::NonZero,
                run.glyphs.iter().map(|&(id, x, y)| vello::Glyph {
                    id: u32::from(id),
                    x,
                    y,
                }),
            );
            self.stats.glyphs += run.glyphs.len();
        }
        if overflow {
            self.pop_layer();
        }
        Ok(())
    }
    /// Bitmap glyph fallback (per colour), kept for faces Vello cannot load.
    fn text_bitmaps(&mut self, glyph: &Glyph, painter: &mut RasterPainter) -> Result<(), String> {
        let rgb = [
            byte(glyph.color[0]),
            byte(glyph.color[1]),
            byte(glyph.color[2]),
        ];
        // Pass 1: make sure every glyph bitmap is cached.
        let placed = painter.gpu_glyphs(glyph);
        let mut drawn = Vec::with_capacity(placed.len());
        for (key, origin) in placed {
            let id = (key, rgb);
            if !self.glyphs.contains_key(&id) {
                let Some(image) = painter.gpu_glyph_image(key) else {
                    continue;
                };
                let p = image.placement;
                if p.width == 0 || p.height == 0 {
                    continue;
                }
                let bytes = p.width as usize * p.height as usize * 4;
                if bytes > GLYPH_BUDGET {
                    return Err("GPU glyph resource exceeds budget".into());
                }
                while self.glyph_bytes + bytes > GLYPH_BUDGET || self.glyphs.len() >= 4096 {
                    let Some(old) = self
                        .glyphs
                        .iter()
                        .min_by_key(|(_, v)| v.used)
                        .map(|(k, _)| *k)
                    else {
                        break;
                    };
                    self.glyph_bytes -= self.glyphs.remove(&old).unwrap().data.data.len();
                }
                let mut rgba = Vec::with_capacity(bytes);
                match image.content {
                    SwashContent::Mask => {
                        for a in &image.data {
                            rgba.extend([
                                ((u16::from(rgb[0]) * u16::from(*a) + 127) / 255) as u8,
                                ((u16::from(rgb[1]) * u16::from(*a) + 127) / 255) as u8,
                                ((u16::from(rgb[2]) * u16::from(*a) + 127) / 255) as u8,
                                *a,
                            ]);
                        }
                    }
                    SwashContent::Color => {
                        for p in image.data.chunks_exact(4) {
                            rgba.extend([
                                ((u16::from(p[0]) * u16::from(p[3]) + 127) / 255) as u8,
                                ((u16::from(p[1]) * u16::from(p[3]) + 127) / 255) as u8,
                                ((u16::from(p[2]) * u16::from(p[3]) + 127) / 255) as u8,
                                p[3],
                            ]);
                        }
                    }
                    SwashContent::SubpixelMask => {
                        return Err("unsupported subpixel glyph resource".into())
                    }
                }
                if rgba.len() != bytes {
                    return Err("invalid GPU glyph data".into());
                }
                self.glyph_bytes += bytes;
                self.stats.resource_bytes += bytes;
                self.glyphs.insert(
                    id,
                    CachedGlyph {
                        data: image_data(p.width, p.height, rgba),
                        offset: [p.left as f64, -(p.top as f64)],
                        used: self.clock,
                    },
                );
            }
            drawn.push((id, origin));
        }
        // Clip only text whose ink leaves its box (scrolled or truncated text):
        // a layer per label made ordinary screens exceed Vello's blend budget.
        let bounds = rect(glyph.bounds);
        let ink = drawn.iter().filter_map(|(id, o)| {
            let e = self.glyphs.get(id)?;
            let (x, y) = (o[0] + e.offset[0], o[1] + e.offset[1]);
            Some(Rect::new(
                x,
                y,
                x + f64::from(e.data.width),
                y + f64::from(e.data.height),
            ))
        });
        let clip = ink
            .fold(None::<Rect>, |a, r| Some(a.map_or(r, |a| a.union(r))))
            .is_some_and(|i| {
                i.x0 < bounds.x0 - 0.5
                    || i.y0 < bounds.y0 - 0.5
                    || i.x1 > bounds.x1 + 0.5
                    || i.y1 > bounds.y1 + 0.5
            });
        if clip {
            self.layer(Mix::Normal, 1., &bounds);
        }
        // Pass 2: draw. An entry evicted mid-text is skipped, never unwrapped.
        for (id, origin) in drawn {
            let Some(entry) = self.glyphs.get_mut(&id) else {
                continue;
            };
            entry.used = self.clock;
            let (gw, gh) = (entry.data.width, entry.data.height);
            let brush = ImageBrush {
                image: entry.data.clone(),
                sampler: ImageSampler::default(),
            }
            .with_alpha(glyph.color[3])
            .with_quality(ImageQuality::Low);
            self.budget.atlas_area += u64::from(gw + 2) * u64::from(gh + 2);
            self.scene.draw_image(
                &brush,
                Affine::translate((origin[0] + entry.offset[0], origin[1] + entry.offset[1])),
            );
            self.stats.glyphs += 1;
        }
        if clip {
            self.pop_layer();
        }
        Ok(())
    }
}
