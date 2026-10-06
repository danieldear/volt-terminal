<div align="center">
  <img src="assets/icon.svg" width="96" height="96" alt="Volt icon" />
  <h1>Volt</h1>
  <p>Stay in the terminal. Keep your project in view.</p>
</div>

<br/>

Volt keeps the shell at the center of your workflow. One search box covers
your scrollback and your project, your project's build, run and test commands
are one click away, and Git changes, branches and worktrees sit in a compact
workspace card beside the terminal or floating over it. GPU-rendered, written
in Rust, for macOS and Linux.

> ⚠️ **Early development.** Volt is functional on macOS, but it has not been validated as a daily driver; APIs and config formats may change before v1.0.

**Public preview (October 2026):** [Volt v0.1.9](https://github.com/danieldear/volt-terminal/releases/tag/v0.1.9)
is available with macOS and Linux archives and SHA-256 checksums. The macOS
archive is **ad-hoc signed, not Developer ID signed or notarized**. macOS
Gatekeeper may prevent it from opening; a notarized public build is planned,
not yet released. You can also [build from source](#building-from-source).

---

## Features

- **GPU-accelerated rendering** — Metal-backed via `wgpu`, smooth at any size
- **VTE-based parsing** — common ANSI/xterm sequences; advanced DCS and some OSC features remain unsupported
- **Tabs + pane splits** — a flat tab bar with color-tinted tabs, drag-to-reorder, split right/left/up/down, drag-resizable dividers
- **Full mouse support** — click, scroll, Shift-click extension, drag selection with edge auto-scroll through retained history, double-click word / triple-click line select
- **Native macOS menu bar** — File/Edit/View/Window menus and a dynamic Services submenu; keyboard shortcuts are handled by Volt, not menu-item accelerators
- **Native right-click context menu** — Copy/Paste, split in any direction, reset terminal, read-only toggle, rename tab/terminal, and choose a tab color
- **Project tasks** — one-click build/run/test commands per project in `.volt/tasks.toml`, run in the current tab with ✓ / ✗ status. Add them from a form or import suggestions from your project files; shared task files are reviewed before anything runs. [Details](#project-tasks)
- **Your editor** — set nvim, nano or any editor per file type in `[editor]`; search results open in it at the matching line, and `Cmd+,` opens the settings in it. [Details](#configuration)
- **Search** (`Cmd+F`) — one local search box for this terminal's full scrollback and the project's file names, file contents, terminal output, branches, worktrees and tasks, with matches highlighted as you type. Enter opens files in your editor, at the matching line, or jumps to matching output. [Scope and limits](docs/workspace-search.md).
- **In-terminal Find** — Enter on an output result highlights every match in the terminal; Enter / Shift+Enter step between them (capped at 100,000 matches)
- **Alternate screen buffer** — vim, htop, etc. work correctly
- **Config reload** — edit `config.toml`, press `Cmd+Shift+R`
- **Themes** — built-in Catppuccin, Tokyo Night, Gruvbox, Nord, Dracula, plus your own; switch from Volt ▸ Theme and edit colors live with **Customize Theme…** ([details](#themes))
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
| Linux (x86_64) | ✅ Release archives. Copy and paste, the menu bar and secure keyboard entry are macOS-only for now |
| Windows | ❌ Not planned |

---

## Security and privacy

On macOS, Volt enables **Secure Event Input** while a focused pane looks like a
canonical, no-echo password prompt. You can also toggle it manually from the
Volt menu for prompts that use other terminal modes. The lock indicator appears
only when macOS accepts the request. This reduces ordinary keyboard-event
eavesdropping by other apps; it **does not** authenticate the prompt, hide
terminal output, or protect clipboard contents. Treat unexpected password
prompts as untrusted.

Volt does not implement OSC 52 clipboard writes from terminal output. Pasting
is user-initiated and limited to 8 MiB per operation; oversized pastes are
rejected. Volt-created `config.toml` files use owner-only permissions on Unix
because configuration may contain an API key. Existing files are made
owner-only when Volt saves or changes the selected theme.

The public repository requires pull requests and passing CI checks on `main`,
including tests, a dependency audit, and a full-history secret scan. GitHub
secret scanning and push protection are enabled. These controls do not prove
that Volt is free of vulnerabilities. Please use [private vulnerability
reporting](SECURITY.md) rather than a public issue for security findings; see
the [publication review](docs/publication-security.md) for the audit's limits.

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

The public [v0.1.9 release](https://github.com/danieldear/volt-terminal/releases/tag/v0.1.9)
is ad-hoc signed on macOS, **not notarized**. The workflow defaults to that
mode until Apple credentials are configured; it never silently substitutes an
ad-hoc build for a requested notarized build. See [distribution setup](docs/distribution.md).
Release notes and `macos-signing-status.txt` identify the actual mode for each
release. Verify downloads against the accompanying `SHA256SUMS.txt`.

---

## Configuration

Volt reads `~/.config/volt/config.toml`. Press **`Cmd+,`** (or Volt ▸
Settings…) to open it in your `[editor]` (or the system's default app if none
is set) — the file is written with fully-commented defaults the first time you
do this. Press **`Cmd+Shift+R`**
(or Volt ▸ Reload Settings) to reload without restarting.

```toml
# Volt Terminal — Configuration

# Built-in: catppuccin  tokyo-night  gruvbox  nord  dracula
# Or the id (file name) of a theme in ~/.config/volt/themes/.
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

[editor]
# Opens files from search (at the matching line) and Volt ▸ Settings….
# Volt types the command into the current tab. Unset: $VISUAL / $EDITOR,
# else the system's default app.
command = "nvim"

[editor.filetypes]          # by extension or full file name
md = "nano"
```

Volt knows how nvim/vim, nano, micro, Emacs, Helix, VS Code, Zed and Sublime
take a line number. For another editor, put `{file}` and `{line}` in the
command, e.g. `command = "myedit --line {line} {file}"`.

### Themes

**Volt ▸ Theme** lists the built-in themes, then your own. Picking one
switches every window immediately and rewrites only the `theme = "…"` line
of `config.toml` (comments and other settings are kept).

**Volt ▸ Theme ▸ Customize Theme…** opens an editor panel over the top-right
of the window. The terminal behind it is the live preview.

- Six base colors (background, foreground, cursor, cursor text, selection,
  selection text) and the 16 ANSI colors. Click a row or swatch, or use
  `Tab`/`Shift+Tab` (or the arrow keys) to move between them.
- Type a hex color — the first digit replaces the shown value, and it
  applies as soon as six digits are entered. A red border means the text
  isn't a complete `#rrggbb` color yet. `Cmd+V` pastes a color,
  `Cmd+Backspace` clears the field.
- **Revert** goes back to the theme as it was opened (or last saved).
  `Esc` or **×** closes the editor and restores it.
- **Save** (`Cmd+S`) overwrites the user theme you're editing. Built-in
  themes can't be overwritten, so their button is **Save as…**, which asks
  for a name; `Cmd+Shift+S` always asks. Saving switches to the saved theme.

Saved themes are TOML files in `~/.config/volt/themes/` (**Open Themes
Folder** in the same menu). The file name is the theme's id — the value used
in `theme = "…"`:

```toml
# ~/.config/volt/themes/nord-custom.toml
name = "Nord Custom"
background = "#2e3440"
foreground = "#d8dee9"
cursor = "#d8dee9"
cursor_text = "#2e3440"
selection_background = "#4c566a"
selection_foreground = "#eceff4"
ansi = [
  "#3b4252", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#88c0d0", "#e5e9f0",
  "#4c566a", "#bf616a", "#a3be8c", "#ebcb8b", "#81a1c1", "#b48ead", "#8fbcbb", "#eceff4",
]
```

All fields are required and unknown fields are rejected. Ids are lowercase
letters, digits and dashes, and can't reuse a built-in id. Files that fail
these checks, symlinks, files over 64 KiB, and anything past the first 256
files are skipped; the reason shows in the alert bar. Themes are re-read on
Reload Settings and after each save.

### Project tasks

Give a project its own one-click commands (build, run, test, deploy) in
`.volt/tasks.toml` at the project root. Commit it to share the tasks with your team.

```toml
[[task]]
name = "Build"
run  = "cargo build --release"

[[task]]
name = "Deploy staging"
run  = "./scripts/deploy.sh --env staging"
cwd  = "infra"          # optional: a folder inside the project
confirm = true          # optional: show the command and ask before each run
```

**Where tasks appear**

- The workspace card (`Cmd+Shift+A`) ▸ **Tasks**: saved tasks first, then
  **Add task**, **Edit tasks file**, and a collapsed **Suggested tasks** list.
- While the card is closed, the same tasks are buttons at its top-right spot,
  hidden while a full-screen app such as nvim is running. A recognized project
  with no saved tasks shows just **+ Add task** there.
- On macOS, **View ▸ Project Tasks** and **View ▸ Add Task…** work whether or
  not the card is open. Search (`Cmd+F`) lists tasks too; Enter there opens
  the Tasks card for review and never runs a command.

**Running**

- A click types the command into the active pane of the current tab and
  presses Enter, so its output is right there and the keyboard stays with the
  terminal. Half-typed input is cleared first (Ctrl-Y brings it back). Outside
  the task's folder the command runs in a subshell, so your shell stays put.
- Volt won't type over something else: a command that's still running, a
  full-screen app, a password prompt, or a read-only pane blocks the task, and
  a short note says why.
- With shell integration enabled, each task shows running, ✓ or ✗ from its
  exit code. Without it, the task is marked **Sent** rather than claiming to run.
- Write `./program` for a compiled executable (not `zsh ./program`). Volt
  never adds `&` or runs tasks in the background.

**Adding and editing**

- **Add task** opens a form (name, command, folder, ask-before-running) and
  saves to `.volt/tasks.toml`, keeping any comments already in the file.
- **Edit tasks file** opens the file in your `[editor]` to change or delete tasks.
- **Suggested tasks** come from `Cargo.toml`, `package.json` scripts, `go.mod`,
  `Package.swift`, and pytest/CMake setups, read without executing those
  manifests. Select one to see its command, folder and source, then choose
  **Add to tasks file**. Importing never runs it, and imported tasks ask before
  each run. Commands already in the file aren't suggested again.

**Trust**

- A tasks file that arrives with a clone or a pull is someone else's commands.
  Its buttons show a shield: clicking one previews that command and its folder,
  Enter runs it **once**, and Escape cancels. That doesn't trust the file or
  its other tasks.
- For one-click runs, review every command in the card and choose **Trust these
  tasks**. Any change to the file invalidates trust and pending confirmations.
- Tasks you add in Volt to a new or already trusted file are trusted
  automatically. Adding a task never trusts commands you haven't reviewed.
- Each command must be a single line. Symlinked files or `.volt` folders,
  folders outside the project, and files over 64 KB are refused.

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
| `Cmd+Shift+Left` / `Cmd+Shift+Right` | Move active custom tab one position left / right |
| `Cmd+1`…`9` | Switch to tab N |
| `Cmd+Alt+Arrow` / `Ctrl+Alt+Arrow` | Move focus between panes |
| `Cmd+C` / `Ctrl+Shift+C` | Copy selection |
| `Cmd+V` / `Ctrl+Shift+V` | Paste |
| `Cmd+F` | Search terminal output and the project (selection prefills it) |
| `Cmd+=` / `Cmd+-` | Increase / decrease font size |
| `Cmd+Shift+A` | Toggle workspace card |
| `Cmd+Shift+P` | Same search panel (alias) |
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

Drag a tab by its title to rearrange custom tabs. A normal click selects the
tab on release; dragging an inactive tab leaves the current terminal active.
The keyboard move stops at the first/last tab rather than wrapping. Native
macOS tabs are managed by AppKit and are not reordered by these Volt commands.

### Custom shortcuts

Add `[[keybindings]]` entries to `~/.config/volt/config.toml`, then reload
with Cmd+Shift+R (or File → Reload Settings). These **override** matching
built-in shortcuts; other defaults stay intact. Example:

```toml
[[keybindings]]
key = "cmd+shift+l"
action = "toggle_workspace_layout"

[[keybindings]]
key = "cmd+shift+s"
action = "split_left"

# Disable a default and let the terminal's normal key encoder handle it:
[[keybindings]]
key = "cmd+d"
action = "unbind"
```

Use `ignore` instead of `unbind` to consume a key without sending it to the
shell. Adding a new shortcut does not remove the old one; unbind it explicitly.
Bindings are single physical-key chords, with **exact** modifiers; the last
entry for a chord wins. Keys use `a`–`z`, `0`–`9`, `f1`–`f24`, named arrows,
`enter`, `tab`, `escape`, `space`, `backspace`, `delete`, `home`, `end`,
`pageup`, `pagedown`, or punctuation names such as `equal`, `minus`,
`bracketleft`, `bracketright`, `comma`, `period`, `slash`, `backslash`,
`semicolon`, `quote`, `backquote`. Modifiers are `ctrl`, `alt`/`option`,
`shift`, `cmd`/`super`. For a shifted punctuation key, include `shift` explicitly.
OS-reserved shortcuts may never reach Volt.

Supported actions: `copy`, `paste`, `find`, `search_workspace`,
`toggle_workspace`, `toggle_workspace_layout`, `new_tab`, `new_window`,
`close_pane`, `next_tab`, `previous_tab`, `move_tab_left`, `move_tab_right`,
`previous_prompt`, `next_prompt`, `split_right`, `split_left`,
`split_down`, `split_up`, `focus_left`, `focus_right`, `focus_up`, `focus_down`,
`increase_font_size`, `decrease_font_size`, `toggle_fullscreen`, `reload_config`,
`open_config`, `customize_theme`, `quit`, `ignore`, `unbind`. `customize_theme`
has no default shortcut; bind it to open or close the theme editor.

Custom bindings do not override text editing in Find/title prompts, workspace
search, the theme editor, or a focused workspace card. Paste continues to respect read-only panes.
Invalid keys/actions and configurations with over 128 bindings are rejected.
Key sequences, key tables and arbitrary shell-command bindings are not supported.

### Links and working directories

**Cmd-click** (macOS) or **Ctrl-click** (Linux) opens visible plain HTTP(S)
URLs, including links soft-wrapped into scrollback. Explicit link clicks are
consumed by Volt even inside mouse-aware TUIs; ordinary clicks still reach the
TUI. No background URL scanning is performed. Schemes other than HTTP(S), URLs
with credentials/control characters, and oversized logical lines are rejected.
OSC 8 labeled links retain their destinations across wrapping, scrollback and
reflow. Modifier-clicking one opens a **read-only destination confirmation**:
Enter opens, Escape cancels, and Left/Right/Home/End inspect long URLs. Unsupported
explicit schemes do not fall back to the visible label. See [OSC 8 details and
limits](docs/osc8-links.md); run `sh scripts/validation/osc8-demo.sh` to test.

New tabs, splits and windows inherit the focused pane's **local shell directory**
on macOS/Linux. This uses the owned process's OS-reported CWD, not a `cd` command
injected into the shell and not a possibly remote OSC 7 path. If the process CWD
cannot be determined, the application's startup directory remains the fallback.
It does not recreate SSH sessions or their remote working directories.

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

With the custom tab bar, right-click a tab (including an inactive one) to
change its title or choose **Tab Color**. The color adds a narrow accent and
subtle tint; the running/idle dot remains separate. **No Color** clears it.
You can also choose Tab Color from the terminal context menu for the active
tab. Tab colors are session-local and are not shown by native macOS tabs.

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

### Prompt navigation (opt-in shell integration)

Source the bundled Zsh/Bash integration to record shell prompts. Fish 4.9.3
emits compatible OSC 133 markers by itself; do not source a Fish hook.
`Cmd+Shift+Up` / `Cmd+Shift+Down` navigate prompt history without sending input
to the shell. Custom actions: `previous_prompt` and `next_prompt`.
Anchors are bounded and discarded on resize; command-output selection is not
yet implemented. See [setup, compatibility and validation](docs/shell-integration.md).
