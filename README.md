<div align="center">
  <img src="assets/icon.svg" width="96" height="96" alt="Volt icon" />
  <h1>Volt</h1>
  <p>A fast, GPU-accelerated terminal emulator for macOS — built in Rust.</p>
</div>

<br/>

> ⚠️ **Early development.** Volt is functional on macOS, but it has not been validated as a daily driver; APIs and config formats may change before v1.0.

---

## Features

- **GPU-accelerated rendering** — Metal-backed via `wgpu`, smooth at any size
- **VTE-based parsing** — common ANSI/xterm sequences; advanced DCS and some OSC features remain unsupported
- **Tabs + pane splits** — split right/left/up/down, drag-resizable dividers
- **Full mouse support** — click, scroll, drag selection, double-click word / triple-click line select
- **Native macOS menu bar** — File/Edit/View/Window menus and a dynamic Services submenu; keyboard shortcuts are handled by Volt, not menu-item accelerators
- **Native right-click context menu** — Copy/Paste, split in any direction, reset terminal, read-only toggle, rename tab/terminal
- **Workspace Search** (`Cmd+Shift+P`) — floating local search for fuzzy file paths, saved text, retained output/current TUI screen, branches, worktrees, and task previews. [Scope and limits](docs/workspace-search.md).
- **In-terminal Find** (`Cmd+F`) — search scrollback + visible output, jump between matches (navigation is capped at 100,000 matches)
- **Alternate screen buffer** — vim, htop, etc. work correctly
- **Config reload** — edit `config.toml`, press `Cmd+Shift+R`
- **Built-in themes** — Catppuccin, Tokyo Night, Gruvbox, Nord, Dracula
- **Nerd Font support** — powerline / icon glyphs with automatic fallback
- **macOS CVDisplayLink** — tear-free rendering locked to display refresh rate
- **Transparent + blur** — compositor effects on macOS

Ask Siri and live spell-check suggestions, which macOS appends automatically
to real `NSTextView` context menus, aren't reproduced in the right-click
menu — Volt's terminal grid is custom GPU-rendered, not an `NSTextView`, so
the OS never offers them here. Everything else in a native context menu,
including the dynamic Services submenu, is genuine.

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

- Rust 1.87+ (`rustup update stable`)
- macOS: Xcode Command Line Tools (`xcode-select --install`)
- Linux: `libudev-dev` and a Wayland/X11 compositor

```bash
git clone https://github.com/danieldear/volt-terminal.git
cd volt-terminal
cargo build --release
./target/release/volt
```

### Release builds

Pushing a `v*` tag runs the release workflow. It builds and uploads:

- a Linux release binary tarball
- a macOS `Volt.app` zip archive
- `SHA256SUMS.txt`

By default the macOS archive is ad-hoc signed. The release workflow also supports
credential-gated Developer ID signing and notarization; see [distribution setup](docs/distribution.md).
Release notes and `macos-signing-status.txt` identify the actual mode. Existing
v0.1.2 downloads remain ad-hoc signed.

---

## Configuration

Volt reads `~/.config/volt/config.toml`. Press **`Cmd+,`** (or Volt ▸
Settings…) to open it in your default editor — the file is written with
fully-commented defaults the first time you do this. Press **`Cmd+Shift+R`**
(or Volt ▸ Reload Settings) to reload without restarting.

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

Volt handles the shortcuts below directly. The menu bar exposes common
commands, but not every shortcut or its key-equivalent hint.

| Shortcut | Action |
|----------|--------|
| `Cmd+N` | New window |
| `Cmd+T` | New tab |
| `Cmd+Shift+W` | Close window |
| `Cmd+W` | Close tab / pane |
| `Cmd+D` | Split pane vertically (right) |
| `Cmd+Shift+D` | Split pane horizontally (down) |
| `Cmd+Tab` / `Ctrl+Tab` | Next tab |
| `Cmd+Shift+Tab` | Previous tab |
| `Cmd+Shift+[` / `Cmd+Shift+]` | Previous / next tab |
| `Cmd+1`…`9` | Switch to tab N |
| `Cmd+Alt+Arrow` / `Ctrl+Alt+Arrow` | Move focus between panes |
| `Cmd+C` / `Ctrl+Shift+C` | Copy selection |
| `Cmd+V` / `Ctrl+Shift+V` | Paste |
| `Cmd+F` | Find in terminal |
| `Cmd+=` / `Cmd+-` | Increase / decrease font size |
| `Cmd+Shift+A` | Toggle workspace card |
| `Cmd+Shift+P` | Open workspace search |
| `Enter` / `Shift+Enter` (Find open) | Next / previous terminal match |
| `Ctrl+Cmd+F` | Toggle full screen |
| `Cmd+K` | Clear screen and scrollback |
| `Cmd+,` | Open config in editor |
| `Cmd+Shift+R` | Reload config |
| `Cmd+Q` | Quit |

Use **View → Switch Workspace: Docked / Floating** or the card's pin button
to change its layout (no dedicated keyboard shortcut).

Double-clicking empty tab-bar chrome (not a tab) maximizes/restores the
window, matching Finder/Safari.

### Right-click context menu

Right-click a pane for Copy/Paste, Split Right/Left/Down/Up,
Reset Terminal (full VT reset — clears scrollback, alt-screen, and all
attributes), Toggle Terminal Inspector (a small debug overlay: grid size,
cursor position, scrollback length), Terminal Read-only (blocks keyboard/
mouse input to that pane until toggled off — shown with a badge while
active), and Change Tab Title…/Change Terminal Title… (overrides the
OSC-reported title until cleared). When a terminal app has enabled mouse
reporting, ordinary right-click goes to that app; use **Shift+right-click**
for Volt's context menu.

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

## Validation

- [Terminal hardening plan and results](docs/terminal-hardening-plan.md)
- [Rendering validation](docs/rendering-validation.md)
- [Performance experiments and measurement limits](docs/performance-experiments.md)

---

## License

[MIT](LICENSE)
