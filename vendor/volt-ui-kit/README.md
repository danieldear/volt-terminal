# Volt UI Kit

The cards, buttons, forms, palettes and vector icons behind Volt — extracted into a small, renderer-independent Rust library.

**Status:** working extraction, version 0.1.0, not published to crates.io. This is an embeddable toolkit foundation, not yet a complete desktop application framework.

## What is here

- A reusable icon-and-label `Button`, also used by Volt's Settings footer.
- Floating/docked/minimized cards, colored indicators, expandable rows and diagnostics.
- Settings/onboarding presentation, navigation geometry and footer controls.
- Action/task button strips and a three-field form with checkbox and status.
- Search palette presentation, Unicode match highlighting and pointer geometry.
- Tab-strip hit testing, reorder target geometry and accent colors.
- Theme-derived surfaces, outlines, text and semantic colors.
- Shared vector icon paths and rounded/line drawing primitives.
- A static SVG backend and standalone component gallery.
- Glass-inspired reusable `Panel` / surface tokens, brightness hysteresis, and clipped topmost hit routing.

The presentation models and paint recipes are real extracted implementations, not screenshots or HTML recreations of Volt. A few component names and default labels retain their Volt origins; use the customizable card footer and generic Button for application-independent UI. Generalizing the remaining labels is on the roadmap.

## Architecture

```text
Application state / native input / accessibility
                    |
Models + shared layout / hit testing
                    |
Widget paint recipes + theme + icons
                    |
Painter adapter (static dispatch)
          /                     \
Host GPU/text backend      SVG documentation backend
```

Implement `Painter` to emit triangles and shaped text into your own buffers. `Shape` and `Glyph` are host-selected types. The toolkit does not allocate a GPU device, start a window, run a timer, read files or execute commands. Volt's adapter reuses its existing wgpu device, glyph atlas, cosmic-text shaping, vertex buffers and per-widget geometry caches. Cache invalidation and event-driven redraw scheduling stay with the host.

Rendering coordinates are physical pixels; layout inputs include an explicit scale and optional top inset. Text uses a top-origin, 1.2-em convention. Colored text callbacks use UTF-8 **byte offsets**. Use a valid positive finite viewport; `Viewport::new` validates the basic dimensions and scale.

## Glass-inspired surfaces (optional)

Inputs from Glass Browser informed three new portable building blocks:

- `Panel`: general-purpose rounded frame, subtle top highlight and an opaque fallback.
- `SurfaceStyle` / `TintPicker`: smoked/frosted material tokens with lightness hysteresis. The **host** supplies brightness only when its backdrop changes; invalid samples are ignored.
- `HitMap<T>`: register targets in paint order, then route to the topmost visible target. Supports clipping, rounded corners and a `Block` target so a floating panel can consume clicks without activating content beneath it.

Existing Volt widgets keep their current theme and render path. Glass is **opt-in**, not a replacement theme. `Panel::paint` is an opaque fallback, not fake blur. A compositor-capable host can consume `backdrop_request()` then use `paint_composited_frame()`; it must implement backdrop capture, tint, blur, correct compositing order and caching itself. Requests use physical pixels; logical corner/blur radii are scaled once. `reduced_transparency()` disables blur and uses an opaque fill.

The source of these ideas is Glass Browser's `crates/ui/src/{theme,tint,view}.rs` and `crates/glass/src/lib.rs`. They are independently implemented here; browser-specific state, CEF, Vello, WGSL pipelines and source files were not copied into this crate. A real GPU glass backend, system accessibility preference detection and animation remain future work. The gallery demonstrates **opaque surface previews**, not backdrop compositing.

## Use it locally

```toml
[dependencies]
volt-ui-kit = { path = "/path/to/volt-ui-kit" }
```

```rust
use volt_ui_kit::{Button, ButtonColors, CardIcon, Painter, Viewport};
use volt_ui_kit::svg::SvgPainter;

let mut painter = SvgPainter::new(Viewport::new(480., 160., 1.).unwrap()).unwrap();
let (mut shapes, mut glyphs) = (vec![], vec![]);
let button = Button { label: "Apply", icon: Some(CardIcon::Check), enabled: true };
let rect = [24., 24., 112., 32.];
button.paint(&mut painter, &mut shapes, &mut glyphs, rect, 1., ButtonColors {
    surface: [0.87, 0.70, 0.40, 1.], text: [0.09, 0.10, 0.10, 1.],
});
assert!(button.hit(rect, 50., 40.));
let svg = painter.document(&shapes, &glyphs, [0.09, 0.10, 0.10, 1.]);
assert!(svg.contains("Apply"));
```

## Component gallery

```sh
cargo run --locked --example gallery
# Open target/gallery/index.html in a browser.

cargo run --locked --example gallery -- --light --output target/gallery-light
cargo run --locked --example gallery -- --scale 2 --output target/gallery-2x
```

The gallery is **static SVG documentation**, not a native interactive application. It runs without Volt, a terminal engine, a windowing dependency or a GPU. Browser font metrics/rasterization may differ from Volt's native text backend; Volt's existing GPU fixtures are the visual regression authority.

## Develop and validate

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo +1.87.0 check --locked --all-targets
cargo package --locked
```

Rust 1.87 or newer. Only runtime dependency: `unicode-width`. The library forbids unsafe code. SVG text is escaped; it never loads assets or executes embedded scripts. A host must still implement secure input, accessibility and safe application actions.

## Volt integration and source ownership

This project is the canonical editable source. Volt consumes a **reproducible vendored snapshot**, so its public checkout and CI remain self-contained before this library has a remote release.

```sh
python3 scripts/sync_into_volt.py --volt ../terminal-public
python3 scripts/sync_into_volt.py --volt ../terminal-public --check
```

The sync tool only updates `vendor/volt-ui-kit`; it refuses an unrecognized nonempty destination. `SOURCE.json` records version and hashes, with no local machine paths. Volt's Python validation verifies that snapshot's file inventory and content. Do not edit the vendored copy independently. After publishing a stable library version, Volt can switch to a normal pinned registry dependency.

The original UI implementations in Volt are now compatibility re-exports and cache/backend adapters, not a second copy of widget painting logic. PTY/parser/grid, task execution/trust, Git/project discovery, config persistence and AppKit integration stay in Volt.

## License

MIT, preserving the original Volt copyright notice.
