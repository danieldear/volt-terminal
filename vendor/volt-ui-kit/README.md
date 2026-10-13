# Volt UI Kit

Native desktop interfaces in plain Rust, drawn on the GPU.

Write a function from your state to a tree of elements. Volt UI Kit lays it
out, draws it with [Vello](https://github.com/linebender/vello) on Metal, DX12
or Vulkan (with a CPU fallback), handles input, focus and accessibility, and
calls your closures when something happens. There's no markup language, no
webview and no runtime to embed.

![The Catalog's analytics dashboard in dark mode](docs/images/dashboard-dark.png)

```rust
use volt_ui_kit::{native::Application, ui::prelude::*};

#[derive(Default)]
struct App {
    count: u32,
}

fn view(app: &App) -> Element<App> {
    vstack![
        text(format!("Clicked {} times", app.count)).font_size(24.),
        button("Click me")
            .key("add")
            .variant(ButtonVariant::Primary)
            .on_click(|app: &mut App| app.count += 1),
    ]
    .padding(24., Edge::All)
    .gap(12., Axis::Vertical)
    .into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Application::new("Counter", App::default(), view).run()
}
```

## Highlights

- **Plain Rust.** Views are functions and callbacks are closures over your own
  state type. Conditionals, loops and reusable components are ordinary Rust.
- **70+ components.** Buttons, fields, selects, date pickers, menus, dialogs,
  command palettes, tables, trees, virtual lists, split panes, toasts and
  interactive charts.
- **Themes you define.** Every colour role is yours to set, from code or a JSON
  theme file that reloads live, with contrast checks built in. Bring any SVG
  icon set.
- **GPU first.** Vello renders on the GPU through wgpu, falls back to the CPU
  automatically, and draws nothing while the window is idle.
- **Desktop integration.** Window chrome and blur, native menus and tray, file
  panels, notifications, commands with shortcuts, close guards and `.app`
  bundles (macOS first).
- **Accessible.** Roles, labels, values and actions go to screen readers
  through AccessKit, with full keyboard navigation.
- **A fast dev loop.** `cargo volt dev` rebuilds on save and restarts the app
  with its state and window position intact.

![The Theme editor in light mode](docs/images/theme-editor-light.png)

## Try it

```sh
git clone https://github.com/danieldear/volt-ui-kit
cd volt-ui-kit
cargo run --release --features native-gpu,native-desktop --example catalog
```

The Catalog shows every component with live previews, source code and API
tables. Add `--page dashboard`, `--theme light` or `--accent violet`.

## Use it

The crate is not on crates.io yet; depend on it from Git:

```toml
[dependencies]
volt-ui-kit = { git = "https://github.com/danieldear/volt-ui-kit", features = ["native-gpu"] }
```

Or start from a template:

```sh
cargo install --git https://github.com/danieldear/volt-ui-kit --features dev-tools --bin cargo-volt
cargo volt new my-app --kit-path /path/to/volt-ui-kit --template navigation
cargo volt dev my-app
```

| Feature | Adds |
| --- | --- |
| `native` | A window with software drawing, text, images, clipboard, IME and accessibility |
| `native-gpu` | GPU drawing with Vello (implies `native`) |
| `native-desktop` | macOS menus, tray and file panels |
| `native-notifications` | macOS local notifications |
| `rust-ui` / `declarative` | The builders and runtime without a window, for custom hosts |
| `dev-tools` | The `cargo volt` command |

Rust 1.89 or newer.

## Status

Version 0.4, pre-release. macOS is the primary platform and is tested on Apple
silicon. Windows and Linux build and pass the headless test suite in CI, but
their native runtime has not been validated yet. The core compiles to
WebAssembly; a web host is planned. Expect API changes before 1.0.

## Documentation

- [Getting started](docs/getting-started.md)
- [Building interfaces](docs/building-uis.md)
- [Components](docs/components.md)
- [Styling and themes](docs/styling-and-themes.md)
- [Windows and rendering](docs/windows-and-rendering.md)
- [Application services](docs/app-services.md)
- [JSON interfaces](docs/json-ui.md)
- [Architecture](docs/architecture.md)
- [Examples](examples/README.md) · [Roadmap](ROADMAP.md) · [Changelog](CHANGELOG.md) · [Contributing](CONTRIBUTING.md)

## License

MIT. See [LICENSE](LICENSE).
