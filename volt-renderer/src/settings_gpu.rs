//! Experimental cached Settings compositor. Existing Volt geometry and atlas
//! remain authoritative. Vello uses its own wgpu 30 device; a bounded CPU
//! readback/upload happens ONLY when Settings geometry invalidates, never on
//! ordinary terminal frames. This is deliberately not described as zero-copy.
use super::{BgVertex, GlyphVertex, Renderer};
use crate::atlas::CpuAtlas;
use anyhow::{anyhow, ensure, Result};
use volt_ui_kit::{
    declarative::{DocumentPainter, PaintBatch},
    graphics::{Image, ImageFit, PathCommand, VectorPath},
    native::{GpuOffscreenRenderer, RasterPainter},
    Painter, Rect, Viewport,
};

const MAX_PIXELS: u64 = 8_388_608;
const MAX_VERTICES: usize = 131_072;
const MAX_GLYPH_BYTES: usize = 16 * 1024 * 1024;
type Batch = PaintBatch<<RasterPainter as Painter>::Shape, <RasterPainter as Painter>::Glyph>;

#[derive(Clone, Debug, Default)]
pub struct SettingsGpuStats {
    pub rebuilds: u64,
    pub uploaded_bytes: u64,
    pub fallbacks: u64,
    pub gpu_initializations: u64,
    pub pipeline_creations: u64,
    pub last_error: Option<String>,
}
struct Surface {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}
struct Compositor {
    format: wgpu::TextureFormat,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
}
pub(super) struct SettingsGpuLayer {
    enabled: bool,
    unavailable: bool,
    gpu: Option<GpuOffscreenRenderer>,
    painter: Option<RasterPainter>,
    compositor: Option<Compositor>,
    surface: Option<Surface>,
    stats: SettingsGpuStats,
}
impl SettingsGpuLayer {
    pub fn from_environment() -> Self {
        Self {
            enabled: std::env::var("VOLT_UI_GPU_SETTINGS").as_deref() == Ok("1"),
            unavailable: false,
            gpu: None,
            painter: None,
            compositor: None,
            surface: None,
            stats: SettingsGpuStats::default(),
        }
    }
    pub fn ready(&self) -> bool {
        self.enabled && self.surface.is_some()
    }
    pub fn hide(&mut self) {
        self.surface = None;
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        atlas: &CpuAtlas,
        bg: &[BgVertex],
        glyphs: &[GlyphVertex],
    ) {
        self.hide();
        if !self.enabled {
            return;
        }
        let result = self.build(device, queue, format, size, atlas, bg, glyphs);
        match result {
            Ok(surface) => {
                if self.stats.rebuilds == 0 {
                    eprintln!("Volt: experimental Vello Settings layer active (cached dirty-only cross-device transfer)");
                }
                self.stats.rebuilds += 1;
                self.stats.uploaded_bytes += u64::from(size[0]) * u64::from(size[1]) * 4;
                self.stats.last_error = None;
                self.surface = Some(surface);
            }
            Err(error) => {
                let message = error.to_string();
                if self.stats.last_error.as_ref() != Some(&message) {
                    eprintln!("Volt: Vello Settings fallback to existing renderer: {message}");
                }
                self.stats.last_error = Some(message);
                self.stats.fallbacks += 1;
            }
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn build(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        atlas: &CpuAtlas,
        bg: &[BgVertex],
        glyphs: &[GlyphVertex],
    ) -> Result<Surface> {
        ensure!(
            size.iter()
                .all(|v| *v > 0 && *v <= device.limits().max_texture_dimension_2d)
                && u64::from(size[0]) * u64::from(size[1]) <= MAX_PIXELS,
            "Settings surface exceeds bounded transfer budget"
        );
        ensure!(!self.unavailable, "UI GPU initialization previously failed");
        let viewport = Viewport::new(size[0] as f32, size[1] as f32, 1.).unwrap();
        let painter = self
            .painter
            .get_or_insert_with(|| RasterPainter::new(viewport));
        painter.set_viewport(viewport);
        let batches = geometry(painter, size, atlas, bg, glyphs)?;
        if self.gpu.is_none() {
            match GpuOffscreenRenderer::new() {
                Ok(gpu) => {
                    self.gpu = Some(gpu);
                    self.stats.gpu_initializations += 1;
                }
                Err(e) => {
                    self.unavailable = true;
                    return Err(anyhow!(e));
                }
            }
        }
        let gpu = self.gpu.as_mut().unwrap();
        let image = gpu
            .render(painter, &batches, [0.; 4])
            .map_err(|e| anyhow!(e))?;
        ensure!(
            gpu.last_over_budget().is_none(),
            "toolkit scene exceeded GPU resource budget"
        );
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("cached Vello Settings layer"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            texture.as_image_copy(),
            image.data(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size[0] * 4),
                rows_per_image: Some(size[1]),
            },
            texture.size(),
        );
        if self.compositor.as_ref().is_none_or(|c| c.format != format) {
            self.compositor = Some(Compositor::new(device, format));
            self.stats.pipeline_creations += 1;
        }
        let layout = &self.compositor.as_ref().unwrap().layout;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cached UI texture"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            }],
        });
        Ok(Surface {
            _texture: texture,
            bind_group,
        })
    }
    pub fn composite<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        if let (Some(surface), Some(compositor)) = (
            self.surface.as_ref().filter(|_| self.enabled),
            self.compositor.as_ref(),
        ) {
            pass.set_pipeline(&compositor.pipeline);
            pass.set_bind_group(0, &surface.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}
impl Compositor {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI texture bridge"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: false },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI layer composite"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
@group(0) @binding(0) var image: texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32) -> @builtin(position) vec4<f32> {
  var p = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
  return vec4(p[i],0.,1.);
}
@fragment fn fs(@builtin(position) p:vec4<f32>) -> @location(0) vec4<f32> {
  return textureLoad(image, vec2<i32>(p.xy), 0);
}
"#
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI layer layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI layer composite"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            format,
            layout,
            pipeline,
        }
    }
}
fn position(p: [f32; 2], size: [u32; 2]) -> [f32; 2] {
    [
        (p[0] + 1.) * size[0] as f32 * 0.5,
        (1. - p[1]) * size[1] as f32 * 0.5,
    ]
}
fn geometry(
    p: &mut RasterPainter,
    size: [u32; 2],
    atlas: &CpuAtlas,
    bg: &[BgVertex],
    glyphs: &[GlyphVertex],
) -> Result<Vec<Batch>> {
    ensure!(
        bg.len() <= MAX_VERTICES
            && glyphs.len() <= MAX_VERTICES
            && bg.len().is_multiple_of(3)
            && glyphs.len().is_multiple_of(6),
        "invalid/budgeted Settings mesh"
    );
    let mut batch = PaintBatch {
        shapes: vec![],
        glyphs: vec![],
        clip: Rect::new(0., 0., size[0] as f32, size[1] as f32).unwrap(),
    };
    // Compound same-color triangles preserve shared edges without antialiasing
    // each diagonal separately. Current Settings meshes have uniform colors.
    let mut path = VectorPath::new();
    let mut color = None;
    for triangle in bg.as_chunks::<3>().0 {
        ensure!(
            triangle
                .iter()
                .all(|v| v.pos.iter().all(|v| v.is_finite()) && v.color == triangle[0].color),
            "unsupported Settings mesh colors/positions"
        );
        if color.is_some_and(|c| c != triangle[0].color) {
            p.ui_path(&mut batch.shapes, &path, color.unwrap(), None);
            path = VectorPath::new();
        }
        color = Some(triangle[0].color);
        let points = triangle.map(|v| position(v.pos, size));
        path.commands.extend([
            PathCommand::Move(points[0]),
            PathCommand::Line(points[1]),
            PathCommand::Line(points[2]),
            PathCommand::Close,
        ]);
    }
    if let Some(c) = color {
        p.ui_path(&mut batch.shapes, &path, c, None);
    }
    let mut glyph_bytes = 0;
    for quad in glyphs.as_chunks::<6>().0 {
        ensure!(
            quad.iter()
                .all(|v| v.pos.iter().chain(&v.uv).all(|v| v.is_finite())
                    && v.color == quad[0].color),
            "unsupported Settings glyph mesh"
        );
        let points = quad.map(|v| position(v.pos, size));
        let bounds = |axis| {
            points
                .iter()
                .map(|p| p[axis])
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                    (a.min(v), b.max(v))
                })
        };
        let (x, right) = bounds(0);
        let (y, bottom) = bounds(1);
        let uv = |axis, dim: f32| {
            quad.iter()
                .map(|v| v.uv[axis] * dim)
                .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| {
                    (a.min(v), b.max(v))
                })
        };
        let (u0, u1) = uv(0, atlas.width as f32);
        let (v0, v1) = uv(1, atlas.height as f32);
        ensure!(
            u0 >= 0. && v0 >= 0. && u1 <= atlas.width as f32 && v1 <= atlas.height as f32,
            "Settings glyph outside atlas"
        );
        let (sx, sy, w, h) = (
            u0.round() as u32,
            v0.round() as u32,
            (u1.round() - u0.round()) as u32,
            (v1.round() - v0.round()) as u32,
        );
        if w == 0 || h == 0 || right <= x || bottom <= y {
            continue;
        }
        let len = w as usize * h as usize * 4;
        glyph_bytes += len;
        ensure!(
            glyph_bytes <= MAX_GLYPH_BYTES,
            "Settings glyph transfer budget exceeded"
        );
        let c = quad[0].color;
        ensure!(
            c.iter().all(|v| v.is_finite() && (0. ..=1.).contains(v)),
            "invalid glyph tint"
        );
        let mut pixels = Vec::with_capacity(len);
        for row in sy..sy + h {
            for col in sx..sx + w {
                let offset = ((row * atlas.width + col) * 4) as usize;
                for (i, tint) in c.into_iter().enumerate() {
                    pixels.push((atlas.data[offset + i] as f32 * tint).round() as u8);
                }
            }
        }
        let image = Image::from_rgba(w, h, pixels).map_err(|e| anyhow!(e))?;
        p.ui_image(
            &mut batch.shapes,
            [x, y, right - x, bottom - y],
            &image.background().fit(ImageFit::Stretch),
            1.,
        );
    }
    Ok(vec![batch])
}
impl Renderer {
    /// Only experimental builds expose this switch. No configuration is written.
    pub fn set_settings_gpu_enabled(&mut self, enabled: bool) {
        self.settings_gpu.enabled = enabled;
        self.settings_gpu.unavailable = false;
        self.settings_gpu.hide();
        self.settings_cache = None;
    }
    pub fn settings_gpu_stats(&self) -> SettingsGpuStats {
        self.settings_gpu.stats.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_settings_geometry_is_rejected_before_gpu_work() {
        let mut p = RasterPainter::new(Viewport::new(64., 64., 1.).unwrap());
        let atlas = CpuAtlas::new(4, 4);
        let bad = BgVertex {
            pos: [f32::NAN, 0.],
            color: [1.; 4],
        };
        assert!(geometry(&mut p, [64, 64], &atlas, &[bad; 3], &[]).is_err());
        let bad = BgVertex {
            pos: [0., 0.],
            color: [f32::NAN; 4],
        };
        assert!(geometry(&mut p, [64, 64], &atlas, &[bad; 3], &[]).is_err());
        assert!(geometry(
            &mut p,
            [64, 64],
            &atlas,
            &[],
            &[GlyphVertex {
                pos: [0.; 2],
                uv: [2.; 2],
                color: [1.; 4]
            }; 6]
        )
        .is_err());
        assert!(geometry(
            &mut p,
            [64, 64],
            &atlas,
            &[BgVertex {
                pos: [0.; 2],
                color: [1.; 4]
            }],
            &[]
        )
        .is_err());
    }
}
