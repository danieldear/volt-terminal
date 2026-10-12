# Themes, surfaces, clipping and scrolling

All examples below are Rust declarations, not JSON. The core remains renderer-independent;
`native` supplies image decoding and software rendering; `native-gpu` adds an optional
alpha-capable window renderer. Legacy `Theme`, `NativeOptions`, `WindowOptions` fields
and existing `Painter` implementations remain source-compatible.

## Try the examples

For everyday component testing, use [Volt Catalog](CATALOG.md). The focused
surface examples below remain regression fixtures for learning and isolation:

```sh
cargo run --features native --example rust_surfaces
cargo run --features native --example rust_surfaces -- --theme light
cargo run --features native --example rust_scroll
cargo run --features native --example rust_effects -- --theme light
cargo run --features native-gpu --example rust_transparent
```

The first two new examples support `--snapshot /absolute/path.png`, `--size 600x760`,
`--scale 2`, and `--theme light|dark|system`. Headless `system` uses a dark fallback;
it does not pretend to discover the desktop appearance. Snapshot rendering is not
proof of native input, window alpha or backdrop blur.

## Semantic colors and global accents

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = panel![
    text("Profile").color(ColorToken::BaseContent),
    text("Saved").color(ColorToken::Success),
].background(ColorToken::Base200).padding(24., Edge::All).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Roles include Base100/200/300, BaseContent, Primary/PrimaryContent,
Secondary/SecondaryContent, Accent/AccentContent, Neutral/NeutralContent,
Info, Success, Warning, Error and their matching `*Content` foregrounds,
Border and Muted. Rust tokens lower to validated `@role` strings; literal hex
colors still work and remain explicit overrides. Text color tokens inherit;
backgrounds and local accents do not. Tokens resolve on paint, not at composition.

For a native application:

```rust,no_run
use volt_ui_kit::{native::{NativeOptions, run_ui_with_options}, ui::prelude::*};
let options = NativeOptions::default()
    .with_theme(ThemeMode::System)
    .with_accent([0.38, 0.42, 0.92, 1.]);
run_ui_with_options(options, (), |_| {
    vstack![text("Hello"), button("Save").primary(true).on_click(|_: &mut ()| {})]
        .background(ColorToken::Base100).color(ColorToken::BaseContent).into()
})?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`with_theme` also accepts Light and Dark. `with_palette(Palette)` supplies a named,
fully custom set of semantic colors. `Palette::dark()/light().with_accent(rgba)` is
an easy starting point; `with_accent` updates both the primary/accent colors and
chooses contrasting black/white foregrounds for opaque fills. Translucent fills
still need a foreground appropriate to their eventual backdrop.

`run_ui_with_theme(options, state, view, |state, system_mode| UiTheme)` lets Rust
application state select the palette. See the Light / Dark button in
`rust_surfaces`. The native host observes winit theme changes where supported.
On platforms without usable system-theme reporting, System falls back to Dark;
it is not a universal desktop-settings integration. A state-derived palette updates
native chrome for explicit Light/Dark configuration; System keeps OS-managed chrome.

Legacy `runtime.paint(painter, &Theme)` keeps its default recipe. Hosts using new
semantic palettes call `runtime.paint_ui(painter, &UiTheme)`.

## Image backgrounds on panels and stacks

Load once outside `view`, then clone the shared handle:

```rust,no_run
use volt_ui_kit::{native::load_image, ui::prelude::*};
let image = load_image("assets/panel.jpg")?;
let ui: Element<()> = vstack![text("Welcome")]
    .background(ColorToken::Base200)
    .background_image(image.background()
        .fit(ImageFit::Cover)
        .position(ImagePosition::Center)
        .opacity(0.35))
    .rounded(16.)
    .clip_contents(true)
    .into();
ui.build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Fits: Cover (crop to fill), Contain (whole image), Stretch, Tile. Position options
include Center, the four sides and the four corners. Tiles use intrinsic logical
size, scaled for DPI. Images respect the surface's rounded silhouette and inset
border. Clones share premultiplied pixels; paint does not read files or decode.
No implicit URLs, network downloads, SVG scripts or external-resource loading.
PNG/JPEG loaders cap encoded input at 16 MiB, dimensions at 4096 per side and
decoder allocation at 128 MiB. `Image::from_rgba` accepts host-supplied RGBA with
a 16 MP per-image limit and premultiplies it once.

### Three different kinds of opacity

```text
image.opacity(0.35)         -> background image only
.background_opacity(0.35)   -> color and image; text/children unchanged
.opacity(0.35)              -> entire subtree, including effects and children
```

Whole-subtree opacity does not disable input; use enabled state or remove controls
when that is intended. Image opacity multiplies background opacity. An opaque
background color remains visible behind a faded image.

## Vector canvas and clipping

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = canvas(|c| {
    c.fill(VectorPath::circle(60., 60., 40.), ColorToken::Accent);
    c.stroke(VectorPath::new().move_to(10., 100.)
        .cubic_to([35., 10.], [90., 10.], [110., 100.]), "#FFFFFF", 2.);
}).width(120.).height(120.).clip(Clip::Circle);
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

The Rust closure runs when composing the view, not on every animation frame.
Commands are retained: rectangles, circles, ellipses, moves/lines, quadratic/cubic
curves, fill and stroke. `path.transformed([a,b,c,d,tx,ty])` supplies affine
translation/scale/rotation. Coordinates are local logical pixels; runtime maps
paths to physical pixels. Fills and path clips use even-odd winding. This is a
vector drawing API, not an SVG-file parser.

`clip_contents(true)` uses the surface's selective rounded corners; false removes
that explicit clip. `clip(Clip::Bounds|Rounded|Circle|Path(path))` selects a mask.
Circle is the centered circle inscribed in the bounds, not an ellipse. Path clips
use local logical coordinates. Input follows the same ancestor clip geometry;
curve input testing uses bounded flattening, while painting has antialiased edges.

```text
subtree opacity group (optional)
  outer shadow / glow                 <-- overflow through ordinary layout parents
  content mask (explicit, optional)
    surface color + image             <-- image also preserves rounded border
    text / vectors / child elements
  scrollbar overlay (scroll views)
end opacity group
```

The surface's own outer glow/shadow is not cut off by its content mask. Descendant
effects obey explicit clip/scroll ancestors and the window boundary. Custom path
content masks do not change the underlying rounded-surface shadow silhouette.
This fixes the prior three-sided clipping caused by tight hstacks. Existing
rectangular layout/hit bounds and padding behavior otherwise remain unchanged.
Native clip masks are cached with a bounded 8 MiB / 32-entry budget.

New `DocumentPainter` image/path/clip hooks have default no-op implementations,
so old adapters compile unchanged. An embedding adapter must implement these
hooks to render the new capabilities; only the supplied native adapter is
verified for them. Do not assume a legacy adapter provides clipping or images.

## Overflow scrolling

```rust
use volt_ui_kit::ui::prelude::*;
let rows = (0..100).map(|i| text(format!("Row {i}")).height(32.).into());
let ui: Element<()> = vstack![
    text("Fixed header").height(40.),
    vstack(rows).key("list").flex(1.).scroll(Scroll::Auto),
].into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Modes: Vertical, Horizontal, Both, Auto. Auto retains natural wrapping in the
cross axis, but permits explicitly wider/taller children to overflow; it enables
scrolling only on axes with excess content. Give nested scroll views a fixed
height/width or a flex allocation so they have a meaningful viewport.

Wheel/trackpad events scroll the deepest view under the pointer. At its boundary,
only the unconsumed delta bubbles to scrollable ancestors. Shift-wheel supports
horizontal scrolling. Offsets use logical pixels, survive matching keyed
recomposition, and clamp after content/viewport changes. Offscreen items do not
receive pointer events. Scroll views automatically clip overflow and draw thin
position indicators. `.scrollbars(Scrollbars::Hidden)` hides only those indicators,
while keeping wheel/trackpad scrolling and clipping enabled. Visible indicators are draggable and keyboard focus reveals mounted offscreen controls, including nested scroll views. Hidden indicators never disable wheel scrolling. Touch/drag content scrolling and application-driven smooth scrolling remain follow-up work.

Embedding hosts call `runtime.scroll_at(physical_x, physical_y, logical_delta)`
and then layout/redraw if it returns true. Positive deltas move toward the end.
Ordinary scrolling does not invoke handlers or rebuild the Rust view. Controlled virtual lists dispatch `WindowChanged`; native hosts use `scroll_outcome` and recompose the bounded row window when needed. Scrolling is opt-in:
we do not silently change existing container sizing or JSON behavior.

## Real window transparency and native blur

```rust,no_run
use volt_ui_kit::{native::{NativeOptions, WindowBackground}, ui::prelude::*};
let options = NativeOptions::default().with_theme(ThemeMode::System)
    .with_background(WindowBackground::Blur { opacity: 0.65 });
```

`WindowBackground::Transparent { opacity }` requests alpha without blur;
Opaque is the backward-compatible default. The opacity controls the root clear alpha;
text and controls remain opaque unless their own modifiers request otherwise.
**Do not paint an opaque root background** if the desktop should remain visible.

`native-gpu` now draws Vello GPU scenes for opaque/alpha windows, adapting the
output to compositor pixel encoding (including the pinned Metal workaround). It does not
make the default widget library GPU-dependent. Without this feature, or when
initialization is unavailable in `auto` mode, the host logs opaque software
fallback. `VOLT_UI_RENDERER=gpu` instead requires successful GPU drawing. See
[GPU renderer/dependency policy](GPU_RENDERING.md).
It never labels an opaque tint/shadow as backdrop blur.

winit blur is an OS request: macOS and KDE Wayland support vary; Windows/X11 and
other unsupported platforms do not gain native blur from this API. There is no
portable backdrop blur-radius control. Pixel tests verify premultiplied rendering;
desktop-through transparency, blur, OS appearance changes and trackpad feel require device/platform acceptance testing. `WindowBackground::Blur` requests a public `NSVisualEffectView` behind-window
material on decorated macOS GPU windows, without invoking winit's private blur API. Borderless/unsupported requests warn and retain transparency. Packaging/dependency review, fullscreen and live accessibility preference acceptance still gate release claims.

Primary references: [semantic colors](https://daisyui.com/docs/colors/),
[winit window capabilities](https://docs.rs/winit/0.30.13/winit/window/struct.Window.html),
[softbuffer RGB format](https://docs.rs/softbuffer/0.4.8/softbuffer/struct.Buffer.html).
The image decoder is pinned to 0.25.8 for Rust 1.87 compatibility. The optional
renderer uses Vello 0.6.0/wgpu 26.0.1 with pruned backend features; the complete
GPU all-target build is checked with Rust 1.87. Newer Vello needs newer Rust.

## Window opacity, tint and blur

Catalog keeps separate controls for **Background opacity**, **Tint strength**,
**Backdrop blur**, and (on macOS) **Material / Compositor**. Opacity is the native
root clear's alpha, not a subtree-opacity animation. The content sheet and text
remain opaque. Tint mixes the selected accent (or a neutral surface for Zinc) into root RGB, not
another alpha layer. Default opacity is 72%, tint 0%; the Catalog opacity control
has a 10% floor to avoid macOS zero-alpha hit-test fall-through. Applications can
request any finite opacity from 0 to 1.

```text
Window content behind app -> selected native blur -> root RGB + opacity
                                                       |
                                                       +-> text/controls on top

Opacity: coverage / see-through strength       10% ... 100% ... 10%
Tint:    accent contribution to background RGB  0% ... 100% (independent)
Blur:    native blur on/off + explicit source   never implied by tint
```

- `WindowBackground::Blur`: existing public AppKit material on macOS, unchanged.
  The material itself adds system tint/opacity; a root alpha of 10% does not mean
  the **combined native material** has 10% opacity. This limitation is labeled.
- `WindowBackground::CompositorBlur`: explicit winit compositor request. Volt
  Terminal uses this path. Pinned winit calls private
  `CGSSetWindowBackgroundBlurRadius` on macOS (radius 80); **not an App Store-safe
  distribution claim**. No radius slider or cross-platform pixel equivalence is
  promised. It has no permanent AppKit material tint underneath the root clear.
- `Transparent`: alpha without blur. `Opaque`: solid root. Software fallback
  remains opaque and disables an opted-in compositor blur after GPU loss/error.
- Raw macOS blur checks Reduce Transparency on each redraw: disable the blur and
  use an opaque clear while enabled. There is no new idle preference observer;
  a subsequent frame/focus/input is needed to apply that preference.

The ordinary GPU Catalog defaults still use public material. To try Terminal-style
behavior, select **Compositor** in Catalog or explicitly launch:

```sh
cargo run --locked --release --features native-gpu,native-desktop --example catalog -- \
  --background compositor --window-opacity 0.72 --tint 0
```

### State-derived window backgrounds

```rust,ignore
Application::new("My app", state, view)
    .background_from_state(|app| WindowBackground::CompositorBlur {
        opacity: app.background_opacity,
    })
    .theme_with(|app, mode| {
        theme(app, mode).with_background_color(app.background_color)
    });
```

`background_from_state` runs after changed/activated input, native command and
background-message updates. Nonfinite or out-of-range values are rejected and retain
the previous surface configuration. Initial options are validated before OS
creation. Changing opacity within a translucent mode does not recreate the device;
changing between `Opaque` and translucent modes does recreate the GPU surface.
`background(...)` called afterward clears the state callback (last builder wins).
`UiTheme::with_background_color` overrides the root only: semantic palette colors
remain intact. The host rejects invalid RGBA on rendering. Existing public options
struct fields, `ApplicationRequest` equality, default dependencies, legacy Painter
and Volt's vendored snapshot remain unchanged. `WindowBackground` gained an
additional variant; exhaustive downstream matches must cover it.

### Native transition and renderer limitations

Public Material is the default for decorated macOS GPU windows. Compositor blur
is a private-API opt-in. The reported **Material -> Compositor square external
corners are not certified fixed**. The current host retains/hides its native
Material underlay between modes, refreshes native views/shadow, and prepares the
backdrop before showing the window; this does not replace desktop-level QA.

Generic in-app GPU backdrop blur remains unimplemented. A native window material
or translucent panel does not blur content drawn inside the app. That requires
an explicit backdrop source, bounded cached targets and alpha-correct blur passes.

Window-only captures cannot certify desktop blur quality. Test light/dark modes,
repeated source switches, desktop contrast and Reduce Transparency on a real Mac.
See [GPU encoding](GPU_RENDERING.md) and [validation](VALIDATION.md).

### Wheel scrolling through an empty overlay

Layout-only layers in an overlapping composition can opt out of being the initial
wheel target. This is explicit, not a change to default hit routing:

```rust
use volt_ui_kit::ui::prelude::*;
let ui: Element<()> = zstack![
    vstack![text("Scrollable content").height(420.)].scroll(Scroll::Vertical),
    spacer().scroll_passthrough(true),
].height(180.).align(Align::Stretch).into();
ui.build()?;
# Ok::<(), volt_ui_kit::declarative::DocumentError>(())
```

Apply it to an empty overlay's container **and** any spacer covering the scroll
view. Descendants remain eligible wheel targets; buttons/keyboard input and
painting are unchanged. Do not apply it to opaque overlays that should block
scrolling behind them. Volt Desk uses it for its empty pane-placement layer.
