use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, CacheKey, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight};

use crate::atlas::CpuAtlas;
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::{CursorStyle, Theme};
use volt_core::cell::CellColor;
use volt_core::grid::Grid;

const ATLAS_SIZE: u32 = 2048;
const INITIAL_BG_VERT_CAPACITY: usize = 16_384;
const INITIAL_GLYPH_VERT_CAPACITY: usize = 32_768;
const SYMBOL_BASELINE_LINE_HEIGHT: f32 = 1.4;
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
#[cfg(target_os = "macos")]
const MACOS_SINGLE_TAB_TITLEBAR_INSET: f32 = 26.0;

/// Positioned glyph from a shaped Buffer run — everything needed to place it
/// in the atlas and emit vertices, WITHOUT the colour (which varies per cell).
#[derive(Clone)]
struct CachedGlyph {
    cache_key: CacheKey,
    /// Horizontal advance from the Buffer layout run (physical pixels).
    glyph_x: f32,
    /// Baseline y within the cell (physical pixels).
    line_y: f32,
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

pub struct Renderer {
    surface: wgpu::Surface<'static>,
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
    pub scale_factor: f32,
    font_size_phys: f32,
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
        let surface = instance
            .create_surface(window.clone())
            .context("failed to create wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .context("failed to request compatible wgpu adapter")?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .context("failed to request wgpu device")?;

        let caps = surface.get_capabilities(&adapter);
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
        surface.configure(&device, &config);

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
            format: wgpu::TextureFormat::R8Unorm,
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

        let cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(font_size_phys * 0.6);
        let cell_height = font_size_phys * line_height;
        let tab_bar_height = (38.0 * scale_factor).round();

        Ok(Self {
            surface,
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
            tab_bar_height,
            scale_factor,
            font_size_phys,
            background_opacity: background_opacity.clamp(0.0, 1.0),
            cursor_style,
            opaque_alpha_mode,
            transparent_alpha_mode,
            font_family: font_family.to_string(),
            padding,
            line_height,
            shape_cache: HashMap::new(),
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

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
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
            self.surface.configure(&self.device, &self.config);
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
        self.cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(self.font_size_phys * 0.6);
        self.cell_height = self.font_size_phys * self.line_height;
        self.tab_bar_height = (38.0 * scale_factor).round();
        self.atlas = CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE);
        self.shape_cache.clear();
    }

    fn tab_bar_height_for_tab_count(&self, tab_count: usize) -> f32 {
        if tab_count > 1 {
            return self.tab_bar_height;
        }
        #[cfg(target_os = "macos")]
        {
            // With transparent/fullsize titlebar, macOS places traffic lights over content.
            // Keep a minimal top inset in single-tab mode so the first row clears titlebar chrome.
            return (MACOS_SINGLE_TAB_TITLEBAR_INSET * self.scale_factor)
                .round()
                .max(20.0);
        }
        #[cfg(not(target_os = "macos"))]
        {
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

    pub fn grid_size_for_tab_count(&self, tab_count: usize) -> (usize, usize) {
        let phys_pad = self.padding * self.scale_factor;
        let term_w = self.config.width as f32 - 2.0 * phys_pad;
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
        fn shape_for_family(
            font_system: &mut FontSystem,
            c: char,
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
            buf.set_text(font_system, &c.to_string(), attrs, Shaping::Advanced);
            buf.shape_until_scroll(font_system, false);
            let mut out = Vec::new();
            for run in buf.layout_runs() {
                for g in run.glyphs.iter() {
                    let physical = g.physical((0.0, 0.0), 1.0);
                    out.push(CachedGlyph {
                        cache_key: physical.cache_key,
                        glyph_x: g.x,
                        line_y: run.line_y,
                    });
                }
            }
            out
        }

        if let Some(fallback_family) = symbol_fallback_family {
            if !fallback_family.eq_ignore_ascii_case(fam_name)
                && (is_private_use_char(c) || is_symbol_fallback_char(c))
            {
                let fallback = shape_for_family(
                    font_system,
                    c,
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
            }
        }

        let primary = shape_for_family(
            font_system,
            c,
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
        if primary_visible {
            return primary;
        }

        if let Some(fallback_family) = symbol_fallback_family {
            if !fallback_family.eq_ignore_ascii_case(fam_name) {
                return shape_for_family(
                    font_system,
                    c,
                    metrics,
                    cell_w,
                    cell_h,
                    resolve_family(fallback_family),
                    bold,
                    italic,
                );
            }
        }

        primary
    }

    // ── drawing helpers ──────────────────────────────────────────────────────

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
                let gx = px + glyph.x + region.offset_x as f32;
                let gy = py + run.line_y - region.offset_y as f32;
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
        self.draw_text_with_line_height(glyphs, text, px, py, font_size_phys, self.line_height, color);
    }

    // ── GPU present helper ───────────────────────────────────────────────────

    fn submit_frame(
        &mut self,
        view: wgpu::TextureView,
        output: wgpu::SurfaceTexture,
        bg_verts: &[BgVertex],
        glyph_verts: &[GlyphVertex],
        clear: [f64; 4],
        partial_redraw: bool,
    ) {
        if self.atlas.dirty {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.atlas.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.atlas.width),
                    rows_per_image: Some(self.atlas.height),
                },
                wgpu::Extent3d {
                    width: self.atlas.width,
                    height: self.atlas.height,
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
            if !bg_verts.is_empty() {
                pass.set_pipeline(&self.bg_pipeline);
                pass.set_vertex_buffer(
                    0,
                    self.bg_vertex_buffer
                        .slice(0..std::mem::size_of_val(bg_verts) as u64),
                );
                pass.draw(0..bg_verts.len() as u32, 0..1);
            }
            if !glyph_verts.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(
                    0,
                    self.glyph_vertex_buffer
                        .slice(0..std::mem::size_of_val(glyph_verts) as u64),
                );
                pass.draw(0..glyph_verts.len() as u32, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    // ── main terminal render entry point ────────────────────────────────────

    pub fn render_frame(
        &mut self,
        grid: &Grid,
        theme: &Theme,
        tabs: &[TabEntry],
        cursor_visible: bool,
        selection: Option<((usize, usize), (usize, usize))>,
        _damage_rows: Option<(usize, usize)>,
    ) {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(o)
            | wgpu::CurrentSurfaceTexture::Suboptimal(o) => o,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = output.texture.create_view(&Default::default());

        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let cw = self.cell_width;
        let ch = self.cell_height;
        let tab_top_h = self.tab_bar_height_for_tab_count(tabs.len());
        let alert_h = self.alert_bar_height();
        let content_top = tab_top_h + alert_h;
        let phys_pad = self.padding * self.scale_factor;
        let metrics = Self::glyph_layout_metrics(self.font_size_phys, self.line_height);
        let symbol_metrics =
            Self::glyph_layout_metrics(self.font_size_phys, SYMBOL_BASELINE_LINE_HEIGHT);
        let symbol_line_height = self.font_size_phys * SYMBOL_BASELINE_LINE_HEIGHT;
        let fam_name = self.font_family.clone();
        // Row-scoped partial redraw is currently unsafe with dynamic line-height / glyph overhang.
        // Force full redraw for correctness.
        let partial_redraw = false;
        let full_redraw = true;
        let (row_start, row_end_exclusive) = (0, grid.rows);

        let mut bg_verts = std::mem::take(&mut self.frame_bg_verts);
        let mut glyph_verts = std::mem::take(&mut self.frame_glyph_verts);
        bg_verts.clear();
        glyph_verts.clear();

        let cursor_col = grid.cursor_col;
        let cursor_row = grid.cursor_row;
        let selection_bg = theme.selection_bg.to_f32();
        let selection_fg = theme.selection_fg.to_f32();
        let cursor_bg = theme.cursor.to_f32();
        let cursor_text = theme.cursor_text.to_f32();
        let cursor_style = self.cursor_style;
        let in_selection = |col: usize, row: usize| -> bool {
            let Some(((start_col, start_row), (end_col, end_row))) = selection else {
                return false;
            };
            (row > start_row || (row == start_row && col >= start_col))
                && (row < end_row || (row == end_row && col <= end_col))
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

        // ── terminal background quads ────────────────────────────────────────
        for row in row_start..row_end_exclusive {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let is_cursor = cursor_visible && col == cursor_col && row == cursor_row;
                let is_selected = in_selection(col, row);
                let resolved_bg = if cell.reverse {
                    cell.fg.resolve_fg(theme).to_f32()
                } else {
                    cell.bg.resolve_bg(theme).to_f32()
                };
                let has_custom_bg = !matches!(cell.bg, CellColor::Default) || cell.reverse;
                if !is_cursor && !is_selected && !has_custom_bg {
                    continue;
                }
                let color = if is_cursor {
                    match cursor_style {
                        CursorStyle::Block => cursor_bg,
                        CursorStyle::Underline => resolved_bg,
                        CursorStyle::Beam => resolved_bg,
                    }
                } else if is_selected {
                    selection_bg
                } else {
                    resolved_bg
                };
                let px = phys_pad + col as f32 * cw;
                let py = content_top + phys_pad + row as f32 * ch;
                if is_cursor {
                    match cursor_style {
                        CursorStyle::Block => self.draw_rect(&mut bg_verts, px, py, cw, ch, color),
                        CursorStyle::Underline => {
                            let h = (1.5 * self.scale_factor).max(2.0).min(ch);
                            self.draw_rect(&mut bg_verts, px, py + ch - h, cw, h, cursor_bg);
                        }
                        CursorStyle::Beam => {
                            let w = (1.2 * self.scale_factor).max(1.5).min(cw);
                            self.draw_rect(&mut bg_verts, px, py, w, ch, cursor_bg);
                        }
                    }
                } else {
                    self.draw_rect(&mut bg_verts, px, py, cw, ch, color);
                }
            }
        }

        // ── terminal glyph quads ─────────────────────────────────────────────
        for row in row_start..row_end_exclusive {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                if cell.c == ' ' {
                    continue;
                }
                let cache_key = (cell.c, cell.bold, cell.italic);
                if !self.shape_cache.contains_key(&cache_key) {
                    let char_metrics =
                        if is_private_use_char(cell.c) || is_symbol_fallback_char(cell.c) {
                            symbol_metrics
                        } else {
                            metrics
                        };
                    let glyphs = Self::shape_char(
                        &mut self.font_system,
                        cell.c,
                        char_metrics,
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

                let cell_top = content_top + phys_pad + row as f32 * ch;
                let is_cursor = cursor_visible && col == cursor_col && row == cursor_row;
                let is_selected = in_selection(col, row);
                let resolved_fg = if cell.reverse {
                    cell.bg.resolve_bg(theme).to_f32()
                } else {
                    cell.fg.resolve_fg(theme).to_f32()
                };
                let color = if is_cursor && cursor_style == CursorStyle::Block {
                    cursor_text
                } else if is_selected {
                    selection_fg
                } else {
                    resolved_fg
                };

                let glyph_infos = match self.shape_cache.get(&cache_key) {
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

                    let gx = phys_pad
                        + col as f32 * cw
                        + gi.glyph_x
                        + region.offset_x as f32
                        + Self::cell_x_offset_for_char(cell.c) * cw;
                    let symbol_vertical_offset =
                        if is_private_use_char(cell.c) || is_symbol_fallback_char(cell.c) {
                            (ch - symbol_line_height) * 0.5
                        } else {
                            0.0
                        };
                    let gy = cell_top + gi.line_y + symbol_vertical_offset - region.offset_y as f32
                        + Self::cell_y_offset_for_char(cell.c, ch);
                    let gx = Self::snap_to_pixel(gx);
                    let gy = Self::snap_to_pixel(gy);
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

        // ── tab bar ──────────────────────────────────────────────────────────
        if tabs.len() > 1 && full_redraw {
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
                let tab_edge = theme.foreground.to_f32_alpha(if active { 0.14 } else { 0.07 });
                let tab_bg = if active {
                    theme.background.to_f32_alpha(0.68)
                } else {
                    theme.background.to_f32_alpha(0.30)
                };
                self.draw_rect(&mut bg_verts, tx, tab_y, tab_w, tab_h, tab_bg);
                self.draw_rect(&mut bg_verts, tx, tab_y, tab_w, (1.0 * sc).max(1.0), tab_edge);
                self.draw_rect(
                    &mut bg_verts,
                    tx,
                    tab_y + tab_h - (1.0 * sc).max(1.0),
                    tab_w,
                    (1.0 * sc).max(1.0),
                    theme.background.to_f32_alpha(if active { 0.44 } else { 0.28 }),
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
                let title_right = (info_x - TAB_BAR_TITLE_RIGHT_RESERVE * sc).max(title_left + 12.0 * sc);
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
                            let rel_x = glyph.x + region.offset_x as f32;
                            if rel_x + region.width as f32 > max_text_w {
                                break;
                            }
                            let gx = bx + glyph.x + region.offset_x as f32;
                            let gy = by + run.line_y - region.offset_y as f32;
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
                    theme.background.to_f32_alpha(if active { 0.44 } else { 0.24 }),
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
        );
        self.frame_bg_verts = bg_verts;
        self.frame_glyph_verts = glyph_verts;
    }
}
