# UI-only GPU renderer prototype

This experiment is **not enabled in Volt**, is not part of the v0.1.15 release,
and does not replace the terminal renderer. It consumes the same pinned toolkit
snapshot as the released integration; no toolkit/vendor source is changed.

## Run

```sh
# Requires a real GPU; fails if the measured scene falls back to software.
cargo run --locked -p volt-renderer --example uigpucheck --features ui-gpu-prototype
# Separate native preview, no PTY, user configuration, or real tasks:
cargo run --locked -p volt-renderer --example uigpucheck --features ui-gpu-prototype -- --interactive
```

PNG pairs and `report.json` are written under `target/uigpucheck`. The preview
uses actual toolkit select/toggle/slider/button components and ordinary Rust
callbacks. Apply and Reset change only the disposable preview state, not Volt's
configuration; Build does not execute a command. It uses a compact hidden-title
macOS window with native traffic lights, matching Volt’s window-attribute
policy, but remains a separate toolkit host—not Volt’s custom tab/window host.

## Architecture boundary

```text
Default Volt (unchanged)
  Settings draft -> UI Kit paint -> Volt atlas/geometry -> wgpu 29 -> window
  Terminal grid -----------------> same optimized renderer ----------^

Opt-in standalone probe
  Disposable Rust state -> UI Kit runtime -> Vello / wgpu 30 -> GPU
                                                    |
                                       PNG / test-only readback
```

The wgpu 29 and wgpu 30 Device/Texture types cannot be shared directly across
these crate versions. The toolkit public offscreen API owns its device and
returns CPU pixels after GPU completion/readback. It does **not** expose a
same-device compositable texture. Uploading that readback into Volt every frame
would add GPU-to-CPU-to-GPU traffic; this probe deliberately does not do that.
A zero-copy production compositor needs an explicit compatible device/texture
adapter (and corresponding upstream API work), not just a Cargo feature flag.

## Tests and timing boundaries

- Four existing Volt palettes × three backing scales × two toggle states.
- Static and restored frames must match within an explicit one-step 8-bit
  quantization tolerance; glyph/image resources must be reused. The first strict
  byte-equality experiment detected three channels differing by one step in a
  680×580 scene. The report retains the maximum observed static delta; this is
  not a claim of bit-identical Vello output.
- Shape/text clipping must leave exterior pixels transparent.
- Hide/show and state/clip restoration must not retain stale pixels.
- GPU fallback is rejected for these representative scenes.
- GPU/software PNG differences are reported, not hidden by a permissive pass
  threshold. Rasterizers may differ in antialiasing; visual acceptance is separate.
- Composition timing and warm render+completion+full-readback median are separate.
  Neither is thread CPU time, production display latency, or TermMark throughput.
- `framebuffer_upload_bytes=0` describes the toolkit encoder's CPU framebuffer
  uploads, not all resource transfers or the test's GPU-to-CPU readback.

The example compares Vello with the toolkit software painter, **not** with
Volt's complete legacy renderer. Existing `settingscheck` remains the separate
legacy renderer visual/cache gate. No cross-renderer speedup is claimed.

## Production migration gates

1. Agree a compatible device/texture bridge with canonical toolkit ownership;
   do not patch the vendored snapshot or upgrade the terminal renderer blindly.
2. Preserve Volt's existing settings transaction and pointer/keyboard/IME routing;
   toolkit preview callbacks are not a substitute for those host contracts.
3. Establish alpha/color-space, clipping, DPI, damage/cached-surface, resize,
   hide/show, GPU-loss/fallback and accessibility contracts for one Settings surface.
4. Compare against the current Volt Settings images and native interaction behavior.
5. Benchmark default terminal output with UI hidden and visible and require matched
   full-app performance acceptance before enabling any production path.
6. Only then extend to workspace cards/search; rich effects remain a later decision.

## Recorded local validation — 2026-10-13

- Release-mode probe passed all 24 palette/DPI/state cases on **Apple M4 Pro / Metal**,
  including clipped exterior alpha, dropdown open/close, toggle restoration,
  hide/show and seven warm submissions per state. No GPU fallback was observed.
- Native preview verified dropdown opening and keyboard selection, live
  palette change, switch click/Space, slider input, disabled-button state and
  resetting the disposable draft. Accessibility exposed buttons, a switch,
  slider and scoped dropdown items. This is not full accessibility certification.
- Default Volt: 717 Rust tests/doc-tests, 37 Python checks, strict Clippy and
  27 existing Settings GPU cases passed. Default normal dependencies still
  contain no Vello, wgpu 30 or toolkit-native cosmic-text 0.14.
- Opt-in example: strict Clippy and Rust 1.89 all-target compilation passed.
- Median composition time across the case matrix: **0.105 ms**.
  Median of warm render/completion/readback medians: **1.784 ms**.
  This is a local offscreen sample, not a production compositor benchmark.
- GPU/software mean absolute channel difference: median
  **0.154/255**. Static maximum channel
  delta was **1/255**. The initial
  strict byte-equality failure and explicit quantization allowance are described
  above; differences are not relabeled as exact matches.
- v0.1.15 is already published and unchanged. This prototype remains on its
  separate branch; no production renderer switch or new release is implied.

## Window appearance correction — 2026-10-13

The original native preview was visually rejected. Its OS-following theme
allowed light native chrome and light default controls to appear on a custom
dark background. The offscreen checks did not catch this because they supplied
an explicit theme; this was a native-host integration gap, not a renderer fault.

The prototype now explicitly derives every component token from its current
palette, requests dark native window appearance, and uses `CompactOverlay`
(transparent title bar, full-size content and hidden title). A reserved empty
header provides a drag region without putting controls under the native traffic
lights. The giant caption and redundant window-sized card border are removed;
the preview starts at 680×500 logical pixels rather than 680×580.

Two opt-in regression tests cover window policy and palette independence from
OS light/dark mode. The updated 24 GPU cases passed. Native QA confirmed visible
colored traffic lights in the focused window, no white title strip, readable
neutral controls and palette changes without mixed light/dark tokens. These
changes affect only the standalone prototype; v0.1.15 remains unchanged.
Timings from different preview dimensions must not be treated as a speedup.
