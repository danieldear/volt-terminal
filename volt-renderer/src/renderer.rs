use std::sync::Arc;

use cosmic_text::{Attrs, Buffer, FontSystem, Metrics, Shaping, SwashCache};
use wgpu::util::DeviceExt;

use crate::atlas::CpuAtlas;
use crate::pipeline::{bg_pipeline, glyph_pipeline, BgVertex, GlyphVertex};
use volt_config::Theme;
use volt_core::grid::Grid;

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
    font_size: f32,
}

impl Renderer {
    pub async fn new(window: Arc<winit::window::Window>, font_size: f32) -> Self {
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

        // Atlas texture (R8 single channel, 1024x1024)
        const ATLAS_SIZE: u32 = 1024;
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

        let font_system = FontSystem::new();
        let cell_width = font_size * 0.6;
        let cell_height = font_size * 1.4;

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
            font_size,
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

    /// Returns (cols, rows) based on current surface size and cell dimensions
    pub fn grid_size(&self) -> (usize, usize) {
        let cols = (self.config.width as f32 / self.cell_width).floor() as usize;
        let rows = (self.config.height as f32 / self.cell_height).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    pub fn render(&mut self, grid: &Grid, theme: &Theme) {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(o) | wgpu::CurrentSurfaceTexture::Suboptimal(o) => o,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = output.texture.create_view(&Default::default());

        let w = self.config.width as f32;
        let h = self.config.height as f32;
        let cw = self.cell_width;
        let ch = self.cell_height;

        // --- Background quads ---
        let mut bg_verts: Vec<BgVertex> = Vec::with_capacity(grid.cols * grid.rows * 6);
        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                let color = cell.bg.resolve_bg(theme).to_f32();
                let x0 = (col as f32 * cw / w) * 2.0 - 1.0;
                let x1 = ((col as f32 + 1.0) * cw / w) * 2.0 - 1.0;
                let y0 = 1.0 - (row as f32 * ch / h) * 2.0;
                let y1 = 1.0 - ((row as f32 + 1.0) * ch / h) * 2.0;
                // Two triangles per quad
                bg_verts.extend_from_slice(&[
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
        }

        // --- Glyph quads ---
        let mut glyph_verts: Vec<GlyphVertex> = Vec::new();
        let metrics = Metrics::new(self.font_size, ch);

        for row in 0..grid.rows {
            for col in 0..grid.cols {
                let cell = grid.cell(col, row);
                if cell.c == ' ' {
                    continue;
                }

                let mut buf = Buffer::new(&mut self.font_system, metrics);
                buf.set_size(&mut self.font_system, cw * 2.0, ch * 2.0);
                buf.set_text(
                    &mut self.font_system,
                    &cell.c.to_string(),
                    Attrs::new(),
                    Shaping::Basic,
                );
                buf.shape_until_scroll(&mut self.font_system, false);

                for layout_run in buf.layout_runs() {
                    for glyph in layout_run.glyphs.iter() {
                        let physical = glyph.physical((0.0, 0.0), 1.0);
                        let cache_key = physical.cache_key;

                        let Some(region) = self.atlas.get_or_rasterize(
                            cache_key,
                            &mut self.font_system,
                            &mut self.swash_cache,
                        ) else {
                            continue;
                        };

                        let gx = col as f32 * cw + glyph.x + region.offset_x as f32;
                        let gy =
                            row as f32 * ch + layout_run.line_y - glyph.y + region.offset_y as f32;
                        let gw = region.width as f32;
                        let gh = region.height as f32;

                        let x0 = (gx / w) * 2.0 - 1.0;
                        let x1 = ((gx + gw) / w) * 2.0 - 1.0;
                        let y0 = 1.0 - (gy / h) * 2.0;
                        let y1 = 1.0 - ((gy + gh) / h) * 2.0;

                        let color = cell.fg.resolve_fg(theme).to_f32();
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
        }

        // Upload atlas if dirty
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

        // Create GPU buffers
        let bg_buf = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bg_verts"),
                contents: bytemuck::cast_slice(&bg_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });

        // glyph_buf needs at least 1 vertex to avoid empty buffer
        let glyph_buf = if glyph_verts.is_empty() {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glyph_verts_empty"),
                    contents: bytemuck::cast_slice(&[GlyphVertex {
                        pos: [0.0; 2],
                        uv: [0.0; 2],
                        color: [0.0; 4],
                    }]),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        } else {
            self.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("glyph_verts"),
                    contents: bytemuck::cast_slice(&glyph_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                })
        };

        let bg_color = theme.background.to_f32();
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

            // Draw backgrounds
            pass.set_pipeline(&self.bg_pipeline);
            pass.set_vertex_buffer(0, bg_buf.slice(..));
            pass.draw(0..bg_verts.len() as u32, 0..1);

            // Draw glyphs
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
