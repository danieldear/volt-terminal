//! GPU drawing and presentation. Vello encodes/rasterizes the native scene on
//! compute-capable GPUs; only layout, shaping and cached glyph resources use CPU.
use super::{
    gpu_scene::{color, GpuSceneEncoder},
    raster::{Glyph, Shape},
    RasterPainter,
};
use crate::{declarative::PaintBatch, graphics::Image};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use vello::{peniko::ImageData, wgpu};
use winit::window::Window;

pub(super) fn alpha_mode(modes: &[wgpu::CompositeAlphaMode]) -> Option<wgpu::CompositeAlphaMode> {
    [
        wgpu::CompositeAlphaMode::PreMultiplied,
        wgpu::CompositeAlphaMode::PostMultiplied,
    ]
    .into_iter()
    .find(|m| modes.contains(m))
}
// Separate surface configuration from pixel encoding: wgpu (26 through 30)
// advertises PostMultiplied for Metal, although Core Animation consumes premultiplied RGB.
// Keep the advertised/supported SurfaceConfiguration, but compensate in the
// presentation shader. Do not apply this quirk to genuine straight-alpha
// compositors. Revisit on a wgpu pin change: gfx-rs/wgpu#9896, #9922.
fn present_straight_alpha(backend: wgpu::Backend, alpha: wgpu::CompositeAlphaMode) -> bool {
    alpha == wgpu::CompositeAlphaMode::PostMultiplied && backend != wgpu::Backend::Metal
}
fn shader_source(post: bool) -> String {
    format!(
        r#"
@group(0) @binding(0) var pixels:texture_2d<f32>;
@vertex fn vs(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {{
    var p=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));return vec4(p[i],0.,1.);
}}
@fragment fn fs(@builtin(position) pos:vec4<f32>)->@location(0) vec4<f32> {{
    let c=textureLoad(pixels,vec2<i32>(pos.xy),0);
    // Vello's output texture is straight alpha. Adapt on the GPU for the
    // compositor's effective encoding; opaque output naturally has alpha=1.
    if ({post}) {{ return c; }}
    return vec4(c.rgb*c.a,c.a);
}}
"#
    )
}
/// Vello drops a frame that overflows any of its fixed GPU buffers, leaving
/// the previous image in place (a window that looks frozen). Such frames are
/// drawn in software instead; this is reported once per process.
fn report_fallback_once(reason: &str) {
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "Volt UI: frame would overflow Vello's {reason} buffer; drawing such frames in software. Deeply nested clips/opacity over large areas, very dense vector paths/text or many large images cause this."
        );
    }
}
/// Premultiplied RGBA (our `Image` and tiny-skia storage) to straight alpha.
fn unpremultiply(data: &[u8]) -> Cow<'_, [u8]> {
    // Opaque assets (and already-zero transparent pixels) are already straight
    // alpha. Borrow them for queue.write_texture instead of cloning up to 64 MiB.
    if data.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255) {
        return Cow::Borrowed(data);
    }
    let mut out = data.to_vec();
    for p in out.chunks_exact_mut(4) {
        let a = u16::from(p[3]);
        if a != 0 && a != 255 {
            for c in &mut p[..3] {
                *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    Cow::Owned(out)
}
/// Mark the device unusable on loss or an uncaptured validation error (wgpu's
/// default handler panics), so the host can recreate it or fall back.
fn watch_device(device: &wgpu::Device) -> Arc<AtomicBool> {
    let lost = Arc::new(AtomicBool::new(false));
    let flag = lost.clone();
    device.set_device_lost_callback(move |reason, message| {
        eprintln!("Volt UI: GPU device lost ({reason:?}): {message}");
        flag.store(true, Ordering::Relaxed);
    });
    let flag = lost.clone();
    device.on_uncaptured_error(std::sync::Arc::new(move |error: wgpu::Error| {
        eprintln!("Volt UI: GPU error: {error}");
        flag.store(true, Ordering::Relaxed);
    }));
    lost
}
/// Images uploaded once as GPU textures and registered with Vello. Vello still
/// packs them into its per-frame atlas, but by GPU copy instead of re-uploading
/// CPU pixels every frame. LRU-bounded; images used this frame stay resident.
#[derive(Default)]
struct TextureImages {
    map: HashMap<usize, Registered>,
    bytes: usize,
    clock: u64,
}
struct Registered {
    /// Keeps the pixel buffer (the key's address) alive while registered.
    _source: Image,
    data: ImageData,
    bytes: usize,
    used: u64,
}
const TEXTURE_BUDGET: usize = 96 * 1024 * 1024;

/// Admit explicit images before any GPU upload. Area alone is insufficient:
/// a full-width strip and a full-height strip cannot coexist in the atlas.
/// Match Vello's allocator and image order at its maximum atlas size.
fn image_admission(
    batches: &[PaintBatch<Shape, Glyph>],
    max_side: u32,
) -> Result<HashSet<usize>, &'static str> {
    admit_images(
        batches
            .iter()
            .flat_map(|b| &b.shapes)
            .filter_map(|s| match s {
                Shape::Image(_, background, _) => {
                    let image = &background.image;
                    Some((
                        image.pixels().as_ptr() as usize,
                        image.width(),
                        image.height(),
                        image.pixels().len(),
                    ))
                }
                _ => None,
            }),
        max_side,
    )
}

fn admit_images(
    images: impl Iterator<Item = (usize, u32, u32, usize)>,
    max_side: u32,
) -> Result<HashSet<usize>, &'static str> {
    let mut ids = HashSet::new();
    let mut bytes = 0usize;
    let mut atlas = None;
    for (id, w, h, len) in images {
        if !ids.insert(id) {
            continue;
        }
        if w == 0 || h == 0 || w.max(h) > max_side.min(8192) {
            return Err("image dimensions");
        }
        bytes = bytes.checked_add(len).ok_or("image residency")?;
        if bytes > TEXTURE_BUDGET {
            return Err("image residency");
        }
        if atlas
            .get_or_insert_with(|| guillotiere::AtlasAllocator::new(guillotiere::size2(8192, 8192)))
            .allocate(guillotiere::size2(w as i32, h as i32))
            .is_none()
        {
            return Err("image atlas packing");
        }
    }
    Ok(ids)
}

fn encode_frame(
    encoder: &mut GpuSceneEncoder,
    textures: &mut TextureImages,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut vello::Renderer,
    batches: &[PaintBatch<Shape, Glyph>],
    painter: &mut RasterPainter,
) -> Result<(), String> {
    let ids = match image_admission(batches, device.limits().max_texture_dimension_2d) {
        Ok(ids) => ids,
        Err(reason) => {
            encoder.reject_frame(reason);
            return Ok(());
        }
    };
    textures.clock += 1;
    // Protect all needed cache hits, not just those already encountered in
    // draw order, avoiding evict/re-upload churn within the same frame.
    for id in ids {
        if let Some(r) = textures.map.get_mut(&id) {
            r.used = textures.clock;
        }
    }
    let mut resolve = |image: &Image| textures.resolve(image, device, queue, renderer);
    encoder.encode_with(batches, painter, Some(&mut resolve))
}
impl TextureImages {
    fn resolve(
        &mut self,
        image: &Image,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut vello::Renderer,
    ) -> Result<ImageData, String> {
        let id = image.pixels().as_ptr() as usize;
        if let Some(r) = self.map.get_mut(&id) {
            r.used = self.clock;
            return Ok(r.data.clone());
        }
        let (w, h) = (image.width(), image.height());
        if w.max(h) > device.limits().max_texture_dimension_2d {
            return Err("image exceeds GPU texture limits".into());
        }
        let bytes = w as usize * h as usize * 4;
        while self.bytes + bytes > TEXTURE_BUDGET {
            let Some(old) = self
                .map
                .iter()
                .filter(|(_, r)| r.used < self.clock)
                .min_by_key(|(_, r)| r.used)
                .map(|(k, _)| *k)
            else {
                return Err("GPU image residency budget exceeded".into());
            };
            let old = self.map.remove(&old).unwrap();
            self.bytes -= old.bytes;
            renderer.unregister_texture(old.data);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volt UI image"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        // Vello treats registered textures as straight (unpremultiplied) alpha.
        queue.write_texture(
            texture.as_image_copy(),
            &unpremultiply(image.pixels()),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let data = renderer.register_texture(texture);
        self.bytes += bytes;
        self.map.insert(
            id,
            Registered {
                _source: image.clone(),
                data: data.clone(),
                bytes,
                used: self.clock,
            },
        );
        Ok(data)
    }
}
/// Premultiplied tiny-skia pixels to the straight alpha the present shader expects.
/// Render the encoded frame into `target`. A frame the encoder split into
/// strips renders each strip through a reusable strip texture (the scene
/// shifted up by the strip's offset) and copies it into place, so large
/// displays stay on the GPU within Vello's fixed per-pass buffers.
#[allow(clippy::too_many_arguments)]
fn render_frame(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut vello::Renderer,
    encoder: &GpuSceneEncoder,
    target: &wgpu::Texture,
    size: [u32; 2],
    background: [f32; 4],
    strip: &mut Option<(wgpu::Texture, [u32; 2])>,
) -> Result<(), String> {
    let params = |height| vello::RenderParams {
        base_color: color(background),
        width: size[0],
        height,
        antialiasing_method: vello::AaConfig::Area,
    };
    if encoder.bands.len() <= 1 {
        return renderer
            .render_to_texture(
                device,
                queue,
                &encoder.scene,
                &target.create_view(&Default::default()),
                &params(size[1]),
            )
            .map_err(|e| e.to_string());
    }
    let tallest = encoder.bands.iter().map(|b| b[1]).max().unwrap_or(1);
    let want = [size[0], tallest];
    if strip.as_ref().is_none_or(|s| s.1 != want) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Volt UI GPU strip"),
            size: wgpu::Extent3d {
                width: want[0],
                height: want[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        *strip = Some((texture, want));
    }
    let (texture, _) = strip.as_ref().unwrap();
    let view = texture.create_view(&Default::default());
    let mut shifted = vello::Scene::new();
    for &[y, height] in &encoder.bands {
        shifted.reset();
        shifted.append(
            &encoder.scene,
            Some(vello::kurbo::Affine::translate((0., -f64::from(y)))),
        );
        renderer
            .render_to_texture(device, queue, &shifted, &view, &params(height))
            .map_err(|e| e.to_string())?;
        // Queue order keeps this copy ahead of the next strip's render.
        let mut commands = device.create_command_encoder(&Default::default());
        commands.copy_texture_to_texture(
            texture.as_image_copy(),
            wgpu::TexelCopyTextureInfo {
                texture: target,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: size[0],
                height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit([commands.finish()]);
    }
    Ok(())
}
fn straight_rgba(pixmap: &tiny_skia::Pixmap) -> Vec<u8> {
    unpremultiply(pixmap.data()).into_owned()
}
pub(super) enum PresentStatus {
    Presented,
    Retry,
    /// The device was lost or hit an uncaptured error; recreate the presenter.
    Lost,
}
pub(super) struct GpuPresenter {
    instance: wgpu::Instance,
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    texture: Option<wgpu::Texture>,
    bind_group: Option<wgpu::BindGroup>,
    /// Reusable strip target for frames rendered in horizontal bands.
    strip: Option<(wgpu::Texture, [u32; 2])>,
    renderer: vello::Renderer,
    encoder: GpuSceneEncoder,
    textures: TextureImages,
    lost: Arc<AtomicBool>,
    pub adapter_name: String,
}
impl GpuPresenter {
    pub fn new(window: Arc<Window>, transparent: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&adapter);
        let alpha = if transparent {
            alpha_mode(&caps.alpha_modes)
                .ok_or("compositor does not advertise transparent alpha support")?
        } else {
            wgpu::CompositeAlphaMode::Opaque
        };
        let info = adapter.get_info();
        let adapter_name = format!("{} / {:?}", info.name, info.backend);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .ok_or("compositor does not advertise an 8-bit non-sRGB surface")?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Volt UI Vello GPU renderer"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let lost = watch_device(&device);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: 1,
            height: 1,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: alpha,
            view_formats: vec![],
            // Auto keeps wgpu's historical non-HDR choice; the presentation
            // shader below encodes alpha for this surface explicitly.
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
        let post = present_straight_alpha(info.backend, alpha);
        if std::env::var_os("VOLT_UI_TRACE").is_some() {
            eprintln!(
                "VOLT_UI_SURFACE alpha_mode={alpha:?} pixel_encoding={}",
                if post { "straight" } else { "premultiplied" }
            );
        }
        let shader = shader_source(post);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(shader.into()),
        });
        let pipe_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipe_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            instance,
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            layout,
            texture: None,
            bind_group: None,
            strip: None,
            renderer,
            encoder: GpuSceneEncoder::default(),
            textures: TextureImages::default(),
            lost,
            adapter_name,
        })
    }
    /// Encode with GPU-resident images from this presenter's registry.
    fn encode(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
    ) -> Result<(), String> {
        encode_frame(
            &mut self.encoder,
            &mut self.textures,
            &self.device,
            &self.queue,
            &mut self.renderer,
            batches,
            painter,
        )
    }
    pub fn present(
        &mut self,
        batches: &[PaintBatch<Shape, Glyph>],
        painter: &mut RasterPainter,
        background: [f32; 4],
    ) -> Result<PresentStatus, String> {
        if self.lost.load(Ordering::Relaxed) {
            return Ok(PresentStatus::Lost);
        }
        self.encode(batches, painter)?;
        let viewport = painter.gpu_viewport();
        let w = viewport.width as u32;
        let h = viewport.height as u32;
        if w > self.device.limits().max_texture_dimension_2d
            || h > self.device.limits().max_texture_dimension_2d
        {
            return Err("window dimensions exceed GPU texture limits".into());
        }
        if self.texture.is_none() || self.config.width != w || self.config.height != h {
            self.config.width = w;
            self.config.height = h;
            self.surface.configure(&self.device, &self.config);
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            self.bind_group = Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                }],
            }));
            self.texture = Some(texture);
        }
        let texture = self.texture.as_ref().unwrap();
        let framebuffer_upload_bytes = if let Some(reason) = self.encoder.stats().over_budget {
            report_fallback_once(reason);
            let pixmap = painter.rasterize(batches, background)?;
            self.queue.write_texture(
                texture.as_image_copy(),
                &straight_rgba(&pixmap),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
            w as usize * h as usize * 4
        } else {
            render_frame(
                &self.device,
                &self.queue,
                &mut self.renderer,
                &self.encoder,
                texture,
                [w, h],
                background,
                &mut self.strip,
            )?;
            0
        };
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self
                    .instance
                    .create_surface(self.window.clone())
                    .map_err(|e| e.to_string())?;
                self.surface.configure(&self.device, &self.config);
                return Ok(PresentStatus::Retry);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(PresentStatus::Retry)
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("GPU surface validation error".into())
            }
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, self.bind_group.as_ref().unwrap(), &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        self.queue.present(frame);
        if self.lost.load(Ordering::Relaxed) {
            return Ok(PresentStatus::Lost);
        }
        if std::env::var_os("VOLT_UI_TRACE").is_some() {
            let stats = self.encoder.stats();
            eprintln!("VOLT_UI_GPU backend=vello adapter={} shapes={} glyphs={} layers={} spill_slots={} bands={} lines_est={} segments_est={} software_frame={:?} resource_bytes={} framebuffer_upload_bytes={}",self.adapter_name,stats.shapes,stats.glyphs,stats.layers,stats.spill_slots,stats.bands,stats.lines_estimate,stats.segments_estimate,stats.over_budget,stats.resource_bytes,framebuffer_upload_bytes);
        }
        Ok(PresentStatus::Presented)
    }
}

/// Persistent real GPU renderer for explicit headless snapshots/verification.
/// Readback waits for GPU completion and copies pixels to CPU; native windows
/// do not use this path. Its timing is not native presentation or photon latency.
pub struct GpuOffscreenRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: vello::Renderer,
    encoder: GpuSceneEncoder,
    textures: TextureImages,
    lost: Arc<AtomicBool>,
    target: Option<(wgpu::Texture, wgpu::Buffer, [u32; 2])>,
    strip: Option<(wgpu::Texture, [u32; 2])>,
    pub adapter_name: String,
}
impl GpuOffscreenRenderer {
    #[cfg(test)]
    pub(super) fn resident_image_bytes(&self) -> usize {
        self.textures.bytes
    }
    pub fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .map_err(|e| e.to_string())?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Volt UI offscreen GPU"),
            required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let lost = watch_device(&device);
        let renderer = vello::Renderer::new(
            &device,
            vello::RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::area_only(),
                ..Default::default()
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Self {
            device,
            queue,
            renderer,
            encoder: GpuSceneEncoder::default(),
            textures: TextureImages::default(),
            lost,
            target: None,
            strip: None,
            adapter_name: format!("{} / {:?}", info.name, info.backend),
        })
    }
    /// CPU-rendered full-window pixels uploaded by this adapter (always zero).
    /// This does not count Vello's scene/asset uploads or GPU memory traffic.
    pub fn framebuffer_upload_bytes(&self) -> usize {
        self.encoder.stats().framebuffer_upload_bytes
    }
    /// Estimated Vello blend-spill slots used by the latest frame. Frames above
    /// the renderer's fixed budget are drawn in software instead of dropped.
    pub fn last_spill_slots(&self) -> u32 {
        self.encoder.stats().spill_slots
    }
    /// The Vello buffer the latest frame would have overflowed (that frame was
    /// then drawn in software), or `None` when it rendered on the GPU.
    pub fn last_over_budget(&self) -> Option<&'static str> {
        self.encoder.stats().over_budget
    }
    /// Estimated (lines, segments) of the latest frame against the
    /// 2,097,152 capacity; frames over it draw in software.
    pub fn last_path_estimate(&self) -> (u64, u64) {
        let s = self.encoder.stats();
        (s.lines_estimate, s.segments_estimate)
    }
    /// Horizontal strips the latest GPU frame was rendered in (1 = one pass).
    pub fn last_bands(&self) -> usize {
        self.encoder.stats().bands
    }
    /// New CPU-side cached glyph/image bytes constructed in the latest render.
    /// Not a measurement of actual GPU transfers or total GPU memory.
    pub fn resource_bytes(&self) -> usize {
        self.encoder.stats().resource_bytes
    }
    pub fn render(
        &mut self,
        painter: &mut RasterPainter,
        batches: &[PaintBatch<Shape, Glyph>],
        background: [f32; 4],
    ) -> Result<tiny_skia::Pixmap, String> {
        if background
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        {
            return Err("invalid GPU background".into());
        }
        if self.lost.load(Ordering::Relaxed) {
            return Err("GPU device lost".into());
        }
        encode_frame(
            &mut self.encoder,
            &mut self.textures,
            &self.device,
            &self.queue,
            &mut self.renderer,
            batches,
            painter,
        )?;
        if let Some(reason) = self.encoder.stats().over_budget {
            report_fallback_once(reason);
            return painter.rasterize(batches, background);
        }
        let vp = painter.gpu_viewport();
        let size = [vp.width as u32, vp.height as u32];
        if size.contains(&0)
            || size
                .iter()
                .any(|v| *v > self.device.limits().max_texture_dimension_2d)
        {
            return Err("invalid GPU dimensions".into());
        }
        let stride = (size[0] * 4).div_ceil(256) * 256;
        if self.target.as_ref().is_none_or(|t| t.2 != size) {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Volt UI GPU snapshot"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::STORAGE_BINDING
                    | wgpu::TextureUsages::COPY_SRC
                    | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Volt UI explicit readback"),
                size: u64::from(stride) * u64::from(size[1]),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            self.target = Some((texture, buffer, size));
        }
        let (texture, buffer, _) = self.target.as_ref().unwrap();
        render_frame(
            &self.device,
            &self.queue,
            &mut self.renderer,
            &self.encoder,
            texture,
            size,
            background,
            &mut self.strip,
        )?;
        let mut commands = self.device.create_command_encoder(&Default::default());
        commands.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(size[1]),
                },
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([commands.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| e.to_string())?;
        recv.recv_timeout(std::time::Duration::from_secs(30))
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        let mut result =
            tiny_skia::Pixmap::new(size[0], size[1]).ok_or("invalid GPU snapshot size")?;
        {
            let data = buffer
                .slice(..)
                .get_mapped_range()
                .map_err(|e| e.to_string())?;
            for (dst, src) in result
                .data_mut()
                .chunks_exact_mut(size[0] as usize * 4)
                .zip(data.chunks_exact(stride as usize))
            {
                dst.copy_from_slice(&src[..dst.len()]);
                // tiny-skia Pixmap (and PNG saving) require premultiplied RGBA;
                // Vello's render target is straight RGBA. Only explicit
                // verification readback does this CPU conversion.
                for p in dst.chunks_exact_mut(4) {
                    for i in 0..3 {
                        p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
                    }
                }
            }
        }
        buffer.unmap();
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opaque_image_staging_borrows_pixels_and_alpha_conversion_is_exact() {
        let opaque = [255, 128, 64, 255, 0, 0, 0, 0];
        let result = unpremultiply(&opaque);
        assert_eq!(
            result.as_ptr(),
            opaque.as_ptr(),
            "unchanged RGBA must not allocate a staging copy"
        );
        assert_eq!(&*result, &opaque);
        assert_eq!(&*unpremultiply(&[128, 64, 0, 128]), &[255, 128, 0, 128]);
    }
    #[test]
    #[ignore = "timing report; not a performance threshold"]
    fn opaque_upload_staging_timing() {
        use std::{hint::black_box, time::Instant};
        let data = [255, 128, 64, 255].repeat(2048 * 2048);
        let old = |data: &[u8]| {
            let mut out = data.to_vec();
            for p in out.chunks_exact_mut(4) {
                let a = u16::from(p[3]);
                if a != 0 && a != 255 {
                    for c in &mut p[..3] {
                        *c = ((u16::from(*c) * 255 + a / 2) / a).min(255) as u8;
                    }
                }
            }
            out
        };
        black_box(old(black_box(&data)));
        black_box(unpremultiply(black_box(&data)));
        let iterations = 30;
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(old(black_box(&data)));
        }
        let before = start.elapsed().as_secs_f64() * 1000. / f64::from(iterations);
        let start = Instant::now();
        for _ in 0..iterations {
            black_box(unpremultiply(black_box(&data)));
        }
        let after = start.elapsed().as_secs_f64() * 1000. / f64::from(iterations);
        println!("opaque 2048x2048 staging (16 MiB), {iterations} iterations: before={before:.3}ms after={after:.3}ms; temporary copied bytes 16777216 -> 0");
    }
    #[test]
    fn image_preflight_bounds_bytes_dimensions_and_deduplicates_handles() {
        let a = (1, 4096, 4096, 64 * 1024 * 1024);
        assert_eq!(admit_images([a, a].into_iter(), 8192).unwrap().len(), 1);
        assert_eq!(
            admit_images([a, (2, 4096, 4096, 64 * 1024 * 1024)].into_iter(), 8192),
            Err("image residency")
        );
        assert_eq!(
            admit_images([(1, 8193, 1, 8193 * 4)].into_iter(), 8192),
            Err("image dimensions")
        );
        assert_eq!(
            admit_images([(1, 4096, 1, 4096 * 4)].into_iter(), 2048),
            Err("image dimensions")
        );
        assert_eq!(
            admit_images(
                [(1, 8192, 1, 8192 * 4), (2, 1, 8192, 8192 * 4)].into_iter(),
                8192
            ),
            Err("image atlas packing")
        );
        assert_eq!(
            admit_images([(1, 1, 1, usize::MAX), (2, 1, 1, 4)].into_iter(), 8192),
            Err("image residency")
        );
    }
    #[test]
    fn premultiplied_and_straight_alpha_shaders_parse_and_validate() {
        for post in [false, true] {
            let module = wgpu::naga::front::wgsl::parse_str(&shader_source(post)).unwrap();
            wgpu::naga::valid::Validator::new(
                wgpu::naga::valid::ValidationFlags::all(),
                wgpu::naga::valid::Capabilities::empty(),
            )
            .validate(&module)
            .unwrap();
        }
    }
    #[test]
    fn metal_surface_encoding_compensates_for_pinned_wgpu_alpha_advertisement() {
        use wgpu::{Backend, CompositeAlphaMode::*};
        // Our pinned Metal HAL only calls setOpaque(false) for PostMultiplied:
        // it does not turn CAMetalLayer into a straight-alpha compositor.
        assert!(!present_straight_alpha(Backend::Metal, PostMultiplied));
        assert!(!present_straight_alpha(Backend::Metal, PreMultiplied));
        assert!(!present_straight_alpha(Backend::Metal, Opaque));
        for backend in [
            Backend::Vulkan,
            Backend::Dx12,
            Backend::Gl,
            Backend::BrowserWebGpu,
        ] {
            assert!(present_straight_alpha(backend, PostMultiplied));
            assert!(!present_straight_alpha(backend, PreMultiplied));
            assert!(!present_straight_alpha(backend, Opaque));
        }
    }
    #[test]
    #[ignore = "requires real GPU; checks presentation shader pixels, not the OS compositor"]
    fn actual_gpu_present_shader_encodes_metal_and_straight_compositors() {
        let gpu = GpuOffscreenRenderer::new().expect("GPU required");
        let device = &gpu.device;
        let extent = wgpu::Extent3d {
            width: 3,
            height: 1,
            depth_or_array_layers: 1,
        };
        let make_texture = |usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some("presentation alpha regression"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage,
                view_formats: &[],
            })
        };
        let input =
            make_texture(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST);
        let straight = [255, 255, 255, 26, 0, 128, 255, 128, 12, 34, 56, 255];
        gpu.queue.write_texture(
            input.as_image_copy(),
            &straight,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(12),
                rows_per_image: Some(1),
            },
            extent,
        );
        let output =
            make_texture(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        for (backend, alpha, expected) in [
            (
                wgpu::Backend::Metal,
                wgpu::CompositeAlphaMode::PostMultiplied,
                [26, 26, 26, 26, 0, 64, 128, 128, 12, 34, 56, 255],
            ),
            (
                wgpu::Backend::Metal,
                wgpu::CompositeAlphaMode::PreMultiplied,
                [26, 26, 26, 26, 0, 64, 128, 128, 12, 34, 56, 255],
            ),
            (
                wgpu::Backend::Vulkan,
                wgpu::CompositeAlphaMode::PostMultiplied,
                straight,
            ),
        ] {
            let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(
                    shader_source(present_straight_alpha(backend, alpha)).into(),
                ),
            });
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &input.create_view(&Default::default()),
                    ),
                }],
            });
            let mut commands = device.create_command_encoder(&Default::default());
            {
                let view = output.create_view(&Default::default());
                let mut pass = commands.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.draw(0..3, 0..1);
            }
            commands.copy_texture_to_buffer(
                output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                extent,
            );
            gpu.queue.submit([commands.finish()]);
            let (send, receive) = std::sync::mpsc::channel();
            readback
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = send.send(result);
                });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            receive
                .recv_timeout(std::time::Duration::from_secs(30))
                .unwrap()
                .unwrap();
            {
                let pixels = readback.slice(..).get_mapped_range().unwrap();
                for (a, b) in pixels[..12].iter().zip(expected) {
                    assert!(
                        a.abs_diff(b) <= 1,
                        "{backend:?}/{alpha:?}: {:?} != {expected:?}",
                        &pixels[..12]
                    );
                }
            }
            readback.unmap();
        }
    }
    #[test]
    fn transparent_modes_are_explicit_and_prefer_premultiplied() {
        use wgpu::CompositeAlphaMode::*;
        assert_eq!(
            alpha_mode(&[Opaque, PostMultiplied, PreMultiplied]),
            Some(PreMultiplied)
        );
        assert_eq!(alpha_mode(&[Opaque, PostMultiplied]), Some(PostMultiplied));
        assert_eq!(alpha_mode(&[Opaque, Auto, Inherit]), None);
    }
}
