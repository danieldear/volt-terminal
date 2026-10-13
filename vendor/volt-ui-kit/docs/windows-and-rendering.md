# Windows and rendering

## Configuring the window

`Application` holds the window settings. Everything is validated before the
window opens.

```rust,no_run
use volt_ui_kit::{
    native::{Application, RendererPreference, WindowBackground, WindowChrome},
    ui::prelude::*,
    ThemeMode,
};

#[derive(Default)]
struct App;

fn view(_: &App) -> Element<App> {
    vstack![
        hstack![text("My app").font_weight(600)]
            .height(60.)
            .padding(100., Edge::Left)
            .align(Align::Center)
            .drag_region(true),
        text("Content"),
    ]
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("My app", App, view)
        .size(1100, 760)
        .theme(ThemeMode::System)
        .chrome(WindowChrome::Overlay)
        .background(WindowBackground::Blur { opacity: 0.8 })
        .renderer(RendererPreference::Auto)
        .run()
}
```

## Title bar and chrome

| `WindowChrome` | Result |
| --- | --- |
| `Native` (default) | The platform's standard title bar. |
| `Overlay` | macOS: content fills the window under a transparent, unified title bar; the traffic lights stay native. Reserve about 60 pt of header and 100 pt on the left for them. Other platforms keep a native title bar. |
| `CompactOverlay` | The same with the shorter, older macOS title bar geometry. |
| `Borderless` | No system decorations; you supply the window controls. |

Mark your header with `.drag_region(true)` so the window can be dragged by it.
Buttons and other controls inside the header still receive clicks.

## Transparency and blur

| `WindowBackground` | Result |
| --- | --- |
| `Opaque` (default) | A solid window. |
| `Transparent { opacity }` | The window's background is see-through; content stays opaque. |
| `Blur { opacity }` | macOS: the public system material (`NSVisualEffectView`) blurs the desktop behind the window. Requires `native-gpu` and a decorated window. |
| `CompositorBlur { opacity }` | A stronger blur through winit's compositor request. On macOS this uses a private API, so it is not App Store safe. |

- `opacity` applies only to the window background. Text and controls keep their
  own opacity; give your root a translucent or no background to let the blur show.
- `.background_from_state(|app| WindowBackground::..)` changes the background
  from your state, and `UiTheme::with_background_color(..)` tints it.
- Blur is a request to the OS. Windows and X11 have no equivalent, and
  Reduce Transparency switches the macOS blur to an opaque background.
- The window blur shows what is behind the window. Blurring the app's own
  content behind a panel is not supported yet.

## GPU and software drawing

With `native-gpu`, each window draws through **Vello** on wgpu (Metal on macOS,
DX12 on Windows, Vulkan on Linux). Without it, or when the GPU cannot be used,
the host draws with **tiny-skia** on the CPU. Both produce the same layout,
text and hit areas.

| `RendererPreference` | Behaviour |
| --- | --- |
| `Auto` (default) | GPU when compiled and available; falls back to software on errors. |
| `GpuRequired` | Returns an error instead of falling back. |
| `Software` | Always draws on the CPU. |

The `VOLT_UI_RENDERER=auto|gpu|software` environment variable does the same for
apps that use `run_ui` instead of `Application`.

Why Vello? Metal, DX12 and Vulkan are the low-level graphics APIs; Vello is a
2D renderer that runs on top of them through wgpu. It turns paths, text,
gradients, clips and blends into GPU compute work, so the kit has one drawing
path on every platform instead of three hand-written ones.

What still runs on the CPU: your callbacks and `view`, layout, text shaping and
encoding the scene. Layout reuses the measurements of unchanged nodes, and text
shaping is cached.

### Large scenes

Vello works within fixed-size GPU buffers. The kit estimates each frame's cost
before drawing it, splits very large frames into up to 16 horizontal strips, and
draws a frame in software only when it cannot fit. Images are admitted up to
96 MiB of unique pixels per frame.

## Performance and diagnostics

- A settled window draws nothing and uses no timers. Animations request frames
  only while they run.
- `VOLT_UI_TRACE=1` prints per-frame timings (layout, paint, GPU encode and
  present, text cache misses), GPU scene statistics (shapes, glyphs, layers,
  estimated buffer use) and the font and surface setup. It never logs your
  application's text.
- Many examples accept `--bench N` and `--snapshot out.png --size 1200x800 --scale 2`
  for repeatable timing and headless screenshots.

```sh
VOLT_UI_TRACE=1 cargo run --release --features native-gpu --example catalog -- --page dashboard
```

Trace timings start when the host handles an event and end when the frame is
handed to the OS; they do not include display scan-out.

## Resource limits

| Resource | Limit |
| --- | --- |
| Window framebuffer | 64 megapixels |
| GPU image cache | 64 MiB |
| GPU glyph cache | 16 MiB / 4,096 glyphs |
| Opacity layer depth / clip depth | 24 / 48 |
| Shadow work per frame (software) | 16 megapixels; extra shadows are skipped and counted |

These bound the kit's own caches. GPU drivers, Vello's internal buffers and the
OS compositor use additional memory.
