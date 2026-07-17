<div align="center">
  <img src="assets/icon.svg" width="96" height="96" alt="Volt icon" />
  <h1>Volt</h1>
  <p>A fast, GPU-accelerated terminal emulator for macOS — built in Rust.</p>
</div>

<br/>

> ⚠️ **Early development.** Volt is functional and daily-driver ready on macOS, but APIs and config formats may change before v1.0.

---

## Features

- **GPU-accelerated rendering** — Metal-backed via `wgpu`, smooth at any size
- **VTE-compliant** — Full ANSI/xterm escape sequence support via the `vte` crate
- **Tabs + pane splits** — `Cmd+T` new tab, `Cmd+D` vertical split, `Cmd+Shift+D` horizontal split
- **Full mouse support** — click, scroll, drag selection
- **Alternate screen buffer** — vim, htop, etc. work correctly
- **Live config reload** — edit `volt.toml`, press `Cmd+Shift+R`
- **Built-in themes** — Catppuccin, Tokyo Night, Gruvbox, Nord, Dracula
- **Nerd Font support** — powerline / icon glyphs with automatic fallback
- **macOS CVDisplayLink** — tear-free rendering locked to display refresh rate
- **Transparent + blur** — compositor effects on macOS

---

## Platform support

| Platform | Status |
|----------|--------|
| macOS 13+ | ✅ Primary target |
| Linux | 🚧 Compiles, UI layer in progress |
| Windows | ❌ Not planned |

---

## Building from source

### Prerequisites

- Rust 1.75+ (`rustup update stable`)
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Linux: `libudev-dev` and a Wayland/X11 compositor

```bash
git clone https://github.com/your-username/volt-terminal.git
cd volt-terminal
cargo build --release
./target/release/volt
```

### Release builds

Pushing a `v*` tag runs the release workflow. It builds and uploads:

- a Linux release binary tarball
- a macOS `Volt.app` zip archive
- `SHA256SUMS.txt`

The macOS archive is ad-hoc signed for bundle integrity, but not Developer ID signed or notarized yet.

---

## Configuration

Volt reads `~/.config/volt/volt.toml` (created automatically on first launch).  
Press **`Cmd+,`** to open it in your default editor. Press **`Cmd+Shift+R`** to reload without restarting.

```toml
# Volt Terminal — Configuration

# Options: catppuccin  tokyo-night  gruvbox  nord  dracula
theme = "catppuccin"

[font]
family = "JetBrainsMono Nerd Font Mono"
size = 14.0

[shell]
program = "/bin/zsh"
args = ["-l"]

[appearance]
padding = 8
line_height = 1.4
opacity = 1.0
cursor_style = "block"   # block | underline | beam
cursor_blink = true
```

---

## Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+T` | New tab |
| `Cmd+W` | Close tab |
| `Cmd+D` | Split pane vertically |
| `Cmd+Shift+D` | Split pane horizontally |
| `Cmd+Tab` / `Ctrl+Tab` | Next tab |
| `Cmd+Shift+Tab` | Previous tab |
| `Cmd+1`…`9` | Switch to tab N |
| `Cmd+C` | Copy selection |
| `Cmd+V` | Paste |
| `Cmd+,` | Open config in editor |
| `Cmd+Shift+R` | Reload config |

---

## Workspace layout

```
volt/           # binary entry point
volt-core/      # PTY management, VTE parser, terminal grid
volt-renderer/  # wgpu/Metal GPU renderer, glyph atlas, pipeline
volt-ui/        # macOS window, event loop, tab/pane management
volt-config/    # config parsing, themes
```

---

## License

[MIT](LICENSE)
