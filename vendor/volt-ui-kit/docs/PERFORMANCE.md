# Native performance measurement

Performance depends on the scene, renderer, build profile, DPI and device. Use
Catalog for current component workloads; older Showcase/Northbound scenes remain
useful regression fixtures, not comparable benchmark substitutes.

```text
Native input / worker message
             |
Rust callback -> whole-view composition -> validation/layout
             |
       cached text shaping -> paint commands
             |
       +-----+-----------------+
       |                       |
 Vello GPU scene         tiny-skia software
 encode/render/present   incremental damage / fallback
       |                       |
       +------- OS compositor--+
```

## What is optimized

- Local dev profiles optimize the toolkit/dependencies while retaining symbols
  and assertions. Consumer workspaces retain their own profile choices.
- Resolved styles, measured text and shaped buffers are cached. Visible labels
  that fit unwrapped reuse natural-width measurements. Text measurement and
  drawing share attributes and editor geometry.
- Plain scrolling shifts laid-out descendants/recomputes clips; overlays and
  virtual lists conservatively fall back to full layout.
- Split drags update state and patch the document, coalescing layout per frame
  and recomposing on release. Slow application callbacks can still block input.
- Software damage tracks changed batches and respects clip/effect extents;
  structural changes force a full repaint. Bounded opacity tiles handle valid
  large fallback scenes without full-window buffers per nested group.
- GPU drawing uses resident image resources, vector text and conservative fixed
  buffer admission; at most 16 strips keep eligible large scenes on the GPU.
- Font bytes are shared across windows with a 256 MiB aggregate registration
  limit. Image/glyph/effect caches remain independently bounded.
- Settled windows are event-driven. Visible ambient/explicit animation requests
  active frames, paced by GPU presentation or software monitor cadence.

These are not fine-grained reactive updates or GPU damage/cached-scene rendering.
CPU callbacks/layout/shaping/decoding still exist; admitted Vello scenes generally
redraw fully. Cache and intermediate limits are not a bound on total RAM/VRAM.

## Reproduce

```sh
# Native per-stage diagnostics; does not log application text:
VOLT_UI_TRACE=1 VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu --example catalog

# Same app and scene with explicit GPU/software drawing:
VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu --example catalog -- --page button --bench 60 --bench-target b-primary --scale 2
VOLT_UI_RENDERER=software cargo run --release --locked --features native-gpu --example catalog -- --page button --bench 60 --bench-target b-primary --scale 2

# Shared scrolling probe (logical delta):
VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu --example catalog -- --page introduction --bench 60 --bench-scroll 60 --scale 2

# Layout drag, incremental hover, staging and hardware readback probes:
cargo test --locked --all-features --example catalog divider_drag_cost_per_page -- --ignored --nocapture
cargo test --locked --all-features --example northbound -- --ignored --nocapture
cargo test --locked --all-features --lib native::gpu -- --ignored --nocapture
```

Choose a reachable enabled control or pass `--bench-target KEY`; use the same
scene, target, scale and build for before/after runs. Record warm-up/iteration
counts, p50/p95 and cache/resource metrics. Do not benchmark concurrently with
compilation or other heavy tests, and retain raw logs outside the documentation set.

## Interpretation and remaining work

Native trace `mouse-release-to-present` starts when the host handles release,
not when the physical click occurred. It excludes prior OS queue delay and ends
before compositor scan-out. Automation round-trip time is not a render benchmark.
`gpu_encode_submit_present_ms` is a CPU-side interval, not GPU completion time.

GPU snapshots/benchmarks wait for completion, read back to CPU and convert alpha;
native windows do not pay that readback cost. Offscreen comparisons cannot prove
native FPS, energy, utilization, thermal load or input-to-photon latency.

For production claims, measure idle/focus/occlusion, native scroll and animation,
image churn, total process/GPU memory and actual GPU timing. Test real device loss
and large displays on each supported OS. Regressions must retain correct clips,
opaque/translucent overlap, text, hit targets and cache/admission bounds.

A lightweight desktop-style demo can use these widgets inside one app window.
A compositor/window manager and mobile hosts require separate platform/input,
lifecycle, accessibility and security work; see the [roadmap](../ROADMAP.md).
