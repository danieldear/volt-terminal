use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::util::DeviceExt;

use crate::atlas::CpuAtlas;
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::Theme;
use volt_core::grid::Grid;

const ATLAS_SIZE: u32 = 2048;

fn resolve_family(name: &str) -> Family<'_> {
    let n = name.trim();
    if n.is_empty() || n.eq_ignore_ascii_case("monospace") {
        Family::Monospace
    } else {
        Family::Name(n)
    }
}

/// One entry for the tab bar
pub struct TabEntry<'a> {
    pub title: &'a str,
    pub active: bool,
    pub index: usize,
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
    pub font_family: String,
    /// Logical padding (from config), physical = padding * scale_factor
    pub padding: f32,
    pub line_height: f32,
}

impl Renderer {
    pub async fn new(
        window: Arc<winit::window::Window>,
        font_size: f32,
        scale_factor: f32,
        font_family: &str,
        padding: f32,
        line_height: f32,
    ) -> Self {
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
            .find(|f| {
                matches!(
                    **f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .copied()
            .unwrap_or_else(|| {
                caps.formats.iter().find(|f| !f.is_srgb()).copied().unwrap_or(caps.formats[0])
            });

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
            size: wgpu::Extent3d { width: ATLAS_SIZE, height: ATLAS_SIZE, depth_or_array_layers: 1 },
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
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas_sampler) },
            ],
        });

        let bg_pl = bg_pipeline(&device, surface_format);
        let glyph_pl = glyph_pipeline(&device, surface_format, &bgl);

        let mut font_system = FontSystem::new();

        let font_size_phys = font_size * scale_factor;
        let resolved_family = resolve_family(font_family);
        let metrics = Metrics::new(font_size_phys, font_size_phys * line_height);
        let mut measure_buf = Buffer::new(&mut font_system, metrics);
        measure_buf.set_size(&mut font_system, 1000.0, font_size_phys * 2.0);
        measure_buf.set_text(&mut font_system, "0", Attrs::new().family(resolved_family), Shaping::Basic);
        measure_buf.shape_until_scroll(&mut font_system, false);

        let cell_width = measure_buf
            .layout_runs()
            .next()
            .and_then(|r| r.glyphs.first())
            .map(|g| g.w)
            .unwrap_or(font_size_phys * 0.6);
        let cell_height = font_size_phys * line_height;
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
            font_family: font_family.to_string(),
            padding,
            line_height,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 { return; }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn update_scale(&mut self, scale_factor: f32, font_size: f32) {
        self.scale_factor = scale_factor;
        self.font_size_phys = font_size * scale_factor;
        let fam_name = self.font_family.clone();
        let fam = resolve_family(&fam_name);
        let metrics = Metrics::new(self.font_size_phys, self.font_size_phys * self.line_height);
        let mut measure_buf = Buffer::new(&mut self.font_system, metrics);
        measure_buf.set_size(&mut self.font_system, 1000.0, self.font_size_phys * 2.0);
        measure_buf.set_text(&mut self.font_system, "0", Attrs::new().family(fam), Shaping::Basic);
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
    }

    /// Terminal area in columns / rows (excludes tab bar and padding)
    pub fn grid_size(&self) -> (usize, usize) {
        let phys_pad = self.padding * self.scale_factor;
        let term_w = self.config.width as f32 - 2.0 * phys_pad;
        let term_h = self.config.height as f32 - self.tab_bar_height - 2.0 * phys_pad;
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
                if lower.contains("mono") || lower.contains("code") || lower.contains("courier")
                    || lower.contains("consol") || lower.contains("menlo") || lower.contains("nerd")
                {
                    seen.insert(name.clone());
                }
            }
        }
        seen.into_iter().collect()
    }

    // ── drawing helpers ──────────────────────────────────────────────────────

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
        let fam_name = self.font_family.clone();
        let fam = resolve_family(&fam_name);
        let metrics = Metrics::new(font_size_phys, font_size_phys * 1.4);
        let mut buf = Buffer::new(&mut self.font_system, metrics);
        buf.set_size(&mut self.font_system, sw, font_size_phys * 2.0);
        buf.set_text(&mut self.font_system, text, Attrs::new().family(fam), Shaping::Basic);
        buf.shape_until_scroll(&mut self.font_system, false);

        for run in buf.layout_runs() {
            for glyph in run.glyphs.iter() {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let Some(region) = self.atlas.get_or_rasterize(
                    physical.cache_key,
                    &mut self.font_system,
                    &mut self.swash_cache,
                ) else { continue };
                let gx = px + glyph.x + region.offset_x as f32;
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

    // ── GPU present helper ───────────────────────────────────────────────────

    fn submit_frame(
        &mut self,
        view: wgpu::TextureView,
        output: wgpu::SurfaceTexture,
        bg_verts: Vec<BgVertex>,
        glyph_verts: Vec<GlyphVertex>,
        clear: [f64; 4],
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
                wgpu::Extent3d { width: self.atlas.width, height: self.atlas.height, depth_or_array_layers: 1 },
            );
            self.atlas.dirty = false;
        }

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

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: clear[0], g: clear[1], b: clear[2], a: clear[3] }),
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

    // ── main terminal render entry point ────────────────────────────────────

    pub fn render_frame(
        &mut self,
        grid: &Grid,
        theme: &Theme,
        tabs: &[TabEntry],
        cursor_visible: bool,
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
        let phys_pad = self.padding * self.scale_factor;
        let metrics = Metrics::new(self.font_size_phys, ch);
        let fam_name = self.font_family.clone();

        let mut bg_verts: Vec<BgVertex> = Vec::new();
        let mut glyph_verts: Vec<GlyphVertex> = Vec::new();

        let cursor_col = grid.cursor_col;
        let cursor_row = grid.cursor_row;

        // ── terminal background quads ────────────────────────────────────────
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let is_cursor = cursor_visible && col == cursor_col && row == cursor_row;
                // At cursor: invert fg/bg to show a block cursor
                let color = if is_cursor {
                    cell.fg.resolve_fg(theme).to_f32()
                } else {
                    cell.bg.resolve_bg(theme).to_f32()
                };
                let px = phys_pad + col as f32 * cw;
                let py = tby + phys_pad + row as f32 * ch;
                self.draw_rect(&mut bg_verts, px, py, cw, ch, color);
            }
        }

        // ── terminal glyph quads ─────────────────────────────────────────────
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                if cell.c == ' ' { continue; }

                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, cw + 10.0, ch + 10.0);
                buf.set_text(
                    &mut self.font_system,
                    &cell.c.to_string(),
                    Attrs::new().family(resolve_family(&fam_name)),
                    Shaping::Advanced,
                );
                buf.shape_until_scroll(&mut self.font_system, false);

                let cell_top = tby + phys_pad + row as f32 * ch;
                let is_cursor = cursor_visible && col == cursor_col && row == cursor_row;
                // At cursor: draw glyph in background color (inverted)
                let color = if is_cursor {
                    cell.bg.resolve_bg(theme).to_f32()
                } else {
                    cell.fg.resolve_fg(theme).to_f32()
                };

                for run in buf.layout_runs() {
                    for glyph in run.glyphs.iter() {
                        let physical = glyph.physical((0.0, 0.0), 1.0);
                        let Some(region) = self.atlas.get_or_rasterize(
                            physical.cache_key,
                            &mut self.font_system,
                            &mut self.swash_cache,
                        ) else { continue };

                        let gx = phys_pad + col as f32 * cw + glyph.x + region.offset_x as f32;
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
        let tab_data: Vec<(String, bool, usize)> = tabs
            .iter()
            .map(|t| (t.title.to_string(), t.active, t.index))
            .collect();

        let sc = self.scale_factor;

        self.draw_rect(&mut bg_verts, 0.0, 0.0, sw, tby,
            [theme.background.r as f32 / 255.0 + 0.04,
             theme.background.g as f32 / 255.0 + 0.04,
             theme.background.b as f32 / 255.0 + 0.05, 1.0]);
        self.draw_rect(&mut bg_verts, 0.0, tby - 1.0, sw, 1.0, [0.22, 0.22, 0.28, 1.0]);

        let left_pad = (78.0 * sc).round();
        let n = tab_data.len().max(1);
        let tab_w = ((sw - left_pad - 32.0 * sc) / n as f32).min(220.0 * sc).max(80.0 * sc);
        let tab_font = (11.5 * sc).round().max(1.0);
        let close_font = (10.0 * sc).round().max(1.0);
        let text_top = (tby - tab_font * 1.2) / 2.0;

        for (i, (title, active, _)) in tab_data.iter().enumerate() {
            let tx = left_pad + i as f32 * tab_w;

            if *active {
                self.draw_rect(&mut bg_verts, tx, 0.0, tab_w, tby,
                    [theme.background.r as f32 / 255.0 + 0.10,
                     theme.background.g as f32 / 255.0 + 0.10,
                     theme.background.b as f32 / 255.0 + 0.12, 1.0]);
                self.draw_rect(&mut bg_verts, tx, 0.0, tab_w, (2.0 * sc).max(2.0),
                    [0.35, 0.53, 0.94, 1.0]);
            }

            let close_w = 16.0 * sc;
            let max_text_w = tab_w - 8.0 * sc - close_w - 6.0 * sc;
            let title_color = if *active { [0.92_f32, 0.92, 0.95, 1.0] } else { [0.52_f32, 0.52, 0.58, 1.0] };
            {
                let metrics = Metrics::new(tab_font, tab_font * 1.4);
                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, max_text_w, tab_font * 2.0);
                buf.set_text(&mut self.font_system, title, Attrs::new().family(Family::Monospace), Shaping::Basic);
                buf.shape_until_scroll(&mut self.font_system, false);
                let bx = tx + 8.0 * sc;
                let by = text_top;
                for run in buf.layout_runs() {
                    for glyph in run.glyphs.iter() {
                        let physical = glyph.physical((0.0, 0.0), 1.0);
                        let Some(region) = self.atlas.get_or_rasterize(physical.cache_key, &mut self.font_system, &mut self.swash_cache) else { continue };
                        let gx = bx + glyph.x + region.offset_x as f32;
                        let gy = by + run.line_y - region.offset_y as f32;
                        let gw = region.width as f32; let gh = region.height as f32;
                        let x0=(gx/sw)*2.0-1.0; let x1=((gx+gw)/sw)*2.0-1.0;
                        let y0=1.0-(gy/sh)*2.0; let y1=1.0-((gy+gh)/sh)*2.0;
                        let [u0,v0,u1,v1]=[region.u0,region.v0,region.u1,region.v1];
                        glyph_verts.extend_from_slice(&[
                            GlyphVertex{pos:[x0,y0],uv:[u0,v0],color:title_color},
                            GlyphVertex{pos:[x1,y0],uv:[u1,v0],color:title_color},
                            GlyphVertex{pos:[x0,y1],uv:[u0,v1],color:title_color},
                            GlyphVertex{pos:[x1,y0],uv:[u1,v0],color:title_color},
                            GlyphVertex{pos:[x1,y1],uv:[u1,v1],color:title_color},
                            GlyphVertex{pos:[x0,y1],uv:[u0,v1],color:title_color},
                        ]);
                    }
                }
            }

            let cx = tx + tab_w - close_w - 2.0 * sc;
            let close_color = if *active { [0.60_f32, 0.60, 0.65, 1.0] } else { [0.35_f32, 0.35, 0.40, 1.0] };
            self.draw_text(&mut glyph_verts, "\u{00d7}", cx, text_top, close_font, close_color);
        }

        if !tab_data.is_empty() {
            let plus_x = left_pad + tab_data.len() as f32 * tab_w + 6.0 * sc;
            self.draw_text(&mut glyph_verts, "+", plus_x, text_top, tab_font, [0.40, 0.40, 0.46, 1.0]);
        }

        let bg_color = theme.background.to_f32();
        self.submit_frame(view, output, bg_verts, glyph_verts,
            [bg_color[0] as f64, bg_color[1] as f64, bg_color[2] as f64, 1.0]);
    }
}
