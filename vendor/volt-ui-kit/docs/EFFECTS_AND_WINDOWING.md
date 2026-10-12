# Selective styling, effects, motion and integrated windows

Implemented in the opt-in Rust builder/runtime and native software adapter.
No JSON authoring bridge, required renderer dependency, or legacy Volt widget
behavior change. This is a bounded first motion slice, not a complete animation
or cross-platform window-management framework.

## Single or multiple selections

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = vstack![
    button("Square").rounded(0).on_click(|_: &mut ()| {}),
    button("Left side").rounded_on(12., Corners::Left).on_click(|_: &mut ()| {}),
    button("Two corners").rounded_on(12., [Corner::TopLeft, Corner::BottomRight])
        .on_click(|_: &mut ()| {}),
].padding(20.5, [Edge::Top, Edge::Left])
 .gap(12., [Axis::Horizontal, Axis::Vertical]).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Selections accept one enum, arrays, array references, slices or vectors.
Duplicates union once; empty arrays are a no-op. Padding selects edges; gaps
select axes, not edges. Responsive values work with either selection form and
last affected-edge/axis setter replaces that property's entire responsive rule
set. Existing scalar/single-enum calls remain valid.

```text
        Top
  TopLeft ------ TopRight
     |              |
Left |              | Right
     |              |
BottomLeft ---- BottomRight
       Bottom
```

`.rounded(value)` still applies to all corners; `.rounded(Rounding::Full)` is a
pill/circle based on the smaller laid-out dimension. `.rounded_on(value,...)`
accepts a `Corner`, a `Corners` side group, or lists of either. The FIRST
`rounded_on` call makes unselected corners square, replacing uniform rounding.
Later `rounded_on` calls update selected corners only; a later `rounded` resets
all four. Top+Left selects three corners. Radius values use logical pixels and
clamp independently to half the smaller dimension. Fill, border and pointer hit
geometry agree. Child clipping remains rectangular by default. Opt into rounded/path masks with
`clip_contents(true)` / `clip(...)`; see [surface composition](SURFACES_THEMES_AND_SCROLLING.md).

Rust `button(...)` now defaults to **2 logical pixels**. Zero is explicitly square;
larger/full rounding is opt-in. The legacy `Button::paint` and existing JSON
button default remain **7 logical pixels**. No effects or motion are added by
default. The Rust builder default is intentionally changed, not pixel-compatible
with its previous 7px appearance.

## Effects and interaction patches

```rust
use volt_ui_kit::ui::prelude::*;
let save: Element<usize> = button("Save").key("save")
    .border_color("#475569").border_width(1.)
    .shadow(Shadow::new().color("#00000040").blur(8.).offset(0., 2.))
    .hover_style(|s| s.background("#326481").border_color("#79CBFF")
        .glow(Glow::new().color("#60a5fa80").blur(12.)))
    .pressed_style(|s| s.opacity(0.7))
    .focus_style(|s| s.border_color("#B3E6FF"))
    .animate_on(Interaction::Hover, Animation::ease_out().duration_ms(120))
    .animate_on(Interaction::Pressed, Animation::ease_out().duration_ms(70))
    .on_click(|count: &mut usize| *count += 1).into();
save.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Shadow specifies color, blur, nonnegative spread and offset. Glow is a shadow
with zero offset. Native blur uses three separable box passes approximating a
Gaussian with sigma=blur/2. This is not backdrop blur or native material glass.
Geometry masks are cached separately from color/alpha; animated color/alpha and
integer-position changes reuse masks. Fractional source-position changes alter
the AA mask key. Premultiplied RGB tints have a separate bounded cache; alpha-only
animation reuses them, while a changed RGB tint is rebuilt. Shadow/glow emit
behind the surface, clipped by the window and explicit clip/scroll ancestors
rather than incidental tight layout-parent bounds. They do not enlarge layout or pointer targets.

`.opacity(0.5)` is true **subtree group opacity**, not independent alpha on every
primitive. Overlapping descendants composite once as a group; glyphs participate.
Opacity zero does not hide, disable, remove focus or suppress callbacks.

Interaction patches apply to their own surface/text: focus, then hover, then
pressed, with later overlapping properties winning. Disabled controls ignore
patches. Panels/text can hover via the topmost visual/interactive route and its
ancestor chain; decorative layers retain established click-through behavior.
Noninteractive containers cannot gain keyboard focus or press capture. Patches
are evaluated once during Rust composition, not callbacks on every frame.
Interaction text-color changes do not re-resolve descendant inherited typography;
apply a text patch to the text itself. Default keyboard focus rings remain
immediate and visible rather than waiting for decorative animation.

## Animation and transitions

`Animation::linear()`, `ease_out()` and `ease_in_out()` accept `duration_ms`.
`animate_on` animates the selected interaction's appearance changes;
`.animation(...)` animates explicit appearance changes for the same key after
Rust recomposition. Color, border color/width and opacity interpolate. Shadow/glow
color/alpha interpolate; blur/spread/offset geometry snaps to the new target to
avoid recalculating blur masks every frame. Layout, fonts, corners, scale/translate,
springs, keyframes, repeats and sequencing are not animated in this slice.

```rust
use volt_ui_kit::ui::prelude::*;
let notice: Element<()> = panel![text("Saved")].key("notification")
    .transition(Transition::fade().duration_ms(180)).into();
notice.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Fade mount/unmount uses stable keys. Same-key replacement/resize does not remount;
a removed-and-reinserted key resumes its in-progress fade. A same key with a
changed component kind starts a fresh lifecycle. Detached nodes retain sampled
visual/interactions, descendant enter opacity and snapshot geometry; they are
paint-only and absent from hit/focus routing. Exits retain sibling paint order,
including sequential sibling removals. Surviving ancestor groups still composite
exits; removed ancestors' sampled opacity is retained. Layout space changes
immediately, so a fade is not a layout transition.

```text
Input -> Rust callback NOW -> view composed once -> new visual targets
                                                |
Clock -> sample current visual -> paint/raster ---+
         no callback, no view composition, no layout tick
```

Interrupted property animations start at the current sampled value, not the old
endpoint. Host clocks supply absolute monotonic `Duration` to
`Runtime::advance_animations`; backwards times clamp. `paint` resolves themed
appearance targets, so the host must paint after changed input before consulting
active motion. Native input already requests that redraw. No background task is
owned by the core runtime.

The native host uses an active-only ~60Hz timer, returns to idle `Wait` on
completion, and settles animation on focus loss, occlusion and suspension.
This is timer pacing, **not display-synchronized/VSync or a guaranteed FPS**.
[Winit timing contract](https://docs.rs/winit/latest/winit/event_loop/enum.ControlFlow.html).

Reduced motion: `WindowOptions::with_reduced_motion(true)` or
`VOLT_UI_REDUCED_MOTION=1` at startup; custom hosts use
`Runtime::set_reduced_motion(true)`. Final styles remain; effects are not removed.
Automatic OS accessibility preference discovery is **not implemented**.

## Volt-style window chrome

```rust,ignore
use volt_ui_kit::native::{NativeOptions, WindowChrome, run_ui_with_options};
let options = NativeOptions::default().with_chrome(WindowChrome::Overlay);
run_ui_with_options(options, App::default(), view)?;
```

Existing `NativeOptions { title, size, document_path }` literals and `run_ui` /
`run` signatures remain valid. Additive `WindowOptions` and `run_ui_with_options`
(`run_with_options` for optional JSON) opt into chrome/reduced motion.

| Chrome | Policy |
|---|---|
| Native | Existing system title bar; default |
| Overlay | macOS transparent, unified title bar + hidden title + full-size content; native traffic lights retained and inset by AppKit (macOS 11+) |
| CompactOverlay | Original compact transparent title bar; for applications with shorter custom headers |
| Borderless | Remove system decorations; application must supply usable window controls |

Neither overlay mode is complete borderless mode. `CompactOverlay` retains
Volt's original custom-tab mode flags. Other platforms deliberately retain a
native decorated title bar for both overlay modes. On macOS 11+, `Overlay`
adds an empty public AppKit `NSToolbar` with unified styling to winit's transparent
full-size titlebar. AppKit owns control size, drawing and resize/fullscreen layout;
Volt does not stretch, repaint or reparent the traffic lights. Earlier macOS
versions retain compact geometry. Native/default and Borderless are unchanged.
The [pinned winit title-bar API](https://docs.rs/winit/0.30.13/winit/platform/macos/trait.WindowAttributesExtMacOS.html)
does not expose a traffic-light inset or unified-toolbar setter.

```text
+---------------------------------------------------+
| o o o  reserved     custom header / controls       |
|---------------------------------------------------|
|                     app content                   |
+---------------------------------------------------+
```

Reserve traffic-light clearance yourself; Catalog uses a 100-point left inset
and a 60-point header for unified `Overlay`. Use `CompactOverlay` for an existing
application whose shorter header depends on the old geometry.
These are example dimensions, not a promise of system metrics on every macOS
version/fullscreen state. `.drag_region(true)` marks a container header. Header
text/decorations inherit the drag region. Interactive controls (including
**disabled controls and controls beneath decorative overlays**) veto dragging;
unrelated topmost layers cannot fall through to an obscured header. Native
window dragging uses the OS drag API and cancels click/key captures. Custom
window-control actions, double-click zoom, title-bar menus and borderless resize
regions are not supplied as declarative components yet.

## Resource limits and adapter contract

Build validation includes inactive patches: opacity 0..1, border 0..16, logical
blur 0..64, spread 0..32, offsets -128..128, duration 0..10000ms, validated hex colors or semantic color tokens.
Corners retain the existing numeric radius 0..256 limit. Invalid views follow
existing atomic installation rules; callback-mutated application state is not
rolled back.

Native raster limits: framebuffer 67,108,864px; individual shadow mask 4,194,304px;
cumulative processed shadow-mask pixels 16,777,216/frame; alpha mask cache
32 entries / 8MiB plus a separate 32MiB RGB tint cache; offscreen group depth 24
and aggregate RGBA buffers 64MiB including root.
Group buffers are cleared/reused and discarded on framebuffer resize. Malformed geometry, clipping and opacity budgets fail explicitly with `Err`.
Valid shadows over the per-effect/frame blur-work budget are skipped and counted
in `RasterPainter::effects_skipped`; decorative shadow failure does not discard
the whole frame.
At most 4096 detached nodes are retained; repeated removals exceeding that cap
settle the oldest exit group early. Motion tracks are pruned after exits expire.

Legacy `Painter` gains **defaulted** methods for independent corners, outlines
and shadow silhouettes; existing adapters do not acquire required methods.
The shadow default is explicitly **unblurred**, not a false blur implementation.
`DocumentPainter::ui_layer_begin/end` default to no-op for source compatibility;
a custom adapter must override both to support group opacity/fades correctly.
RasterPainter implements these effects in software. The optional `native-gpu`
adapter now draws Vello GPU scenes with analytic shadows, nested clipping and
group opacity; see [GPU rendering](GPU_RENDERING.md) for quality differences and
resource limits. No Volt vendor changes.
New effects/corner lists/motion metadata are Rust-only; JSON schema 1 stays strict
and unchanged (new Rust-only fields are not silently accepted from JSON).

## Try and accept

```sh
cargo run --locked --features native --example rust_effects
cargo run --locked --features native --example rust_effects -- --reduced-motion
cargo run --locked --features native --example rust_effects -- --chrome native
cargo test --locked --all-features --all-targets
```

Snapshots support `--snapshot PATH --size WIDTHxHEIGHT --scale NUMBER` and show
settled motion. They do not capture native window chrome or prove OS input.
The effects example adapts its button stack below 768 logical width.

Headless tests validate actual raster pixels, 1x/2x opacity/fade composition,
1x/1.5x/2x example raster/actions, selected corners/DPI, callback timing, cancelled
captures, cache reuse and budgets. **Native acceptance still required:** traffic
lights/fullscreen clearance, dragging header versus controls, actual timer pacing,
OS keyboard/mouse/resize/focus/occlusion, other-platform window behavior. No mobile,
accessibility adapter, IME or full framework completion claim.

### Public macOS material (0.3.0)

`WindowBackground::Blur` uses public AppKit `NSVisualEffectView` in decorated
macOS GPU windows (10.14+). It is a behind-window sibling below the unchanged
winit content view; no custom native buttons, replacement delegates or raw
pointer dereferences. `Opaque`/`Transparent` never install a material. AppKit
chooses the active/inactive look and responds to system material preferences.
The current semantic material is UnderWindowBackground; the developer controls
where it is exposed through their own surface alpha and opaque UI fills.
Unavailable or borderless requests warn and use transparency without blur;
software fallback removes the material and stays opaque. No claim of App Store
certification, comprehensive fullscreen acceptance or content-backdrop blur.


### Independent window opacity and tint

`Application::background_from_state` derives the root opacity/blur from Rust
state without fading foreground components. `UiTheme::with_background_color`
keeps root RGB separate from semantic surface colors. Catalog demonstrates both.
The new explicitly opted-in `WindowBackground::CompositorBlur` uses winit's raw
blur request (private macOS API), not public AppKit material; it is not an App
Store-safe promise. Existing `Blur` and all toolkit defaults retain public material.
See [glass controls and native limits](SURFACES_THEMES_AND_SCROLLING.md); the requested
in-app GPU backdrop renderer remains pending.
