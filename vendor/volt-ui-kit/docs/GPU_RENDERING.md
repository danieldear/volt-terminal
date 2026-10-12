# Native GPU drawing and dependency policy

Current native renderer and dependency policy; validated build commands are in
[validation](VALIDATION.md).
The optional `native-gpu` host now draws
shapes, paths, image/glyph textures, clips, group opacity and shadows with
**Vello 0.6 / wgpu 26**. It no longer software-rasterizes the full window and
uploads that framebuffer for admitted GPU scenes. Detected over-budget frames
deliberately use software fallback; opaque windows use the GPU path too.
See [security/performance hardening](SECURITY_AND_PERFORMANCE.md) for the hard
image registry limit, packing admission, fresh measurements and remaining gaps.

## The boundary

```text
Rust declarations + state + callbacks
                  |
                  v
CPU: validation -> layout -> cosmic-text shaping -> paint commands
                  |                         |
                  |              cached CPU glyph bitmaps (on miss)
                  v                         v
GPU: Vello scene + cached image/glyph resources
     paths / fills / strokes / nested clips / opacity / analytic shadows
                  |
                  v
GPU render texture -> alpha-mode conversion -> wgpu surface -> OS compositor
```

`winit` owns windows/input; it is not a renderer. [Vello](https://docs.rs/vello/0.6.0/vello/)
is a GPU compute 2D renderer. Native drawing sets `use_cpu: false`. CPU work still
includes callbacks, whole-view composition, layout, font shaping, image decoding,
scene encoding and cached glyph bitmap construction. This is not a GPU-only app.

There is one renderer/device per native window, not one per component. The
renderer-independent default toolkit and Volt's existing host adapter do not
create a new GPU context. No Volt checkout/vendor synchronization is performed.

## Run existing apps without changing declarations

```sh
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example catalog
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example desk
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example northbound

# Compare the same compiled app with software drawing:
VOLT_UI_RENDERER=software cargo run --locked --features native-gpu --example catalog

# GPU-capable alpha window; OS blur is a separate platform capability:
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example catalog -- --background blur --window-opacity 0.65
```

| `VOLT_UI_RENDERER` | Behavior |
| --- | --- |
| unset / `auto` | GPU if compiled/available; initialization or drawing errors log opaque software fallback |
| `gpu` | Require `native-gpu` and successful GPU drawing; return errors rather than silently use software |
| `software` | Use tiny-skia/softbuffer even in a GPU-enabled binary; no window alpha/blur guarantee |

Startup logs the selected GPU adapter/backend. Resize recreates targets; surface
loss/outdating retries, while other failures follow the policy above. The native
window path never reads the rendered image back to CPU. Idle redraw stays
event-driven; bounded animation may schedule temporary deadlines.

## Feature-pruned dependencies

- Default library: **only `unicode-width`** as a direct dependency, unchanged.
- `native`: software host/font/image/clipboard dependencies, unchanged.
- `native-gpu`: optional Vello/wgpu/pollster, reusing the same native host.
- Vello defaults disabled; its `wgpu` and `bump_estimate` features are enabled.
- wgpu defaults disabled; `std`/`wgsl` plus **Metal on macOS, DX12 on Windows,
  Vulkan on other targets**. No GLES/WebGPU backend in this feature policy.

Vello is pinned to `=0.6.0`, wgpu to `=26.0.1` to retain Rust **1.87** compatibility.
The latest Vello release has a newer minimum Rust requirement; see its
[upstream changelog](https://github.com/linebender/vello/blob/main/research/CHANGELOG.md).
This is a deliberate compatibility pin, not a claim to use the newest release.

Pruned features reduce unused backend/feature scope, not necessarily the number
of compiled macOS crates. Vello adds drawing dependencies compared with a
texture-only presenter. Do not claim smaller binaries/faster builds without
an equivalent measurement. Inspect the selected graph with `cargo tree --locked
--features native-gpu -e normal` and `cargo tree -e features`.

## Correctness and resource boundaries

- Paint batches preserve ordered scopes across batches. Independent corners,
  fills/strokes, text clipping and rounded/circle clips share physical geometry.
- Path clipping honors the public **even-odd** contract with a GPU destination-in
  mask; same-winding holes remain empty. Group opacity composites overlapping
  siblings once, including inside clips.
- Shadow/glow uses analytic GPU Gaussian-like rounded-rect evaluation. Selected
  corners are evaluated by quadrants. This differs from the software three-box
  approximation and is not guaranteed pixel-identical for extreme blur/corners.
- Image fit/position/tile/opacity are supported. GPU image sampling differs from
  software bicubic scaling; antialiasing also differs. Font selection/shaping and
  editor geometry are shared, but not every final pixel is identical.
- Vello's straight-alpha target is adapted to the surface alpha mode on GPU.
  Explicit snapshots convert readback pixels to premultiplied RGBA for tiny-skia.
- Adapter-side image cache: **64 MiB**; glyph cache: **16 MiB / 4096 entries**;
  shared Swash cache: **16 MiB / 4096 entries**. Framebuffer: **64 megapixels**;
  explicit opacity depth: **24**, clip depth: **48**. Invalid streams return errors.
- These are adapter limits, **not a bound on total GPU memory**. Vello's atlas,
  intermediate surfaces, command buffers and OS compositor memory are additional.

`VOLT_UI_GPU` diagnostics report newly constructed cached resource bytes and
`framebuffer_upload_bytes=0` for admitted GPU scenes; native software fallback
reports its actual RGBA upload payload. Neither is an instrumented GPU bandwidth counter:
Vello still transfers commands/assets. `VOLT_UI_TRACE` reports CPU-side
`gpu_encode_submit_present_ms`, not GPU completion time or display scanout.

## Validation / performance

The hardware acceptance test is opt-in so ordinary tests do not require a GPU:

```sh
cargo test --locked --features native-gpu --lib native::gpu_tests::actual_gpu_readback -- --ignored --nocapture

VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio --snapshot target/gpu-studio.png --size 1280x900 --scale 2
VOLT_UI_RENDERER=gpu cargo run --locked --features native-gpu --example showcase -- --screen studio --bench 60 --bench-target studio-add --size 1280x900 --scale 2
```

GPU snapshots/benchmarks include blocking GPU completion, CPU readback and alpha
conversion; native windows do not read back the rendered framebuffer. These are
not native FPS, utilization, energy or input-to-photon measurements. See
[performance methodology](PERFORMANCE.md) and [validation gates](VALIDATION.md).

## Large scenes and fallback

Vello 0.6 uses fixed per-pass buffers and can silently omit overflowing frames.
Admission accounts for lines/segments, clips/blend spill, image packing, bins,
tiles and the 64-word reservation per 16x16 tile, with headroom.

Area-overflowing scenes can render in up to **16 horizontal GPU strips**, using a
reusable strip texture and queue-ordered copies into the final target. Whole-scene
flattening and resource budgets still apply to every pass. Strips do not make all
large scenes admissible: rejected scenes retain explicit per-frame software fallback.
The actual 6K-strip regression compares readback with software at strip seams.

Software fallback for valid large opacity groups replays into **512px tiles with
4px AA overlap**, rather than allocating a full-window buffer per group. Temporary
group buffers fit about **26 MiB at depth 24**; output/other caches are additional.
The 64-megapixel frame limit and malformed-stream/depth checks remain. This is a
bounded slow path, not a claim of normal GPU performance or exact curved-stroke AA.

## Metal surface alpha and typography

Pinned wgpu 26 advertises `PostMultiplied` on Metal, but Core Animation consumes
premultiplied pixels. The presentation shader separates advertised configuration
from pixel encoding: Metal receives premultiplied RGB, genuine other-backend
straight-alpha surfaces retain straight RGB. The native surface uses non-sRGB
8-bit formats to avoid extra alpha/color gamma encoding. GPU scenes and texture
fallback share this presentation policy. Revisit it when upgrading wgpu.

```text
Vello straight RGBA      GPU presentation      macOS composition
(1,1,1,0.1) white  ->    (0.1,0.1,0.1,0.1) -> correct translucent white
```

`VOLT_UI_TRACE=1` reports `VOLT_UI_SURFACE` encoding and `VOLT_UI_FONTS` family
selection without logging application text. Metal can intentionally report
`alpha_mode=PostMultiplied` and `pixel_encoding=premultiplied` together. SF resolves
to installed SF Pro Text/Display and SF Mono, with portable fallbacks; no Apple
fonts are distributed. Actual presentation-shader and font/weight regressions
are separate from desktop blur and subjective visual acceptance.

Public Material is the normal macOS blur path; private Compositor is opt-in.
See [window opacity/tint/blur](SURFACES_THEMES_AND_SCROLLING.md). Windows/Linux
runtime, device loss under real stress, compositor corners, memory/energy and
assistive-technology acceptance remain separate gates.
