use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::util::DeviceExt;

use crate::atlas::CpuAtlas;
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::Theme;
use volt_core::grid::Grid;

const ATLAS_SIZE: u32 = 2048;

/// One entry for the tab bar
pub struct TabEntry<'a> {
    pub title: &'a str,
    pub active: bool,
    pub index: usize, // 1-based
}

/// Data for the settings overlay
pub struct SettingsOverlay<'a> {
    pub font_size: f32,
    pub font_family: &'a str,
    pub theme_names: &'a [&'static str],
    pub theme_idx: usize,
    pub focused_field: u8, // 0=theme 1=font_family 2=font_size
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
    font_size_phys: f32, // font_size * scale_factor
}

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>, font_size: f32, scale_factor: f32) -> Self {
        let size = window.inner_size();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            flags: wgpu::InstanceFlags::default(),
            memory_budget_thresholds: Default::default(),
            backend_options: Default::default(),
            display: None,
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

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
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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

        let mut font_system = FontSystem::new();

        // Measure cell dimensions at physical pixel size
        let font_size_phys = font_size * scale_factor;
        let metrics = Metrics::new(font_size_phys, font_size_phys * 1.4);
        let mut measure_buf = Buffer::new(&mut font_system, metrics);
        measure_buf.set_size(&mut font_system, 1000.0, font_size_phys * 2.0);
        measure_buf.set_text(
            &mut font_system,
            "0",
            Attrs::new().family(Family::Monospace),
            Shaping::Basic,
        );
        measure_buf.shape_until_scroll(&mut font_system, false);

        let cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(font_size_phys * 0.6);
        let cell_height = font_size_phys * 1.4;
        let tab_bar_height = (38.0 * scale_factor).round();

        Self {
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
            swash_cache: SwashCache::new(),
            cell_width,
            cell_height,
            tab_bar_height,
            scale_factor,
            font_size_phys,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn update_scale(&mut self, scale_factor: f32, font_size: f32) {
        self.scale_factor = scale_factor;
        self.font_size_phys = font_size * scale_factor;
        let metrics = Metrics::new(self.font_size_phys, self.font_size_phys * 1.4);
        let mut measure_buf = Buffer::new(&mut self.font_system, metrics);
        measure_buf.set_size(&mut self.font_system, 1000.0, self.font_size_phys * 2.0);
        measure_buf.set_text(
            &mut self.font_system,
            "0",
            Attrs::new().family(Family::Monospace),
            Shaping::Basic,
        );
        measure_buf.shape_until_scroll(&mut self.font_system, false);
        self.cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(self.font_size_phys * 0.6);
        self.cell_height = self.font_size_phys * 1.4;
        self.tab_bar_height = (38.0 * scale_factor).round();
        // Invalidate glyph atlas since font size changed
        self.atlas = CpuAtlas::new(ATLAS_SIZE, ATLAS_SIZE);
    }

    /// Terminal area size in columns and rows (excludes tab bar)
    pub fn grid_size(&self) -> (usize, usize) {
        let term_h = self.config.height as f32 - self.tab_bar_height;
        let cols = (self.config.width as f32 / self.cell_width).floor() as usize;
        let rows = (term_h / self.cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    // ── drawing helpers ──────────────────────────────────────────────────────

    /// Push a filled rect (physical pixels) into bg_verts.
    fn draw_rect(
        &self,
        bg: &mut Vec<BgVertex>,
        px: f32, py: f32, pw: f32, ph: f32,
        color: [f32; 4],
    ) {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let x0 = (px / sw) * 2.0 - 1.0;
        let x1 = ((px + pw) / sw) * 2.0 - 1.0;
        let y0 = 1.0 - (py / sh) * 2.0;
        let y1 = 1.0 - ((py + ph) / sh) * 2.0;
        bg.extend_from_slice(&[
            BgVertex { pos: [x0, y0], color },
            BgVertex { pos: [x1, y0], color },
            BgVertex { pos: [x0, y1], color },
            BgVertex { pos: [x1, y0], color },
            BgVertex { pos: [x1, y1], color },
            BgVertex { pos: [x0, y1], color },
        ]);
    }

    /// Render a text string starting at physical pixel (px, py) = top-left of text.
    fn draw_text(
        &mut self,
        glyphs: &mut Vec<GlyphVertex>,
        text: &str,
        px: f32, py: f32,
        font_size_phys: f32,
        color: [f32; 4],
    ) {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let metrics = Metrics::new(font_size_phys, font_size_phys * 1.4);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, sw, font_size_phys * 2.0);
        buf.set_text(&mut self.font_system, text, Attrs::new().family(Family::Monospace), Shaping::Basic);
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
                // line_y is baseline from buffer top; offset_y = placement.top (pixels above baseline)
                let gy = py + run.line_y - region.offset_y as f32;
                let gw = region.width as f32;
                let gh = region.height as f32;

                let x0 = (gx / sw) * 2.0 - 1.0;
                let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                let y0 = 1.0 - (gy / sh) * 2.0;
                let y1 = 1.0 - ((gy + gh) / sh) * 2.0;
                let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];
                glyphs.extend_from_slice(&[
                    GlyphVertex { pos: [x0, y0], uv: [u0, v0], color },
                    GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                    GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                    GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                    GlyphVertex { pos: [x1, y1], uv: [u1, v1], color },
                    GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                ]);
            }
        }
    }

    // ── settings overlay ─────────────────────────────────────────────────────

    fn render_settings(
        &mut self,
        bg: &mut Vec<BgVertex>,
        glyphs: &mut Vec<GlyphVertex>,
        s: &SettingsOverlay,
    ) {
        let sw = self.config.width as f32;
        let sh = self.config.height as f32;
        let sc = self.scale_factor;

        // Dim overlay
        self.draw_rect(bg, 0.0, 0.0, sw, sh, [0.0, 0.0, 0.0, 0.65]);

        // Panel dimensions
        let pw = (480.0 * sc).min(sw - 40.0 * sc);
        let ph = (360.0 * sc).min(sh - 40.0 * sc);
        let px = ((sw - pw) / 2.0).round();
        let py = ((sh - ph) / 2.0).round();

        // Panel background
        self.draw_rect(bg, px, py, pw, ph, [0.12, 0.12, 0.16, 1.0]);
        // Panel border
        self.draw_rect(bg, px, py, pw, 1.0, [0.30, 0.30, 0.40, 1.0]);
        self.draw_rect(bg, px, py + ph - 1.0, pw, 1.0, [0.30, 0.30, 0.40, 1.0]);
        self.draw_rect(bg, px, py, 1.0, ph, [0.30, 0.30, 0.40, 1.0]);
        self.draw_rect(bg, px + pw - 1.0, py, 1.0, ph, [0.30, 0.30, 0.40, 1.0]);

        let hdr_font = (16.0 * sc).round();
        let lbl_font = (13.0 * sc).round();
        let val_font = (13.0 * sc).round();
        let pad = 20.0 * sc;
        let row_h = 34.0 * sc;
        let hdr_color = [0.85_f32, 0.85, 0.90, 1.0];
        let lbl_color = [0.60_f32, 0.60, 0.65, 1.0];
        let val_color = [0.90_f32, 0.90, 0.95, 1.0];
        let focus_color = [0.38_f32, 0.57, 0.96, 1.0];

        // Title
        self.draw_text(glyphs, "Settings", px + pad, py + pad, hdr_font, hdr_color);

        // ── APPEARANCE section ───────────────────────────────────────────────
        let sec_y = py + pad + hdr_font * 1.6 + 8.0 * sc;
        self.draw_text(glyphs, "APPEARANCE", px + pad, sec_y, (10.0 * sc).round(), [0.45, 0.45, 0.55, 1.0]);

        // Theme row
        let row1_y = sec_y + (10.0 * sc) * 1.4 + 6.0 * sc;
        self.draw_text(glyphs, "Theme", px + pad, row1_y, lbl_font, lbl_color);

        let field_x = px + pad + 90.0 * sc;
        let field_w = pw - pad - 90.0 * sc - pad;
        let field_h = row_h * 0.8;
        let f0_focused = s.focused_field == 0;
        let f0_border = if f0_focused { focus_color } else { [0.25, 0.25, 0.32, 1.0] };
        self.draw_rect(bg, field_x, row1_y - 2.0 * sc, field_w, field_h, [0.17, 0.17, 0.22, 1.0]);
        self.draw_rect(bg, field_x, row1_y - 2.0 * sc, field_w, 1.0, f0_border);
        self.draw_rect(bg, field_x, row1_y - 2.0 * sc + field_h - 1.0, field_w, 1.0, f0_border);
        let theme_name = s.theme_names.get(s.theme_idx).copied().unwrap_or("dark");
        self.draw_text(glyphs, theme_name, field_x + 6.0 * sc, row1_y, val_font, val_color);

        // ── FONT section ─────────────────────────────────────────────────────
        let sec2_y = row1_y + row_h + 16.0 * sc;
        self.draw_text(glyphs, "FONT", px + pad, sec2_y, (10.0 * sc).round(), [0.45, 0.45, 0.55, 1.0]);

        // Font family
        let row2_y = sec2_y + (10.0 * sc) * 1.4 + 6.0 * sc;
        self.draw_text(glyphs, "Family", px + pad, row2_y, lbl_font, lbl_color);
        let f1_focused = s.focused_field == 1;
        let f1_border = if f1_focused { focus_color } else { [0.25, 0.25, 0.32, 1.0] };
        self.draw_rect(bg, field_x, row2_y - 2.0 * sc, field_w, field_h, [0.17, 0.17, 0.22, 1.0]);
        self.draw_rect(bg, field_x, row2_y - 2.0 * sc, field_w, 1.0, f1_border);
        self.draw_rect(bg, field_x, row2_y - 2.0 * sc + field_h - 1.0, field_w, 1.0, f1_border);
        let family_display = if f1_focused {
            format!("{}|", s.font_family)
        } else {
            s.font_family.to_string()
        };
        self.draw_text(glyphs, &family_display, field_x + 6.0 * sc, row2_y, val_font, val_color);

        // Font size
        let row3_y = row2_y + row_h;
        self.draw_text(glyphs, "Size", px + pad, row3_y, lbl_font, lbl_color);
        let f2_focused = s.focused_field == 2;
        let f2_border = if f2_focused { focus_color } else { [0.25, 0.25, 0.32, 1.0] };
        let size_field_w = 80.0 * sc;
        self.draw_rect(bg, field_x, row3_y - 2.0 * sc, size_field_w, field_h, [0.17, 0.17, 0.22, 1.0]);
        self.draw_rect(bg, field_x, row3_y - 2.0 * sc, size_field_w, 1.0, f2_border);
        self.draw_rect(bg, field_x, row3_y - 2.0 * sc + field_h - 1.0, size_field_w, 1.0, f2_border);
        self.draw_text(glyphs, &format!("{}", s.font_size as u32), field_x + 6.0 * sc, row3_y, val_font, val_color);
        // +/- buttons
        let btn_x = field_x + size_field_w + 8.0 * sc;
        self.draw_rect(bg, btn_x, row3_y - 2.0 * sc, 26.0 * sc, field_h, [0.20, 0.20, 0.27, 1.0]);
        self.draw_text(glyphs, "-", btn_x + 8.0 * sc, row3_y, val_font, val_color);
        self.draw_rect(bg, btn_x + 30.0 * sc, row3_y - 2.0 * sc, 26.0 * sc, field_h, [0.20, 0.20, 0.27, 1.0]);
        self.draw_text(glyphs, "+", btn_x + 30.0 * sc + 8.0 * sc, row3_y, val_font, val_color);

        // Hint at bottom
        let hint_y = py + ph - pad - lbl_font * 1.4;
        self.draw_text(glyphs, "Tab/Shift+Tab: cycle fields   Up/Down: change value   Esc: close", px + pad, hint_y, (10.0 * sc).round(), [0.40, 0.40, 0.50, 1.0]);
    }

    // ── main render entry point ───────────────────────────────────────────────

    pub fn render_frame(
        &mut self,
        grid: &Grid,
        theme: &Theme,
        tabs: &[TabEntry],
        settings: Option<&SettingsOverlay>,
    ) {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(o) | wgpu::CurrentSurfaceTexture::Suboptimal(o) => o,
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
        let tby = self.tab_bar_height;
        let metrics = Metrics::new(self.font_size_phys, ch);

        let mut bg_verts: Vec<BgVertex> = Vec::new();
        let mut glyph_verts: Vec<GlyphVertex> = Vec::new();

        // ── terminal background quads ────────────────────────────────────────
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let color = cell.bg.resolve_bg(theme).to_f32();
                let px = col as f32 * cw;
                let py = tby + row as f32 * ch;
                self.draw_rect(&mut bg_verts, px, py, cw, ch, color);
            }
        }

        // ── terminal glyph quads ─────────────────────────────────────────────
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                if cell.c == ' ' {
                    continue;
                }

                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, cw + 10.0, ch + 10.0);
                buf.set_text(
                    &mut self.font_system,
                    &cell.c.to_string(),
                    Attrs::new().family(Family::Monospace),
                    Shaping::Basic,
                );
                buf.shape_until_scroll(&mut self.font_system, false);

                let cell_top = tby + row as f32 * ch;
                let color = cell.fg.resolve_fg(theme).to_f32();

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

                        let gx = col as f32 * cw + glyph.x + region.offset_x as f32;
                        // Correct: baseline from cell top + line_y, minus placement.top
                        let gy = cell_top + run.line_y - region.offset_y as f32;
                        let gw = region.width as f32;
                        let gh = region.height as f32;

                        let x0 = (gx / sw) * 2.0 - 1.0;
                        let x1 = ((gx + gw) / sw) * 2.0 - 1.0;
                        let y0 = 1.0 - (gy / sh) * 2.0;
                        let y1 = 1.0 - ((gy + gh) / sh) * 2.0;
                        let [u0, v0, u1, v1] = [region.u0, region.v0, region.u1, region.v1];

                        glyph_verts.extend_from_slice(&[
                            GlyphVertex { pos: [x0, y0], uv: [u0, v0], color },
                            GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                            GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                            GlyphVertex { pos: [x1, y0], uv: [u1, v0], color },
                            GlyphVertex { pos: [x1, y1], uv: [u1, v1], color },
                            GlyphVertex { pos: [x0, y1], uv: [u0, v1], color },
                        ]);
                    }
                }
            }
        }

        // ── tab bar ──────────────────────────────────────────────────────────
        // Collect tab data to avoid borrow issues
        let tab_data: Vec<(String, bool, usize)> = tabs
            .iter()
            .map(|t| (t.title.to_string(), t.active, t.index))
            .collect();

        // Draw tab bar background
        self.draw_rect(&mut bg_verts, 0.0, 0.0, sw, tby, [0.11, 0.11, 0.14, 1.0]);

        let sc = self.scale_factor;
        let left_pad = (78.0 * sc).round();
        let tab_w = if !tab_data.is_empty() {
            ((sw - left_pad - 40.0 * sc) / tab_data.len() as f32).min(240.0 * sc)
        } else {
            0.0
        };
        let tab_font = (12.0 * sc).round();
        let pad_v = (tby - tab_font * 1.4) / 2.0;

        for (i, (title, active, _idx)) in tab_data.iter().enumerate() {
            let tx = left_pad + i as f32 * tab_w;
            if *active {
                self.draw_rect(&mut bg_verts, tx, 0.0, tab_w, tby, [0.20, 0.20, 0.25, 1.0]);
                self.draw_rect(&mut bg_verts, tx, tby - 2.0, tab_w, 2.0, [0.38, 0.57, 0.96, 1.0]);
            }
            if i > 0 {
                self.draw_rect(&mut bg_verts, tx, 6.0 * sc, 1.0, tby - 12.0 * sc, [0.25, 0.25, 0.30, 1.0]);
            }
            let color = if *active {
                [0.90_f32, 0.90, 0.95, 1.0]
            } else {
                [0.65_f32, 0.65, 0.70, 1.0]
            };
            self.draw_text(&mut glyph_verts, title, tx + 8.0 * sc, pad_v, tab_font, color);
        }
        if !tab_data.is_empty() {
            let plus_x = left_pad + tab_data.len() as f32 * tab_w + 8.0 * sc;
            self.draw_text(&mut glyph_verts, "+", plus_x, pad_v, tab_font, [0.45, 0.45, 0.50, 1.0]);
        }

        // ── settings overlay ─────────────────────────────────────────────────
        if let Some(s) = settings {
            // Collect data to avoid multiple borrows
            let font_size = s.font_size;
            let font_family = s.font_family.to_string();
            let theme_names: Vec<&'static str> = s.theme_names.to_vec();
            let theme_idx = s.theme_idx;
            let focused_field = s.focused_field;
            let overlay = SettingsOverlay {
                font_size,
                font_family: &font_family,
                theme_names: &theme_names,
                theme_idx,
                focused_field,
            };
            self.render_settings(&mut bg_verts, &mut glyph_verts, &overlay);
        }

        // ── upload atlas if dirty ─────────────────────────────────────────────
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

        // ── GPU buffers ───────────────────────────────────────────────────────
        let bg_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("bg_verts"),
            contents: bytemuck::cast_slice(&bg_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let glyph_buf = if glyph_verts.is_empty() {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glyph_empty"),
                contents: bytemuck::cast_slice(&[GlyphVertex { pos: [0.0; 2], uv: [0.0; 2], color: [0.0; 4] }]),
                usage: wgpu::BufferUsages::VERTEX,
            })
        } else {
            self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("glyph_verts"),
                contents: bytemuck::cast_slice(&glyph_verts),
                usage: wgpu::BufferUsages::VERTEX,
            })
        };

        let bg_color = theme.background.to_f32();
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: bg_color[0] as f64,
                            g: bg_color[1] as f64,
                            b: bg_color[2] as f64,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.bg_pipeline);
            pass.set_vertex_buffer(0, bg_buf.slice(..));
            pass.draw(0..bg_verts.len() as u32, 0..1);

            if !glyph_verts.is_empty() {
                pass.set_pipeline(&self.glyph_pipeline);
                pass.set_bind_group(0, &self.atlas_bind_group, &[]);
                pass.set_vertex_buffer(0, glyph_buf.slice(..));
                pass.draw(0..glyph_verts.len() as u32, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}
