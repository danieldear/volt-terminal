# Architecture

## Layers

```text
 your app: state, view(&App), callbacks
            │
 ui          Rust builders and typed callbacks              (feature rust-ui)
            │
 declarative validated tree · layout · focus · input ·        (feature declarative)
            semantics · animation · optional JSON frontend
            │
 paint      Painter / DocumentPainter traits, component      (always available)
            recipes, theme, icons, SVG backend
            │
 native     winit window · Vello GPU or tiny-skia drawing ·   (feature native, native-gpu)
            cosmic-text · AccessKit · clipboard/IME ·
            menus, files, notifications
```

Each layer is optional above the one below it. With no features, the crate is
a small library of drawing recipes and geometry that depends only on
`unicode-width`, so an existing renderer can embed it. `rust-ui` adds the
builders and runtime without a window; `native` adds a complete host.

## A frame

1. Input arrives (pointer, key, text, scroll, a background message or command).
2. The runtime routes it to the topmost enabled target and runs your callback.
3. If a callback ran, `view(&App)` builds a new tree. It is validated and
   installed in one step, keeping focus, scroll and animation by key.
4. Layout measures and places the tree, reusing measurements from unchanged
   nodes. Text shaping is cached.
5. Painting produces ordered drawing batches, which Vello encodes for the GPU
   (or tiny-skia rasterizes, redrawing only damaged regions).
6. The window presents the frame and goes back to waiting. Nothing runs while
   the window is idle.

Hover, scrolling, resizing and animation skip step 3.

## Who owns what

| Owner | Responsibilities |
| --- | --- |
| Your application | Domain state, persistence, networking, async work, commands |
| The toolkit | Layout, drawing, input routing, focus, accessibility semantics, validation |
| The native host | Window and event loop, GPU and font resources, clipboard and IME, OS services |
| Documents (JSON) | Data only: no scripts, imports, expressions or file access |

## Accessibility

The native host publishes the interface to the platform's assistive
technologies through [AccessKit](https://accesskit.dev): roles, names, values,
states and focus for every control, and actions such as press, focus, set value
and scroll. `.accessible_label(..)` overrides an element's spoken name.
Requests from assistive technology go through the same checks as input
(enabled, mounted, inside the active modal). The tree is covered by automated
tests; live sessions with VoiceOver, Narrator and Orca are still to do.

## Security and resource boundaries

- The crate is `#![forbid(unsafe_code)]`. Platform bridges use safe bindings
  (`objc2`, `muda`, `tray-icon`) that contain native FFI internally.
- Every tree is validated before installation: sizes, colours, text length,
  depth and node count are bounded (see [limits](building-uis.md#validation-and-limits)).
- Images: PNG/JPEG up to 16 MiB encoded, 4096 px per side and 128 MiB decoded.
  SVG icons and paths are parsed with size limits and never run scripts or load
  external resources. Fonts: 64 files, 256 MiB in total.
- Accessibility requests are bounded (text up to 8 KiB, finite numbers only).
- File helpers refuse symlinks and non-regular files and never overwrite
  silently, but they are not a sandbox: do not treat them as a security boundary
  for untrusted paths.
- `WindowBackground::CompositorBlur` uses a private macOS API and is opt-in.
- Callbacks are your code. The toolkit does not sandbox them.

## Dependencies and Rust version

- The minimum Rust version is 1.89, set by Vello 0.11. Vello (0.11) and wgpu
  (30) are pinned to exact versions because GPU output is validated against them.
- wgpu is built with only the backend each platform uses (Metal, DX12 or
  Vulkan); Vello's default features are off.
- `cargo audit` reports no vulnerabilities. It lists unmaintained-crate
  warnings for `rustybuzz` and `ttf-parser` (through cosmic-text), which have
  no fixed versions yet.

## Embedding the drawing layer

Without any features, you can draw the component recipes into your own
renderer by implementing `Painter`. The bundled SVG painter shows the shape of
that API:

```rust
use volt_ui_kit::svg::SvgPainter;
use volt_ui_kit::{Button, ButtonColors, CardIcon, Painter, Viewport};

let mut painter = SvgPainter::new(Viewport::new(480., 160., 1.).unwrap()).unwrap();
let (mut shapes, mut glyphs) = (vec![], vec![]);
let button = Button { label: "Apply", icon: Some(CardIcon::Check), enabled: true };
let rect = [24., 24., 112., 32.];
button.paint(&mut painter, &mut shapes, &mut glyphs, rect, 1., ButtonColors {
    surface: [0.87, 0.70, 0.40, 1.],
    text: [0.09, 0.10, 0.10, 1.],
});
assert!(button.hit(rect, 50., 40.));
let svg = painter.document(&shapes, &glyphs, [0.09, 0.10, 0.10, 1.]);
assert!(svg.contains("Apply"));
```

`cargo run --example gallery` renders every recipe this way into
`target/gallery/`. These recipes came from the Volt terminal, which still uses
them through this layer.
