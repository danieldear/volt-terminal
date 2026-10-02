use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{
    Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight,
};

use crate::atlas::CpuAtlas;
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::{CursorStyle, Theme};
use volt_core::cell::CellColor;
use volt_core::grid::Grid;

const ATLAS_SIZE: u32 = 2048;
const INITIAL_BG_VERT_CAPACITY: usize = 16_384;
const INITIAL_GLYPH_VERT_CAPACITY: usize = 32_768;
const TAB_BAR_LEFT_PAD: f32 = 78.0;
const TAB_BAR_GAP: f32 = 4.0;
const TAB_BAR_PLUS_W: f32 = 24.0;
const TAB_BAR_RIGHT_PAD: f32 = 10.0;
const TAB_BAR_MIN_W: f32 = 100.0;
const TAB_BAR_MAX_W: f32 = 220.0;
const TAB_BAR_CLOSE_W: f32 = 16.0;
const TAB_BAR_UI_LINE_HEIGHT: f32 = 1.15;
const TAB_BAR_TITLE_LEFT_PAD: f32 = 18.0;
const TAB_BAR_TITLE_RIGHT_RESERVE: f32 = 44.0;
const TAB_BAR_STATUS_RIGHT_RESERVE: f32 = 22.0;
const TAB_BAR_TAB_TOP_INSET: f32 = 4.0;
const TAB_BAR_TAB_BOTTOM_INSET: f32 = 4.0;
const TAB_BAR_BUTTON_INSET: f32 = 4.0;
const TAB_BAR_BUTTON_PADDING: f32 = 5.0;

/// Positioned glyph from a shaped Buffer run — everything needed to place it
/// in the atlas and emit vertices, WITHOUT the colour (which varies per cell).
#[derive(Clone)]
struct CachedGlyph {
    cache_key: CacheKey,
    /// Shaped physical position, including the font's glyph offsets.
    glyph_x: i32,
    glyph_y: i32,
    /// Baseline y within the cell (physical pixels).
    line_y: f32,
}

/// Match cosmic-text's Buffer::draw placement: the shaped physical offsets
/// must be applied in addition to the bitmap's bearing. Using LayoutGlyph::x
/// alone loses x_offset/y_offset, which varies between fonts and glyphs.
fn glyph_bitmap_origin(
    physical_x: i32,
    physical_y: i32,
    line_y: f32,
    bitmap_left: i32,
    bitmap_top: i32,
) -> (f32, f32) {
    (
        (physical_x + bitmap_left) as f32,
        line_y + (physical_y - bitmap_top) as f32,
    )
}

#[allow(clippy::too_many_arguments)]
fn append_glyph_quad(
    out: &mut Vec<GlyphVertex>,
    sw: f32,
    sh: f32,
    x: f32,
    y: f32,
    region: crate::atlas::AtlasRegion,
    color: [f32; 4],
) {
    let color = if region.is_color {
        [1.0, 1.0, 1.0, color[3]]
    } else {
        color
    };
    let x0 = x / sw * 2.0 - 1.0;
    let x1 = (x + region.width as f32) / sw * 2.0 - 1.0;
    let y0 = 1.0 - y / sh * 2.0;
    let y1 = 1.0 - (y + region.height as f32) / sh * 2.0;
    let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];
    out.extend_from_slice(&[
        GlyphVertex {
            pos: [x0, y0],
            uv: [u0, v0],
            color,
        },
        GlyphVertex {
            pos: [x1, y0],
            uv: [u1, v0],
            color,
        },
        GlyphVertex {
            pos: [x0, y1],
            uv: [u0, v1],
            color,
        },
        GlyphVertex {
            pos: [x1, y0],
            uv: [u1, v0],
            color,
        },
        GlyphVertex {
            pos: [x1, y1],
            uv: [u1, v1],
            color,
        },
        GlyphVertex {
            pos: [x0, y1],
            uv: [u0, v1],
            color,
        },
    ]);
}

fn underline_top(cell_top: f32, baseline: f32, thickness: f32, cell_height: f32) -> f32 {
    // Keep the underline tied to the font baseline rather than the bottom of
    // the cell, which can be far away when line_height is increased.
    cell_top
        + (baseline + thickness)
            .round()
            .clamp(0.0, (cell_height - thickness).max(0.0))
}

fn resolve_family(name: &str) -> Family<'_> {
    let n = name.trim();
    if n.is_empty() || n.eq_ignore_ascii_case("monospace") {
        Family::Monospace
    } else {
        Family::Name(n)
    }
}

fn is_private_use_char(c: char) -> bool {
    matches!(c as u32,
        0xE000..=0xF8FF         // BMP private use area (powerline/nerd icons)
        | 0xF0000..=0xFFFFD     // Plane 15 private use area
        | 0x100000..=0x10FFFD   // Plane 16 private use area
    )
}

fn is_symbol_fallback_char(c: char) -> bool {
    matches!(
        c as u32,
        0x2190..=0x21FF       // arrows
        | 0x2300..=0x23FF       // misc technical (clock/watch-like prompt glyphs)
        | 0x2460..=0x24FF       // enclosed alphanumerics
        | 0x2500..=0x257F       // box drawing
        | 0x2580..=0x259F       // block elements
        | 0x25A0..=0x25FF       // geometric shapes
        | 0x2600..=0x26FF       // miscellaneous symbols
        | 0x2700..=0x27BF       // dingbats (e.g. ❯)
        | 0x27F0..=0x27FF       // supplemental arrows-a
        | 0x2900..=0x297F       // supplemental arrows-b
        | 0x2B00..=0x2BFF       // misc symbols and arrows
    )
}

/// Ordered list of Nerd Font / symbol families to try as a fallback when the
/// primary font lacks private-use or symbol glyphs.  Checked by both
/// `detect_symbol_font_family` and `default_symbol_font_family` so the
/// preference list is maintained in exactly one place.
const PREFERRED_SYMBOL_FONTS: &[&str] = &[
    "JetBrainsMono Nerd Font Mono",
    "JetBrainsMono NL Nerd Font Mono",
    "ZedMono Nerd Font Mono",
    "CaskaydiaMono Nerd Font Mono",
    "FiraMono Nerd Font Mono",
    "Hack Nerd Font Mono",
    "SFMono Nerd Font",
    "FiraCode Nerd Font Mono",
    "Symbols Nerd Font Mono",
    "Symbols Nerd Font",
    "Apple Symbols",
];

fn detect_symbol_font_family(
    font_system: &mut FontSystem,
    _swash_cache: &mut SwashCache,
    primary_family: &str,
    _metrics: Metrics,
) -> Option<String> {
    let primary = primary_family.trim();
    let primary_lower = primary.to_ascii_lowercase();
    if primary_lower.contains("nerd font") || primary_lower.contains("symbols nerd") {
        return None;
    }

    let mut families = std::collections::BTreeSet::new();
    for face in font_system.db().faces() {
        if let Some((name, _)) = face.families.first() {
            families.insert(name.clone());
        }
    }

    for &wanted in PREFERRED_SYMBOL_FONTS {
        if let Some(found) = families
            .iter()
            .find(|name| name.eq_ignore_ascii_case(wanted) && !name.eq_ignore_ascii_case(primary))
        {
            return Some(found.clone());
        }
    }

    default_symbol_font_family(font_system, primary_family)
}

fn default_symbol_font_family(font_system: &FontSystem, primary_family: &str) -> Option<String> {
    let primary = primary_family.trim();
    let mut families = std::collections::BTreeSet::new();
    for face in font_system.db().faces() {
        if let Some((name, _)) = face.families.first() {
            families.insert(name.clone());
        }
    }

    for &wanted in PREFERRED_SYMBOL_FONTS {
        if let Some(found) = families
            .iter()
            .find(|name| name.eq_ignore_ascii_case(wanted) && !name.eq_ignore_ascii_case(primary))
        {
            return Some(found.clone());
        }
    }

    for face in font_system.db().faces() {
        let Some((name, _)) = face.families.first() else {
            continue;
        };
        if name.eq_ignore_ascii_case(primary) {
            continue;
        }
        let lower = name.to_ascii_lowercase();
        if (face.monospaced && lower.contains("nerd font mono")) || lower.contains("symbols nerd") {
            return Some(name.clone());
        }
    }

    None
}

/// One entry for the tab bar
pub struct TabEntry<'a> {
    pub title: &'a str,
    pub active: bool,
    pub index: usize,
    pub busy: bool,
    pub pane_count: usize,
}

/// A pixel-precise divider line between panes.
#[derive(Debug, Clone, Copy)]
pub struct PaneDivider {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub color: [f32; 4],
}

/// The Find / Change Tab Title / Change Terminal Title text-entry overlay.
/// volt-renderer has no knowledge of `volt-ui`'s `TextPrompt` type, so the
/// caller flattens it into this small POD struct each frame.
pub struct PromptOverlay<'a> {
    pub title: &'a str,
    pub text: &'a str,
    /// Read-only destination preview; enable bounded horizontal paging.
    pub read_only: bool,
    /// Char index of the input cursor within `text`.
    pub cursor: usize,
    /// Total live search matches, or 0 outside Find mode.
    pub match_count: usize,
    pub matches_truncated: bool,
    /// 0-based index of the current match within `match_count`.
    pub current_match: usize,
}

/// "Toggle Terminal Inspector" debug overlay contents.
pub struct InspectorInfo {
    pub cols: usize,
    pub rows: usize,
    pub cursor_col: usize,
    pub cursor_row: usize,
    pub scrollback_len: usize,
}

type CellRange = Option<((usize, usize), (usize, usize))>;
#[derive(Clone, Copy, PartialEq, Default)]
struct RowKey {
    cursor: Option<usize>,
    selection: CellRange,
    block: bool,
    search: CellRange,
}
#[derive(Default)]
struct CachedRow {
    valid: bool,
    cells: Vec<volt_core::cell::Cell>,
    extended: Vec<(usize, String)>,
    key: RowKey,
    backgrounds: Vec<BgVertex>,
    glyphs: Vec<GlyphVertex>,
}
impl CachedRow {
    fn matches(&self, grid: &Grid, row: usize, key: RowKey) -> bool {
        self.key == key
            && self.cells == grid.row_cells(row)
            && self
                .extended
                .iter()
                .all(|(col, text)| grid.extended_text(grid.cell(*col, row)) == Some(text.as_str()))
    }
}

pub struct Renderer {
    pub search_palette: Option<crate::search_palette::SearchView>,
    search_palette_cache: Option<search_palette_draw::SearchGeometry>,
    pub workspace_card: Option<crate::workspace_card::WorkspaceCard>,
    workspace_card_cache: Option<workspace_card_draw::CardGeometry>,
    /// Theme editor panel; drawn in the top layer, above the search palette.
    pub theme_editor: Option<crate::theme_editor::ThemeEditorView>,
    theme_editor_cache: Option<theme_editor_draw::EditorGeometry>,
    row_cache: Vec<CachedRow>,
    cache_context: Option<([f32; 8], Theme, CursorStyle)>,
    row_cache_enabled: bool,
    pub last_frame_reused_rows: usize,
    surface: Option<wgpu::Surface<'static>>,
    offscreen: Option<wgpu::Texture>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    bg_pipeline: wgpu::RenderPipeline,
    glyph_pipeline: wgpu::RenderPipeline,
    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    atlas: CpuAtlas,
    font_system: FontSystem,
    swash_cache: SwashCache,
    pub cell_width: f32,
    pub cell_height: f32,
    pub tab_bar_height: f32,
    /// When false, the custom GPU tab bar is suppressed (e.g. native macOS window tabbing active).
    pub custom_tab_bar: bool,
    pub scale_factor: f32,
    font_size_phys: f32,
    font_baseline: f32,
    pub background_opacity: f32,
    pub cursor_style: CursorStyle,
    opaque_alpha_mode: wgpu::CompositeAlphaMode,
    transparent_alpha_mode: wgpu::CompositeAlphaMode,
    pub font_family: String,
    /// Logical padding (from config), physical = padding * scale_factor
    pub padding: f32,
    pub line_height: f32,
    /// Per-character shape cache: avoids re-shaping the same glyph every frame.
    /// Per-character shape cache: avoids re-shaping the same glyph every frame.
    /// Keyed by (char, bold, italic); cleared when font family/size/scale changes.
    shape_cache: HashMap<(char, bool, bool), Vec<CachedGlyph>>,
    extended_shape_cache: HashMap<(String, bool, bool), Vec<CachedGlyph>>,
    symbol_font_family: Option<String>,
    top_alert: Option<String>,
    bg_vertex_buffer: wgpu::Buffer,
    bg_vertex_capacity: usize,
    glyph_vertex_buffer: wgpu::Buffer,
    glyph_vertex_capacity: usize,
    frame_bg_verts: Vec<BgVertex>,
    frame_glyph_verts: Vec<GlyphVertex>,
}

impl Renderer {
    fn glyph_layout_metrics(font_size_phys: f32, line_height: f32) -> Metrics {
        Metrics::new(font_size_phys, font_size_phys * line_height)
    }

    fn snap_to_pixel(value: f32) -> f32 {
        value.round()
    }

    fn create_vertex_buffer<T>(
        device: &wgpu::Device,
        label: &'static str,
        capacity: usize,
    ) -> wgpu::Buffer {
        let elem_size = std::mem::size_of::<T>() as u64;
        let size = (capacity.max(1) as u64) * elem_size;
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn ensure_bg_vertex_buffer_capacity(&mut self, required_vertices: usize) {
        if required_vertices <= self.bg_vertex_capacity {
            return;
        }
        let new_capacity = required_vertices.next_power_of_two();
        self.bg_vertex_buffer =
            Self::create_vertex_buffer::<BgVertex>(&self.device, "bg_verts", new_capacity);
        self.bg_vertex_capacity = new_capacity;
    }

    fn ensure_glyph_vertex_buffer_capacity(&mut self, required_vertices: usize) {
        if required_vertices <= self.glyph_vertex_capacity {
            return;
        }
        let new_capacity = required_vertices.next_power_of_two();
        self.glyph_vertex_buffer =
            Self::create_vertex_buffer::<GlyphVertex>(&self.device, "glyph_verts", new_capacity);
        self.glyph_vertex_capacity = new_capacity;
    }

    fn pick_alpha_modes(
        caps: &wgpu::SurfaceCapabilities,
    ) -> (wgpu::CompositeAlphaMode, wgpu::CompositeAlphaMode) {
        let opaque = if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            caps.alpha_modes[0]
        };
        let transparent = caps
            .alpha_modes
            .iter()
            .copied()
            .find(|mode| *mode != wgpu::CompositeAlphaMode::Opaque)
            .unwrap_or(opaque);
        (opaque, transparent)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        window: Arc<winit::window::Window>,
        font_size: f32,
        scale_factor: f32,
        font_family: &str,
        padding: f32,
        line_height: f32,
        background_opacity: f32,
        cursor_style: CursorStyle,
    ) -> Result<Self> {
        let size = window.inner_size();
        Self::new_target(
            Some(window),
            size,
            font_size,
            scale_factor,
            font_family,
            padding,
            line_height,
            background_opacity,
            cursor_style,
        )
        .await
    }

    /// Real GPU renderer without a window/compositor, for pixel regressions and
    /// submission/completion benchmarks. Not input-to-photon timing.
    #[allow(clippy::too_many_arguments)]
    pub async fn new_offscreen(
        width: u32,
        height: u32,
        font_size: f32,
        scale_factor: f32,
        font_family: &str,
        line_height: f32,
    ) -> Result<Self> {
        Self::new_target(
            None,
            winit::dpi::PhysicalSize::new(width, height),
            font_size,
            scale_factor,
            font_family,
            8.0,
            line_height,
            1.0,
            CursorStyle::Block,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn new_target(
        window: Option<Arc<winit::window::Window>>,
        size: winit::dpi::PhysicalSize<u32>,
        font_size: f32,
        scale_factor: f32,
        font_family: &str,
        padding: f32,
        line_height: f32,
        background_opacity: f32,
        cursor_style: CursorStyle,
    ) -> Result<Self> {
        #[cfg(target_os = "macos")]
        let backends = wgpu::Backends::METAL;
        #[cfg(not(target_os = "macos"))]
        let backends = wgpu::Backends::all();
        #[cfg(debug_assertions)]
        let instance_flags = wgpu::InstanceFlags::default();
        #[cfg(not(debug_assertions))]
        let instance_flags = wgpu::InstanceFlags::empty();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            flags: instance_flags,
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let surface = window
            .map(|window| instance.create_surface(window))
            .transpose()
            .context("failed to create wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to request compatible wgpu adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("failed to request wgpu device")?;

        let caps = surface
            .as_ref()
            .map(|surface| surface.get_capabilities(&adapter))
            .unwrap_or(wgpu::SurfaceCapabilities {
                formats: vec![wgpu::TextureFormat::Rgba8Unorm],
                present_modes: vec![wgpu::PresentMode::Fifo],
                alpha_modes: vec![wgpu::CompositeAlphaMode::Opaque],
                usages: wgpu::TextureUsages::RENDER_ATTACHMENT,
            });
        let surface_format = caps
            .formats
            .iter()
            .find(|f| {
                matches!(
                    **f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .copied()
            .unwrap_or_else(|| {
                caps.formats
                    .iter()
                    .find(|f| !f.is_srgb())
                    .copied()
                    .unwrap_or(caps.formats[0])
            });

        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::Fifo
        };
        let (opaque_alpha_mode, transparent_alpha_mode) = Self::pick_alpha_modes(&caps);
        let alpha_mode = if background_opacity < 0.999 {
            transparent_alpha_mode
        } else {
            opaque_alpha_mode
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        if let Some(surface) = surface.as_ref() {
            surface.configure(&device, &config);
        }
        let offscreen = if surface.is_none() {
            Some(Self::offscreen_texture(&device, &config))
        } else {
            None
        };

        let bg_vertex_buffer =
            Self::create_vertex_buffer::<BgVertex>(&device, "bg_verts", INITIAL_BG_VERT_CAPACITY);
        let glyph_vertex_buffer = Self::create_vertex_buffer::<GlyphVertex>(
            &device,
            "glyph_verts",
            INITIAL_GLYPH_VERT_CAPACITY,
        );

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&Default::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let bg_pl = bg_pipeline(&device, surface_format);
        let glyph_pl = glyph_pipeline(&device, surface_format, &bgl);

        let font_size_phys = font_size * scale_factor;
        let resolved_family = resolve_family(font_family);
        let metrics = Self::glyph_layout_metrics(font_size_phys, line_height);
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let symbol_font_family =
            detect_symbol_font_family(&mut font_system, &mut swash_cache, font_family, metrics);
        let mut measure_buf = Buffer::new(&mut font_system, metrics);
        measure_buf.set_size(&mut font_system, 1000.0, font_size_phys * 2.0);
        measure_buf.set_text(
            &mut font_system,
            "0",
            Attrs::new().family(resolved_family),
            Shaping::Advanced,
        );
        measure_buf.shape_until_scroll(&mut font_system, false);

        let font_baseline = measure_buf
            .layout_runs()
            .next()
            .map(|r| r.line_y)
            .unwrap_or(font_size_phys);
        let cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(font_size_phys * 0.6);
        let cell_height = font_size_phys * line_height;
        let tab_bar_height = (38.0 * scale_factor).round();

        Ok(Self {
            row_cache: Vec::new(),
            cache_context: None,
            row_cache_enabled: true,
            last_frame_reused_rows: 0,
            surface,
            offscreen,
            device,
            queue,
            config,
            bg_pipeline: bg_pl,
            glyph_pipeline: glyph_pl,
            atlas_texture,
            atlas_bind_group,
            atlas: CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE),
            font_system,
            swash_cache,
            cell_width,
            cell_height,
            search_palette: None,
            search_palette_cache: None,
            workspace_card: None,
            workspace_card_cache: None,
            theme_editor: None,
            theme_editor_cache: None,
            tab_bar_height,
            custom_tab_bar: true,
            scale_factor,
            font_size_phys,
            font_baseline,
            background_opacity: background_opacity.clamp(0.0, 1.0),
            cursor_style,
            opaque_alpha_mode,
            transparent_alpha_mode,
            font_family: font_family.to_string(),
            padding,
            line_height,
            shape_cache: HashMap::new(),
            extended_shape_cache: HashMap::new(),
            symbol_font_family,
            top_alert: None,
            bg_vertex_buffer,
            bg_vertex_capacity: INITIAL_BG_VERT_CAPACITY,
            glyph_vertex_buffer,
            glyph_vertex_capacity: INITIAL_GLYPH_VERT_CAPACITY,
            frame_bg_verts: Vec::with_capacity(INITIAL_BG_VERT_CAPACITY),
            frame_glyph_verts: Vec::with_capacity(INITIAL_GLYPH_VERT_CAPACITY),
        })
    }

    fn offscreen_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("volt-rendercheck"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    pub fn wait_for_gpu(&self) -> Result<()> {
        self.device.poll(wgpu::PollType::wait_indefinitely())?;
        Ok(())
    }

    /// Read RGBA pixels from an offscreen frame. Includes a GPU wait; never used
    /// by the interactive render path.
    pub fn read_offscreen_rgba(&self) -> Result<Vec<u8>> {
        let texture = self
            .offscreen
            .as_ref()
            .context("not an offscreen renderer")?;
        let stride = (self.config.width * 4).div_ceil(256) * 256;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("volt-rendercheck-readback"),
            size: stride as u64 * self.config.height as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(self.config.height),
                },
            },
            wgpu::Extent3d {
                width: self.config.width,
                height: self.config.height,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.wait_for_gpu()?;
        rx.recv()??;
        let mapped = buffer.slice(..).get_mapped_range();
        let mut pixels = Vec::with_capacity((self.config.width * self.config.height * 4) as usize);
        for row in mapped.chunks(stride as usize) {
            pixels.extend_from_slice(&row[..(self.config.width * 4) as usize]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(pixels)
    }

    pub fn set_row_cache_enabled(&mut self, enabled: bool) {
        self.row_cache_enabled = enabled;
        self.row_cache.clear();
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if self.offscreen.is_some() {
            self.offscreen = Some(Self::offscreen_texture(&self.device, &self.config));
        }
        if let Some(surface) = self.surface.as_ref() {
            surface.configure(&self.device, &self.config);
        }
    }

    pub fn set_background_opacity(&mut self, opacity: f32) {
        self.background_opacity = opacity.clamp(0.0, 1.0);
        let desired_alpha_mode = if self.background_opacity < 0.999 {
            self.transparent_alpha_mode
        } else {
            self.opaque_alpha_mode
        };
        if self.config.alpha_mode != desired_alpha_mode {
            self.config.alpha_mode = desired_alpha_mode;
            if let Some(surface) = self.surface.as_ref() {
                surface.configure(&self.device, &self.config);
            }
        }
    }

    pub fn set_top_alert(&mut self, alert: Option<String>) {
        self.top_alert = alert.and_then(|msg| {
            let trimmed = msg.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    }

    pub fn update_scale(&mut self, scale_factor: f32, font_size: f32) {
        self.scale_factor = scale_factor;
        self.font_size_phys = font_size * scale_factor;
        let fam_name = self.font_family.clone();
        let fam = resolve_family(&fam_name);
        let metrics = Self::glyph_layout_metrics(self.font_size_phys, self.line_height);
        self.symbol_font_family = detect_symbol_font_family(
            &mut self.font_system,
            &mut self.swash_cache,
            &fam_name,
            metrics,
        );
        let mut measure_buf = Buffer::new(&mut self.font_system, metrics);
        measure_buf.set_size(&mut self.font_system, 1000.0, self.font_size_phys * 2.0);
        measure_buf.set_text(
            &mut self.font_system,
            "0",
            Attrs::new().family(fam),
            Shaping::Advanced,
        );
        measure_buf.shape_until_scroll(&mut self.font_system, false);
        self.font_baseline = measure_buf
            .layout_runs()
            .next()
            .map(|r| r.line_y)
            .unwrap_or(self.font_size_phys);
        self.cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(self.font_size_phys * 0.6);
        self.cell_height = self.font_size_phys * self.line_height;
        self.tab_bar_height = (38.0 * scale_factor).round();
        self.atlas = CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE);
        self.workspace_card_cache = None;
        self.search_palette_cache = None;
        self.theme_editor_cache = None;
        self.shape_cache.clear();
        self.extended_shape_cache.clear();
        self.row_cache.clear();
    }

    fn tab_bar_height_for_tab_count(&self, _tab_count: usize) -> f32 {
        // When custom_tab_bar is disabled (e.g. native macOS window tabbing), return 0
        // so the terminal content fills from the top of the content view.
        if self.custom_tab_bar {
            self.tab_bar_height
        } else {
            0.0
        }
    }

    fn alert_bar_height(&self) -> f32 {
        if self.top_alert.is_some() {
            (28.0 * self.scale_factor).round().max(24.0)
        } else {
            0.0
        }
    }

    fn cell_x_offset_for_char(c: char) -> f32 {
        let _ = c;
        0.0
    }

    fn cell_y_offset_for_char(c: char, cell_h: f32) -> f32 {
        let _ = (c, cell_h);
        0.0
    }

    pub fn top_offset_for_tab_count(&self, tab_count: usize) -> f32 {
        self.tab_bar_height_for_tab_count(tab_count)
    }

    pub fn content_top_offset_for_tab_count(&self, tab_count: usize) -> f32 {
        self.tab_bar_height_for_tab_count(tab_count) + self.alert_bar_height()
    }

    /// Terminal area in columns / rows (excludes tab bar and padding)
    pub fn grid_size(&self) -> (usize, usize) {
        self.grid_size_for_tab_count(2)
    }

    pub fn surface_width(&self) -> u32 {
        self.config.width
    }
    pub fn surface_height(&self) -> u32 {
        self.config.height
    }

    pub fn grid_size_for_tab_count(&self, tab_count: usize) -> (usize, usize) {
        let phys_pad = self.padding * self.scale_factor;
        let term_w =
            self.config.width as f32 - 2.0 * phys_pad - self.workspace_card_reserved_width();
        let term_h = self.config.height as f32
            - self.content_top_offset_for_tab_count(tab_count)
            - 2.0 * phys_pad;
        let cols = (term_w / self.cell_width).floor() as usize;
        let rows = (term_h / self.cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    /// Return all monospace font family names found in the system font database.
    pub fn list_monospace_families(&self) -> Vec<String> {
        let mut seen = std::collections::BTreeSet::new();
        for face in self.font_system.db().faces() {
            if face.monospaced {
                if let Some((name, _)) = face.families.first() {
                    seen.insert(name.clone());
                }
            }
        }
        // Include fonts with common monospace keywords even if flag not set
        for face in self.font_system.db().faces() {
            if let Some((name, _)) = face.families.first() {
                let lower = name.to_lowercase();
                if lower.contains("mono")
                    || lower.contains("code")
                    || lower.contains("courier")
                    || lower.contains("consol")
                    || lower.contains("menlo")
                    || lower.contains("nerd")
                {
                    seen.insert(name.clone());
                }
            }
        }
        seen.into_iter().collect()
    }

    // ── shape cache helpers ──────────────────────────────────────────────────

    /// Shape a single character and return its glyph layout info.
    /// Called at most once per unique (char, bold, italic) triple per font configuration.
    #[allow(clippy::too_many_arguments)]
    fn shape_char(
        font_system: &mut FontSystem,
        c: char,
        metrics: Metrics,
        cell_w: f32,
        cell_h: f32,
        fam_name: &str,
        symbol_fallback_family: Option<&str>,
        swash_cache: &mut SwashCache,
        bold: bool,
        italic: bool,
    ) -> Vec<CachedGlyph> {
        Self::shape_text(
            font_system,
            &c.to_string(),
            metrics,
            cell_w,
            cell_h,
            fam_name,
            symbol_fallback_family,
            swash_cache,
            bold,
            italic,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn shape_text(
        font_system: &mut FontSystem,
        text: &str,
        metrics: Metrics,
        cell_w: f32,
        cell_h: f32,
        fam_name: &str,
        symbol_fallback_family: Option<&str>,
        swash_cache: &mut SwashCache,
        bold: bool,
        italic: bool,
    ) -> Vec<CachedGlyph> {
        let c = text.chars().next().unwrap_or(' ');
        fn shape_for_family(
            font_system: &mut FontSystem,
            text: &str,
            metrics: Metrics,
            cell_w: f32,
            cell_h: f32,
            family: Family<'_>,
            bold: bool,
            italic: bool,
        ) -> Vec<CachedGlyph> {
            let attrs = Attrs::new()
                .family(family)
                .weight(if bold { Weight::BOLD } else { Weight::NORMAL })
                .style(if italic { Style::Italic } else { Style::Normal });
            let mut buf = Buffer::new(font_system, metrics);
            buf.set_size(font_system, cell_w * 2.0, cell_h * 2.0);
            buf.set_text(font_system, text, attrs, Shaping::Advanced);
            buf.shape_until_scroll(font_system, false);
            let mut out = Vec::new();
            for run in buf.layout_runs() {
                for g in run.glyphs.iter() {
                    let physical = g.physical((0.0, 0.0), 1.0);
                    out.push(CachedGlyph {
                        cache_key: physical.cache_key,
                        glyph_x: physical.x,
                        glyph_y: physical.y,
                        line_y: run.line_y,
                    });
                }
            }
            out
        }

        let primary = shape_for_family(
            font_system,
            text,
            metrics,
            cell_w,
            cell_h,
            resolve_family(fam_name),
            bold,
            italic,
        );
        let primary_visible = primary.iter().any(|glyph| {
            swash_cache
                .get_image_uncached(font_system, glyph.cache_key)
                .map(|img| img.placement.width > 0 && img.placement.height > 0)
                .unwrap_or(false)
        });
        if primary_visible && !is_private_use_char(c) {
            return primary;
        }

        if let Some(fallback_family) = symbol_fallback_family {
            if !fallback_family.eq_ignore_ascii_case(fam_name)
                && (is_private_use_char(c) || is_symbol_fallback_char(c))
            {
                let fallback = shape_for_family(
                    font_system,
                    text,
                    metrics,
                    cell_w,
                    cell_h,
                    resolve_family(fallback_family),
                    bold,
                    italic,
                );
                let fallback_visible = fallback.iter().any(|glyph| {
                    swash_cache
                        .get_image_uncached(font_system, glyph.cache_key)
                        .map(|img| img.placement.width > 0 && img.placement.height > 0)
                        .unwrap_or(false)
                });
                if fallback_visible {
                    return fallback;
                }
            } else if !fallback_family.eq_ignore_ascii_case(fam_name) {
                return shape_for_family(
                    font_system,
                    text,
                    metrics,
                    cell_w,
                    cell_h,
                    resolve_family(fallback_family),
                    bold,
                    italic,
                );
            }
        }

        if primary_visible {
            return primary;
        }

        primary
    }

    // ── drawing helpers ──────────────────────────────────────────────────────

    /// Cheap width estimate for overlay layout (cursor position, right-aligned
    /// counters). Not a real shape pass — assumes a roughly monospace advance
    /// of `0.6 * font_size` per character, which is what these UI fonts render
    /// close enough to for positioning text that isn't part of the cell grid.
    fn approx_text_width(text: &str, font_size_phys: f32) -> f32 {
        text.chars().count() as f32 * font_size_phys * 0.6
    }

    fn draw_rect(
        &self,
        bg: &mut Vec<BgVertex>,
        px: f32,
        py: f32,
        pw: f32,
        ph: f32,
        color: [f32; 4],
    ) {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let x0 = (px / sw) * 2.0 - 1.0;
        let x1 = ((px + pw) / sw) * 2.0 - 1.0;
        let y0 = 1.0 - (py / sh) * 2.0;
        let y1 = 1.0 - ((py + ph) / sh) * 2.0;
        bg.extend_from_slice(&[
            BgVertex {
                pos: [x0, y0],
                color,
            },
            BgVertex {
                pos: [x1, y0],
                color,
            },
            BgVertex {
                pos: [x0, y1],
                color,
            },
            BgVertex {
                pos: [x1, y0],
                color,
            },
            BgVertex {
                pos: [x1, y1],
                color,
            },
            BgVertex {
                pos: [x0, y1],
                color,
            },
        ]);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_text_with_line_height(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        text: &str,
        px: f32,
        py: f32,
        font_size_phys: f32,
        line_height: f32,
        color: [f32; 4],
    ) {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let fam_name = self.font_family.clone();
        let fam = resolve_family(&fam_name);
        let metrics = Self::glyph_layout_metrics(font_size_phys, line_height);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, sw, font_size_phys * 2.0);
        buf.set_text(
            &mut self.font_system,
            text,
            Attrs::new().family(fam),
            Shaping::Advanced,
        );
        buf.shape_until_scroll(&mut self.font_system, false);

        for run in buf.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let Some(region) = self.atlas.get_or_rasterize(
                    physical.cache_key,
                    &mut self.font_system,
                    &mut self.swash_cache,
                ) else {
                    continue;
                };
                let color = if region.is_color {
                    [1.0, 1.0, 1.0, color[3]]
                } else {
                    color
                };
                let (local_x, local_y) = glyph_bitmap_origin(
                    physical.x,
                    physical.y,
                    run.line_y,
                    region.offset_x,
                    region.offset_y,
                );
                let gx = px + local_x;
                let gy = py + local_y;
                let gx = Self::snap_to_pixel(gx);
                let gy = Self::snap_to_pixel(gy);
                let gw = region.width as f32;
                let gh = region.height as f32;
                let x0 = (gx / sw) * 2.0 - 1.0;
                let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                let y0 = 1.0 - (gy / sh) * 2.0;
                let y1 = 1.0 - ((gy + gh) / sh) * 2.0;
                let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];
                glyphs.extend_from_slice(&[
                    GlyphVertex {
                        pos: [x0, y0],
                        uv: [u0, v0],
                        color,
                    },
                    GlyphVertex {
                        pos: [x1, y0],
                        uv: [u1, v0],
                        color,
                    },
                    GlyphVertex {
                        pos: [x0, y1],
                        uv: [u0, v1],
                        color,
                    },
                    GlyphVertex {
                        pos: [x1, y0],
                        uv: [u1, v0],
                        color,
                    },
                    GlyphVertex {
                        pos: [x1, y1],
                        uv: [u1, v1],
                        color,
                    },
                    GlyphVertex {
                        pos: [x0, y1],
                        uv: [u0, v1],
                        color,
                    },
                ]);
            }
        }
    }

    #[allow(dead_code)]
    fn draw_text(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        text: &str,
        px: f32,
        py: f32,
        font_size_phys: f32,
        color: [f32; 4],
    ) {
        self.draw_text_with_line_height(
            glyphs,
            text,
            px,
            py,
            font_size_phys,
            self.line_height,
            color,
        );
    }

    // ── GPU present helper ───────────────────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    fn submit_frame(
        &mut self,
        view: wgpu::TextureView,
        output: Option<wgpu::SurfaceTexture>,
        bg_verts: &[BgVertex],
        glyph_verts: &[GlyphVertex],
        clear: [f64; 4],
        partial_redraw: bool,
        hud_start: (usize, usize),
        overlay_start: (usize, usize),
        search_start: (usize, usize),
    ) {
        if let Some((x, y, width, height)) = self.atlas.take_dirty_rect() {
            let start = ((y * self.atlas.width + x) * 4) as usize;
            let end = start + (((height - 1) * self.atlas.width + width) * 4) as usize;
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &self.atlas.data[start..end],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.atlas.width * 4),
                    rows_per_image: Some(self.atlas.height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            self.atlas.dirty = false;
        }

        if !bg_verts.is_empty() {
            self.ensure_bg_vertex_buffer_capacity(bg_verts.len());
            self.queue
                .write_buffer(&self.bg_vertex_buffer, 0, bytemuck::cast_slice(bg_verts));
        }
        if !glyph_verts.is_empty() {
            self.ensure_glyph_vertex_buffer_capacity(glyph_verts.len());
            self.queue.write_buffer(
                &self.glyph_vertex_buffer,
                0,
                bytemuck::cast_slice(glyph_verts),
            );
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if partial_redraw {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color {
                                r: clear[0],
                                g: clear[1],
                                b: clear[2],
                                a: clear[3],
                            })
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // Composite the inspector after terminal glyphs, not just after
            // backgrounds. Floating cards must actually occlude TUI text.
            for (bg_range, glyph_range) in [
                (0..hud_start.0 as u32, 0..hud_start.1 as u32),
                (
                    hud_start.0 as u32..overlay_start.0 as u32,
                    hud_start.1 as u32..overlay_start.1 as u32,
                ),
                (
                    overlay_start.0 as u32..search_start.0 as u32,
                    overlay_start.1 as u32..search_start.1 as u32,
                ),
                (
                    search_start.0 as u32..bg_verts.len() as u32,
                    search_start.1 as u32..glyph_verts.len() as u32,
                ),
            ] {
                if !bg_range.is_empty() {
                    pass.set_pipeline(&self.bg_pipeline);
                    pass.set_vertex_buffer(
                        0,
                        self.bg_vertex_buffer
                            .slice(0..std::mem::size_of_val(bg_verts) as u64),
                    );
                    pass.draw(bg_range, 0..1);
                }
                if !glyph_range.is_empty() {
                    pass.set_pipeline(&self.glyph_pipeline);
                    pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                    pass.set_vertex_buffer(
                        0,
                        self.glyph_vertex_buffer
                            .slice(0..std::mem::size_of_val(glyph_verts) as u64),
                    );
                    pass.draw(glyph_range, 0..1);
                }
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        if let Some(output) = output {
            output.present();
        }
    }

    // ── main terminal render entry point ────────────────────────────────────

    #[allow(clippy::too_many_arguments)]
    pub fn render_frame(
        &mut self,
        grid: &Grid,
        theme: &Theme,
        tabs: &[TabEntry],
        cursor_visible: bool,
        selection: Option<((usize, usize), (usize, usize))>,
        selection_block: bool,
        _damage_rows: Option<(usize, usize)>,
        dividers: &[PaneDivider],
        search_match: Option<((usize, usize), (usize, usize))>,
        prompt: Option<PromptOverlay<'_>>,
        inspector: Option<InspectorInfo>,
        read_only: bool,
    ) {
        if self.atlas.full {
            self.atlas = CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE);
            self.workspace_card_cache = None;
            self.search_palette_cache = None;
            self.theme_editor_cache = None;
            self.row_cache.clear();
        }
        if self.extended_shape_cache.len() > 4096 {
            self.extended_shape_cache.clear();
        }
        if self.shape_cache.len() > 65_536 {
            self.shape_cache.clear();
        }
        let output = if let Some(surface) = self.surface.as_ref() {
            match surface.get_current_texture() {
                wgpu::CurrentSurfaceTexture::Success(o)
                | wgpu::CurrentSurfaceTexture::Suboptimal(o) => Some(o),
                wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                    surface.configure(&self.device, &self.config);
                    return;
                }
                _ => return,
            }
        } else {
            None
        };
        let texture = output
            .as_ref()
            .map(|o| &o.texture)
            .or(self.offscreen.as_ref())
            .expect("render target");
        let view = texture.create_view(&Default::default());

        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let cw = self.cell_width;
        let ch = self.cell_height;
        let tab_top_h = self.tab_bar_height_for_tab_count(tabs.len());
        let alert_h = self.alert_bar_height();
        let content_top = tab_top_h + alert_h;
        let phys_pad = self.padding * self.scale_factor;
        let metrics = Self::glyph_layout_metrics(self.font_size_phys, self.line_height);
        let fam_name = self.font_family.clone();
        // Reuse unchanged CPU row geometry, but always clear/repaint the GPU
        // target. Swapchain images do NOT promise last-frame contents, and
        // full repaint preserves glyph overhang/transparency without ghosts.
        let partial_redraw = false;
        let full_redraw = true;
        let (row_start, row_end_exclusive) = (0, grid.rows);

        let mut bg_verts = std::mem::take(&mut self.frame_bg_verts);
        let mut glyph_verts = std::mem::take(&mut self.frame_glyph_verts);
        bg_verts.clear();
        glyph_verts.clear();

        let cursor_row = grid.cursor_row.min(grid.rows.saturating_sub(1));
        let mut cursor_col = grid.cursor_col.min(grid.cols.saturating_sub(1));
        if cursor_col > 0 && grid.cell(cursor_col, cursor_row).is_continuation() {
            cursor_col -= 1;
        }
        let cursor_width = grid.cell(cursor_col, cursor_row).width().max(1);
        let selection_bg = theme.selection_bg.to_f32();
        let selection_fg = theme.selection_fg.to_f32();
        let cursor_bg = theme.cursor.to_f32();
        let cursor_text = theme.cursor_text.to_f32();
        let cursor_style = self.cursor_style;
        let in_selection = |col: usize, row: usize| -> bool {
            let Some(((start_col, start_row), (end_col, end_row))) = selection else {
                return false;
            };
            if selection_block {
                let min_col = start_col.min(end_col);
                let max_col = start_col.max(end_col);
                let min_row = start_row.min(end_row);
                let max_row = start_row.max(end_row);
                row >= min_row && row <= max_row && col >= min_col && col <= max_col
            } else {
                (row > start_row || (row == start_row && col >= start_col))
                    && (row < end_row || (row == end_row && col <= end_col))
            }
        };
        // Search-match highlight: a fixed amber, independent of the active
        // theme, so it reads clearly against any color scheme. Matches are
        // always single-row (Find operates on flattened row text).
        const MATCH_BG: [f32; 4] = [0.85, 0.62, 0.09, 0.9];
        const MATCH_FG: [f32; 4] = [0.05, 0.03, 0.0, 1.0];
        let in_match = |col: usize, row: usize| -> bool {
            let Some(((start_col, m_row), (end_col, _))) = search_match else {
                return false;
            };
            row == m_row && col >= start_col && col <= end_col
        };

        if partial_redraw && row_start < row_end_exclusive {
            let mut base_bg = theme.background.to_f32();
            base_bg[3] = self.background_opacity;
            let grid_width = grid.cols as f32 * cw;
            for row in row_start..row_end_exclusive {
                let py = content_top + phys_pad + row as f32 * ch;
                self.draw_rect(&mut bg_verts, phys_pad, py, grid_width, ch, base_bg);
            }
        }

        let context = [
            sw,
            sh,
            cw,
            ch,
            phys_pad,
            content_top,
            self.scale_factor,
            self.background_opacity,
        ];
        if self
            .cache_context
            .as_ref()
            .is_none_or(|(old, old_theme, style)| {
                *old != context || old_theme != theme || *style != cursor_style
            })
        {
            self.row_cache.clear();
            self.cache_context = Some((context, theme.clone(), cursor_style));
        }
        self.row_cache.resize_with(grid.rows, CachedRow::default);
        let row_keys: Vec<RowKey> = (0..grid.rows)
            .map(|row| RowKey {
                cursor: if cursor_visible && row == cursor_row {
                    Some(cursor_col)
                } else {
                    None
                },
                selection,
                block: selection_block,
                search: search_match,
            })
            .collect();
        let row_matches: Vec<bool> = (0..grid.rows)
            .map(|row| self.row_cache[row].matches(grid, row, row_keys[row]))
            .collect();
        // Under full-screen churn, caching large vertex arrays adds work with
        // no reuse. Keep only compact source snapshots until output settles.
        let cache_geometry = self.row_cache_enabled
            && row_matches.iter().filter(|&&same| same).count() * 4 >= grid.rows;
        let reuse_rows: Vec<bool> = row_matches
            .iter()
            .enumerate()
            .map(|(row, &same)| self.row_cache_enabled && same && self.row_cache[row].valid)
            .collect();

        self.last_frame_reused_rows = reuse_rows.iter().filter(|&&reuse| reuse).count();

        // ── terminal background quads ────────────────────────────────────────
        for (row, &reuse) in reuse_rows
            .iter()
            .enumerate()
            .take(row_end_exclusive)
            .skip(row_start)
        {
            if reuse {
                bg_verts.extend_from_slice(&self.row_cache[row].backgrounds);
                continue;
            }
            let first_vertex = bg_verts.len();
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let is_cursor = cursor_visible
                    && col >= cursor_col
                    && col < cursor_col + cursor_width
                    && row == cursor_row;
                let is_selected = in_selection(col, row);
                let is_match = in_match(col, row);
                let resolved_bg = if cell.reverse {
                    cell.fg.resolve_fg(theme).to_f32()
                } else {
                    cell.bg.resolve_bg(theme).to_f32()
                };
                let has_custom_bg = !matches!(cell.bg, CellColor::Default) || cell.reverse;
                if !is_cursor && !is_selected && !is_match && !has_custom_bg && !cell.underline {
                    continue;
                }
                let is_block_cursor = is_cursor && cursor_style == CursorStyle::Block;
                let color = if is_block_cursor {
                    cursor_bg
                } else if is_match {
                    MATCH_BG
                } else if is_selected {
                    selection_bg
                } else {
                    resolved_bg
                };
                let px = (phys_pad + col as f32 * cw).round();
                let py = (content_top + phys_pad + row as f32 * ch).round();
                let cw = (phys_pad + (col + 1) as f32 * cw).round() - px;
                let ch = (content_top + phys_pad + (row + 1) as f32 * ch).round() - py;
                // Non-block cursors only add a stroke over the normal cell.
                // Filling a default cell with resolved_bg would replace the
                // transparent clear with alpha=1 and create a dark rectangle.
                if is_block_cursor || is_selected || is_match || has_custom_bg {
                    self.draw_rect(&mut bg_verts, px, py, cw, ch, color);
                }
                if is_cursor {
                    match cursor_style {
                        CursorStyle::Block => {}
                        CursorStyle::Underline => {
                            let h = (1.5 * self.scale_factor).max(2.0).min(ch);
                            self.draw_rect(&mut bg_verts, px, py + ch - h, cw, h, cursor_bg);
                        }
                        CursorStyle::Beam => {
                            let w = (1.2 * self.scale_factor).max(1.5).min(cw);
                            if col == cursor_col {
                                self.draw_rect(&mut bg_verts, px, py, w, ch, cursor_bg);
                            }
                        }
                    }
                }
                if cell.underline {
                    let thickness = self.scale_factor.round().max(1.0).min(ch);
                    let fg = if is_cursor && cursor_style == CursorStyle::Block {
                        cursor_text
                    } else if is_match {
                        MATCH_FG
                    } else if is_selected {
                        selection_fg
                    } else if cell.reverse {
                        cell.bg.resolve_bg(theme).to_f32()
                    } else {
                        cell.fg.resolve_fg(theme).to_f32()
                    };
                    self.draw_rect(
                        &mut bg_verts,
                        px,
                        underline_top(py, self.font_baseline, thickness, ch),
                        cw,
                        thickness,
                        fg,
                    );
                }
            }
            let cached = &mut self.row_cache[row];
            cached.backgrounds.clear();
            if cache_geometry {
                cached
                    .backgrounds
                    .extend_from_slice(&bg_verts[first_vertex..]);
            }
        }

        // ── terminal glyph quads ─────────────────────────────────────────────
        for (row, &reuse) in reuse_rows
            .iter()
            .enumerate()
            .take(row_end_exclusive)
            .skip(row_start)
        {
            if reuse {
                glyph_verts.extend_from_slice(&self.row_cache[row].glyphs);
                continue;
            }
            let first_vertex = glyph_verts.len();
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                // OSC 8 may store a single scalar in the extended-cell table.
                // Keep scalar shaping/procedural symbols identical to unlinked text.
                let extended = grid
                    .extended_text(cell)
                    .filter(|text| text.chars().nth(1).is_some());
                let c = grid.cell_char(cell);
                if cell.is_continuation() || (c == ' ' && extended.is_none()) {
                    continue;
                }
                let cell_top = content_top + phys_pad + row as f32 * ch;
                let is_cursor = cursor_visible
                    && col >= cursor_col
                    && col < cursor_col + cursor_width
                    && row == cursor_row;
                let is_selected = in_selection(col, row);
                let is_match = in_match(col, row);
                let resolved_fg = if cell.reverse {
                    cell.bg.resolve_bg(theme).to_f32()
                } else {
                    cell.fg.resolve_fg(theme).to_f32()
                };
                let color = if is_cursor && cursor_style == CursorStyle::Block {
                    cursor_text
                } else if is_match {
                    MATCH_FG
                } else if is_selected {
                    selection_fg
                } else {
                    resolved_fg
                };

                if extended.is_none() && crate::symbols::is_cell_symbol(c) {
                    let x = (phys_pad + col as f32 * cw).round();
                    let y = cell_top.round();
                    let width = ((phys_pad + (col + 1) as f32 * cw).round() - x).max(1.0) as u32;
                    let height = ((content_top + phys_pad + (row + 1) as f32 * ch).round() - y)
                        .max(1.0) as u32;
                    if let Some(region) = self.atlas.get_cell_symbol(c, width, height) {
                        append_glyph_quad(&mut glyph_verts, sw, sh, x, y, region, color);
                    }
                    continue;
                }
                let cache_key = (c, cell.bold, cell.italic);
                let extended_key = extended.map(|text| (text.to_string(), cell.bold, cell.italic));
                if let Some(key) = extended_key.as_ref() {
                    if !self.extended_shape_cache.contains_key(key) {
                        let glyphs = Self::shape_text(
                            &mut self.font_system,
                            &key.0,
                            metrics,
                            cw,
                            ch,
                            &fam_name,
                            self.symbol_font_family.as_deref(),
                            &mut self.swash_cache,
                            cell.bold,
                            cell.italic,
                        );
                        self.extended_shape_cache.insert(key.clone(), glyphs);
                    }
                } else if !self.shape_cache.contains_key(&cache_key) {
                    let glyphs = Self::shape_char(
                        &mut self.font_system,
                        c,
                        metrics,
                        cw,
                        ch,
                        &fam_name,
                        self.symbol_font_family.as_deref(),
                        &mut self.swash_cache,
                        cell.bold,
                        cell.italic,
                    );
                    self.shape_cache.insert(cache_key, glyphs);
                }

                let glyph_infos = match extended_key
                    .as_ref()
                    .and_then(|key| self.extended_shape_cache.get(key))
                    .or_else(|| self.shape_cache.get(&cache_key))
                {
                    Some(v) => v,
                    None => continue,
                };

                for gi in glyph_infos {
                    let Some(region) = self.atlas.get_or_rasterize(
                        gi.cache_key,
                        &mut self.font_system,
                        &mut self.swash_cache,
                    ) else {
                        continue;
                    };

                    let (local_x, local_y) = glyph_bitmap_origin(
                        gi.glyph_x,
                        gi.glyph_y,
                        gi.line_y,
                        region.offset_x,
                        region.offset_y,
                    );
                    let gx =
                        phys_pad + col as f32 * cw + local_x + Self::cell_x_offset_for_char(c) * cw;
                    let gy = cell_top + local_y + Self::cell_y_offset_for_char(c, ch);
                    let gx = Self::snap_to_pixel(gx);
                    let gy = Self::snap_to_pixel(gy);
                    append_glyph_quad(&mut glyph_verts, sw, sh, gx, gy, region, color);
                }
            }
            let cached = &mut self.row_cache[row];
            cached.glyphs.clear();
            if cache_geometry {
                cached
                    .glyphs
                    .extend_from_slice(&glyph_verts[first_vertex..]);
            }
            cached.valid = cache_geometry;
            cached.cells.clear();
            cached.cells.extend_from_slice(grid.row_cells(row));
            cached.extended.clear();
            if grid.has_extended_text() {
                for (col, cell) in grid.row_cells(row).iter().enumerate() {
                    if let Some(text) = grid.extended_text(cell) {
                        cached.extended.push((col, text.to_string()));
                    }
                }
            }
            cached.key = row_keys[row];
        }

        // ── tab bar ──────────────────────────────────────────────────────────
        if self.custom_tab_bar && tabs.len() > 1 && full_redraw {
            let sc = self.scale_factor;
            let ui_line_height = TAB_BAR_UI_LINE_HEIGHT;
            self.draw_rect(
                &mut bg_verts,
                0.0,
                0.0,
                sw,
                tab_top_h,
                theme.background.to_f32_alpha(0.94),
            );
            self.draw_rect(
                &mut bg_verts,
                0.0,
                0.0,
                sw,
                (1.0 * sc).max(1.0),
                theme.foreground.to_f32_alpha(0.05),
            );
            self.draw_rect(
                &mut bg_verts,
                0.0,
                tab_top_h - (1.0 * sc).max(1.0),
                sw,
                (1.0 * sc).max(1.0),
                theme.foreground.to_f32_alpha(0.16),
            );

            let left_pad = (TAB_BAR_LEFT_PAD * sc).round();
            let tab_gap = (TAB_BAR_GAP * sc).round().max(2.0);
            let plus_w = (TAB_BAR_PLUS_W * sc).round().max(20.0);
            let right_pad = (TAB_BAR_RIGHT_PAD * sc).round();
            let plus_x = (sw - right_pad - plus_w).max(left_pad + plus_w);
            let tab_area_w = (plus_x - left_pad - 10.0 * sc).max(80.0 * sc);
            let n = tabs.len().max(1);
            let tab_w = ((tab_area_w - tab_gap * (n.saturating_sub(1)) as f32) / n as f32)
                .min(TAB_BAR_MAX_W * sc)
                .max(TAB_BAR_MIN_W * sc);
            let tab_h = (tab_top_h - (TAB_BAR_TAB_TOP_INSET + TAB_BAR_TAB_BOTTOM_INSET) * sc)
                .max(24.0 * sc);
            let tab_y = ((tab_top_h - tab_h) * 0.5).round().max(2.0 * sc);
            let tab_font = (11.0 * sc).round().max(1.0);
            let info_font = (9.0 * sc).round().max(1.0);
            let close_font = (11.0 * sc).round().max(1.0);
            let text_top = tab_y + (tab_h - tab_font * ui_line_height) * 0.5;
            let icon_top = tab_y + (tab_h - info_font * ui_line_height) * 0.5;
            let close_top = tab_y + (tab_h - close_font * ui_line_height) * 0.5;

            for (i, tab) in tabs.iter().enumerate() {
                let tx = left_pad + i as f32 * (tab_w + tab_gap);
                let title = tab.title;
                let active = tab.active;
                let tab_edge = theme
                    .foreground
                    .to_f32_alpha(if active { 0.14 } else { 0.07 });
                let tab_bg = if active {
                    theme.background.to_f32_alpha(0.68)
                } else {
                    theme.background.to_f32_alpha(0.30)
                };
                self.draw_rect(&mut bg_verts, tx, tab_y, tab_w, tab_h, tab_bg);
                self.draw_rect(
                    &mut bg_verts,
                    tx,
                    tab_y,
                    tab_w,
                    (1.0 * sc).max(1.0),
                    tab_edge,
                );
                self.draw_rect(
                    &mut bg_verts,
                    tx,
                    tab_y + tab_h - (1.0 * sc).max(1.0),
                    tab_w,
                    (1.0 * sc).max(1.0),
                    theme
                        .background
                        .to_f32_alpha(if active { 0.44 } else { 0.28 }),
                );
                self.draw_rect(
                    &mut bg_verts,
                    tx,
                    tab_y,
                    (1.0 * sc).max(1.0),
                    tab_h,
                    tab_edge,
                );
                self.draw_rect(
                    &mut bg_verts,
                    tx + tab_w - (1.0 * sc).max(1.0),
                    tab_y,
                    (1.0 * sc).max(1.0),
                    tab_h,
                    tab_edge,
                );
                if active {
                    self.draw_rect(
                        &mut bg_verts,
                        tx,
                        tab_y,
                        tab_w,
                        (1.5 * sc).max(1.0),
                        theme.foreground.to_f32_alpha(0.56),
                    );
                }

                let dot = if tab.busy { "\u{25cf}" } else { "\u{25cb}" };
                let dot_color = if tab.busy {
                    [0.55, 0.92, 0.65, 0.95]
                } else {
                    theme.foreground.to_f32_alpha(0.46)
                };
                self.draw_text_with_line_height(
                    &mut glyph_verts,
                    dot,
                    tx + 7.0 * sc,
                    icon_top,
                    info_font,
                    ui_line_height,
                    dot_color,
                );

                let close_w = TAB_BAR_CLOSE_W * sc;
                let close_box_w = close_w + (TAB_BAR_BUTTON_PADDING + 1.0) * sc;
                let close_box_x = tx + tab_w - close_box_w - (TAB_BAR_BUTTON_INSET + 1.0) * sc;
                let close_box_y = tab_y + (1.0 * sc).max(1.0);
                let close_box_h = (tab_h - 2.0 * sc).max(18.0 * sc);
                let info_text = if tab.pane_count > 1 {
                    format!("#{} · {}", tab.index, tab.pane_count)
                } else {
                    format!("#{}", tab.index)
                };
                let info_x = close_box_x - TAB_BAR_STATUS_RIGHT_RESERVE * sc;
                let title_left = tx + TAB_BAR_TITLE_LEFT_PAD * sc;
                let title_right =
                    (info_x - TAB_BAR_TITLE_RIGHT_RESERVE * sc).max(title_left + 12.0 * sc);
                let max_text_w = (title_right - title_left).max(12.0 * sc);
                let title_color = if active {
                    theme.foreground.to_f32()
                } else {
                    theme.foreground.to_f32_alpha(0.66)
                };
                {
                    let metrics = Metrics::new(tab_font, tab_font * ui_line_height);
                    let mut buf = Buffer::new(&mut self.font_system, metrics);
                    buf.set_size(&mut self.font_system, max_text_w, tab_font * 2.0);
                    buf.set_text(
                        &mut self.font_system,
                        title,
                        Attrs::new().family(Family::Monospace),
                        Shaping::Advanced,
                    );
                    buf.shape_until_scroll(&mut self.font_system, false);
                    let bx = title_left;
                    let by = text_top;
                    for run in buf.layout_runs() {
                        for glyph in run.glyphs.iter() {
                            let physical = glyph.physical((0.0, 0.0), 1.0);
                            let Some(region) = self.atlas.get_or_rasterize(
                                physical.cache_key,
                                &mut self.font_system,
                                &mut self.swash_cache,
                            ) else {
                                continue;
                            };
                            let title_color = if region.is_color {
                                [1.0, 1.0, 1.0, title_color[3]]
                            } else {
                                title_color
                            };
                            let (rel_x, rel_y) = glyph_bitmap_origin(
                                physical.x,
                                physical.y,
                                run.line_y,
                                region.offset_x,
                                region.offset_y,
                            );
                            if rel_x + region.width as f32 > max_text_w {
                                break;
                            }
                            let gx = bx + rel_x;
                            let gy = by + rel_y;
                            let gw = region.width as f32;
                            let gh = region.height as f32;
                            let x0 = (gx / sw) * 2.0 - 1.0;
                            let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                            let y0 = 1.0 - (gy / sh) * 2.0;
                            let y1 = 1.0 - ((gy + gh) / sh) * 2.0;
                            let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];
                            glyph_verts.extend_from_slice(&[
                                GlyphVertex {
                                    pos: [x0, y0],
                                    uv: [u0, v0],
                                    color: title_color,
                                },
                                GlyphVertex {
                                    pos: [x1, y0],
                                    uv: [u1, v0],
                                    color: title_color,
                                },
                                GlyphVertex {
                                    pos: [x0, y1],
                                    uv: [u0, v1],
                                    color: title_color,
                                },
                                GlyphVertex {
                                    pos: [x1, y0],
                                    uv: [u1, v0],
                                    color: title_color,
                                },
                                GlyphVertex {
                                    pos: [x1, y1],
                                    uv: [u1, v1],
                                    color: title_color,
                                },
                                GlyphVertex {
                                    pos: [x0, y1],
                                    uv: [u0, v1],
                                    color: title_color,
                                },
                            ]);
                        }
                    }
                }

                self.draw_text_with_line_height(
                    &mut glyph_verts,
                    &info_text,
                    info_x,
                    icon_top,
                    info_font,
                    ui_line_height,
                    theme
                        .foreground
                        .to_f32_alpha(if active { 0.76 } else { 0.46 }),
                );

                self.draw_rect(
                    &mut bg_verts,
                    close_box_x,
                    close_box_y,
                    close_box_w,
                    close_box_h,
                    theme
                        .background
                        .to_f32_alpha(if active { 0.44 } else { 0.24 }),
                );
                let cx = close_box_x + (TAB_BAR_BUTTON_PADDING * 0.75) * sc;
                let close_color = if active {
                    theme.foreground.to_f32_alpha(0.82)
                } else {
                    theme.foreground.to_f32_alpha(0.36)
                };
                self.draw_text_with_line_height(
                    &mut glyph_verts,
                    "\u{00d7}",
                    cx,
                    close_top,
                    close_font,
                    ui_line_height,
                    close_color,
                );
            }

            let plus_box_x = plus_x - (TAB_BAR_BUTTON_INSET / 2.0) * sc;
            let plus_box_y = tab_y + (1.0 * sc).max(1.0);
            let plus_box_h = (tab_h - 2.0 * sc).max(18.0 * sc);
            self.draw_rect(
                &mut bg_verts,
                plus_box_x,
                plus_box_y,
                plus_w + TAB_BAR_BUTTON_INSET * sc,
                plus_box_h,
                theme.background.to_f32_alpha(0.44),
            );
            self.draw_text_with_line_height(
                &mut glyph_verts,
                "+",
                plus_x + plus_w * 0.28,
                icon_top,
                tab_font,
                ui_line_height,
                theme.foreground.to_f32_alpha(0.72),
            );
        }

        if full_redraw {
            if let Some(alert) = self.top_alert.clone() {
                let sc = self.scale_factor;
                let bar_h = alert_h;
                let bar_y = tab_top_h;
                let icon_x = 12.0 * sc;
                let text_x = 30.0 * sc;
                let text_font = (12.0 * sc).round().max(1.0);
                let text_y = bar_y + (bar_h - (text_font * TAB_BAR_UI_LINE_HEIGHT)) * 0.5;
                self.draw_rect(
                    &mut bg_verts,
                    0.0,
                    bar_y,
                    sw,
                    bar_h,
                    [0.72, 0.20, 0.24, 0.92],
                );
                self.draw_text_with_line_height(
                    &mut glyph_verts,
                    "!",
                    icon_x,
                    text_y,
                    text_font,
                    TAB_BAR_UI_LINE_HEIGHT,
                    [1.0, 1.0, 1.0, 1.0],
                );
                self.draw_text_with_line_height(
                    &mut glyph_verts,
                    &alert,
                    text_x,
                    text_y,
                    text_font,
                    TAB_BAR_UI_LINE_HEIGHT,
                    [1.0, 1.0, 1.0, 1.0],
                );
                self.draw_rect(
                    &mut bg_verts,
                    0.0,
                    bar_y + bar_h - (1.0 * sc).max(1.0),
                    sw,
                    (1.0 * sc).max(1.0),
                    [0.98, 0.93, 0.93, 0.35],
                );
            }
        }

        let bg_color = theme.background.to_f32();
        for d in dividers {
            self.draw_rect(&mut bg_verts, d.x, d.y, d.width, d.height, d.color);
        }

        let hud_start = (bg_verts.len(), glyph_verts.len());
        // ── overlays: read-only badge, inspector, Find/rename prompt ─────────
        // Drawn last so they sit on top of the terminal content and dividers.
        if read_only {
            let font_sz = 11.0 * self.scale_factor;
            let pad = 5.0 * self.scale_factor;
            let label = "READ-ONLY";
            let box_w = Self::approx_text_width(label, font_sz) + pad * 2.0;
            let box_h = font_sz * 1.7;
            let box_x = phys_pad;
            let box_y = content_top + phys_pad;
            self.draw_rect(
                &mut bg_verts,
                box_x,
                box_y,
                box_w,
                box_h,
                [0.6, 0.15, 0.15, 0.85],
            );
            self.draw_text(
                &mut glyph_verts,
                label,
                box_x + pad,
                box_y + pad * 0.4,
                font_sz,
                [1.0, 0.9, 0.9, 1.0],
            );
        }

        if let Some(info) = inspector.as_ref() {
            let text = format!(
                "grid {}x{}  cursor {},{}  scrollback {}",
                info.cols, info.rows, info.cursor_col, info.cursor_row, info.scrollback_len
            );
            let font_sz = 11.0 * self.scale_factor;
            let pad = 6.0 * self.scale_factor;
            let box_w = Self::approx_text_width(&text, font_sz) + pad * 2.0;
            let box_h = font_sz * 1.8;
            let margin = 8.0 * self.scale_factor;
            let box_x = sw - box_w - margin;
            let box_y = sh - box_h - margin;
            self.draw_rect(
                &mut bg_verts,
                box_x,
                box_y,
                box_w,
                box_h,
                [0.05, 0.05, 0.06, 0.85],
            );
            self.draw_text(
                &mut glyph_verts,
                &text,
                box_x + pad,
                box_y + pad * 0.5,
                font_sz,
                [0.85, 0.85, 0.5, 1.0],
            );
        }

        if let Some(p) = prompt.as_ref() {
            let box_w = (420.0 * self.scale_factor).min((sw - 40.0).max(200.0));
            let box_h = 72.0 * self.scale_factor;
            let box_x = (sw - box_w) / 2.0;
            let box_y = (40.0 * self.scale_factor).max(20.0);
            let border = (1.5 * self.scale_factor).max(1.0);
            self.draw_rect(
                &mut bg_verts,
                box_x - border,
                box_y - border,
                box_w + border * 2.0,
                box_h + border * 2.0,
                [0.42, 0.42, 0.47, 1.0],
            );
            self.draw_rect(
                &mut bg_verts,
                box_x,
                box_y,
                box_w,
                box_h,
                [0.12, 0.12, 0.14, 1.0],
            );

            let pad = 14.0 * self.scale_factor;
            let title_size = 12.0 * self.scale_factor;
            let input_size = 15.0 * self.scale_factor;
            self.draw_text(
                &mut glyph_verts,
                p.title,
                box_x + pad,
                box_y + pad * 0.5,
                title_size,
                [0.7, 0.7, 0.75, 1.0],
            );

            if p.title == "Find" {
                let counter = if p.match_count == 0 {
                    "0/0".to_string()
                } else {
                    format!(
                        "{}/{}{}",
                        p.current_match + 1,
                        p.match_count,
                        if p.matches_truncated { "+" } else { "" }
                    )
                };
                let counter_w = Self::approx_text_width(&counter, title_size);
                self.draw_text(
                    &mut glyph_verts,
                    &counter,
                    box_x + box_w - pad - counter_w,
                    box_y + pad * 0.5,
                    title_size,
                    [0.7, 0.7, 0.75, 1.0],
                );
            }

            let input_y = box_y + pad * 0.5 + title_size * 1.5;
            let preview = p.read_only.then(|| {
                crate::prompt_viewport::window(
                    p.text,
                    p.cursor,
                    ((box_w - 2.0 * pad) / (input_size * 0.65)).floor().max(4.0) as usize,
                )
            });
            let (display_text, display_cursor) = preview
                .as_ref()
                .map(|(text, cursor)| (text.as_str(), *cursor))
                .unwrap_or((p.text, p.cursor));
            let display_text = if display_text.is_empty() {
                " "
            } else {
                display_text
            };
            self.draw_text(
                &mut glyph_verts,
                display_text,
                box_x + pad,
                input_y,
                input_size,
                [0.95, 0.95, 0.97, 1.0],
            );

            let chars_before: String = display_text.chars().take(display_cursor).collect();
            let cursor_advance = if p.read_only {
                unicode_width::UnicodeWidthStr::width(chars_before.as_str()) as f32
                    * input_size
                    * 0.6
            } else {
                Self::approx_text_width(&chars_before, input_size)
            };
            let cursor_x = box_x + pad + cursor_advance;
            let cursor_w = (1.5 * self.scale_factor).max(1.0);
            self.draw_rect(
                &mut bg_verts,
                cursor_x,
                input_y,
                cursor_w,
                input_size * 1.15,
                [0.95, 0.95, 0.97, 0.9],
            );
        }

        let overlay_start = (bg_verts.len(), glyph_verts.len());
        if let Some(card) = self.workspace_card.clone() {
            self.draw_workspace_card(
                &mut bg_verts,
                &mut glyph_verts,
                &card,
                theme,
                tab_top_h + alert_h,
            );
        }

        let search_start = (bg_verts.len(), glyph_verts.len());
        if let Some(search) = self.search_palette.clone() {
            self.draw_search_palette(
                &mut bg_verts,
                &mut glyph_verts,
                &search,
                theme,
                tab_top_h + alert_h,
            );
        }
        if let Some(editor) = self.theme_editor.clone() {
            self.draw_theme_editor(&mut bg_verts, &mut glyph_verts, &editor);
        }
        self.submit_frame(
            view,
            output,
            &bg_verts,
            &glyph_verts,
            [
                bg_color[0] as f64,
                bg_color[1] as f64,
                bg_color[2] as f64,
                self.background_opacity as f64,
            ],
            partial_redraw,
            hud_start,
            overlay_start,
            search_start,
        );
        self.frame_bg_verts = bg_verts;
        self.frame_glyph_verts = glyph_verts;
    }
}

#[path = "workspace_card_draw.rs"]
mod workspace_card_draw;

#[cfg(test)]
mod placement_tests {
    use super::*;

    #[test]
    fn physical_glyph_offsets_and_bitmap_bearings_are_preserved() {
        // A shaped glyph may carry offsets in both axes (notably fallback and
        // combining glyphs). These are the coordinates cosmic-text draws at.
        assert_eq!(glyph_bitmap_origin(7, -3, 15.5, -2, 11), (5.0, 1.5));
        assert_eq!(glyph_bitmap_origin(0, 0, 18.0, 1, 14), (1.0, 4.0));
    }

    #[test]
    fn underline_tracks_baseline_when_line_height_changes() {
        let thickness = 2.0;
        let top_short = underline_top(100.0, 15.0, thickness, 20.0);
        let top_tall = underline_top(100.0, 23.0, thickness, 36.0);
        assert_eq!(top_short, 117.0);
        assert_eq!(top_tall, 125.0);
        assert!(top_short + thickness <= 120.0);
        assert!(top_tall + thickness <= 136.0);
    }

    /// CPU glyph shaping only; not a GPU-frame or input-to-photon benchmark.
    /// Run with `cargo test -p volt-renderer --release benchmark_terminal_glyph_shaping -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn benchmark_terminal_glyph_shaping() {
        let mut font_system = FontSystem::new();
        let mut swash_cache = SwashCache::new();
        let glyphs: Vec<char> = "0123456789abcdefNORMAL→│█".chars().collect();
        let font_size = 14.0;
        let iterations = 100;
        let start = std::time::Instant::now();
        let mut shaped = 0usize;
        for line_height in [1.0, 1.2, 1.8] {
            let metrics = Renderer::glyph_layout_metrics(font_size, line_height);
            for _ in 0..iterations {
                for &c in &glyphs {
                    shaped += Renderer::shape_char(
                        &mut font_system,
                        c,
                        metrics,
                        9.0,
                        font_size * line_height,
                        "monospace",
                        None,
                        &mut swash_cache,
                        false,
                        false,
                    )
                    .len();
                }
            }
        }
        std::hint::black_box(shaped);
        eprintln!(
            "terminal glyph CPU shaping: {} chars, {} glyphs in {:?} ({:.0} chars/s)",
            glyphs.len() * iterations * 3,
            shaped,
            start.elapsed(),
            (glyphs.len() * iterations * 3) as f64 / start.elapsed().as_secs_f64()
        );
    }
}

#[path = "search_palette_draw.rs"]
mod search_palette_draw;

#[path = "theme_editor_draw.rs"]
mod theme_editor_draw;
