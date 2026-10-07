# Settings and first-run setup

Implemented in current source; not yet part of the published v0.1.10 archive.

## First launch

With no configuration file, Volt opens a four-step introduction:

1. Welcome and privacy boundaries.
2. Theme, installed monospaced font, font size, spacing and transparency.
3. Tabs/panes, Settings/search, project tasks and the workspace card.
4. Shell, arguments, bundled Meow or the existing prompt, shell integration.

Every step can be skipped. Skip retains the original preferences and records
completion. Existing configuration files do not trigger onboarding or opt into
Meow. Help → Getting Started reopens the introduction. Fresh setup suggests
Meow and integration only when the current shell is Bash, Zsh or Fish.
Finishing opens a new ready-to-use tab; the original tab is retained so startup
jobs cannot accidentally be terminated.

## Settings

`Cmd+,` on macOS / `Ctrl+,` on Linux opens the centered, GPU-rendered panel.
The native menu opens the same surface. No browser or webview is involved.

```
+-------------------+-------------------------------------+
| Settings          |                                     |
| Appearance        | Theme       catppuccin         -  + |
| Shell & prompt    | Font        SF Mono            -  + |
| Terminal          | Font size   14                 -  + |
| Workspace         |                                     |
| Tasks             | Live terminal preview               |
| Shortcuts         |                                     |
| Security          | Validation / focused-control hint   |
| Advanced          |                                     |
| Open config       | Reset page     Cancel       Apply   |
+-------------------+-------------------------------------+
```

- Click a section. `PageUp`/`PageDown` also changes sections.
- `Tab`/`Shift+Tab` and up/down move through controls, scrolling the visible
  rows as needed. The header shows the visible range. Scroll wheel also moves
  focus. This keeps all controls accessible in the default-size window.
- Minus/plus or left/right adjust choices and numeric values. Click/Enter to
  edit a value directly; typing replaces the selected value. Home/End and
  left/right move the text caret. Command-A selects all; copy/paste is supported
  on macOS. Enter confirms. Invalid input stays editable with an explanation.
- In Shortcuts, Enter starts recording; press the desired physical chord.
  Conflicting custom assignments are rejected. Delete removes that action's
  override. Reset page removes all custom bindings. Custom bindings take
  precedence over built-in defaults; defaults remain available unless explicitly
  unbound in TOML. Cmd+Q remains available to quit a modal panel.
- Apply saves; Command-S / Control-S also saves. Escape exits editing first,
  then cancels the panel. Cancel restores the opening appearance.
- Reset page changes only that page's draft. Apply is still required.
- Apply before opening the separate theme editor or task editor; opening an
  external surface closes the settings draft. The existing theme editor is
  reused rather than replaced.
- Shell and startup-card choices are explicit. The workspace card stays closed
  by default; normal terminal use does not start workspace discovery.
- Advanced configuration, native-tab mode and per-file editor mappings remain
  available in TOML. Native-tab mode requires restarting Volt.

## Saving safely

Existing comments, unknown keys and credential-bearing `[ai]` data are retained.
The settings UI does not display/edit credentials or read terminal contents.
Missing files are initialized with explicit settings, including shell arguments,
so subsequent launches do not lose the selected login behavior. Files are written
atomically with owner-only permissions. Managed file symlinks are preserved.

Malformed files are not overwritten. External changes detected since opening
Settings cause saving to fail with a reopen instruction. Reopening reads the
current file. This protects ordinary concurrent edits, but is not a locking
protocol for other applications. Config files must be regular and at most 1 MiB.

History truncation occurs only on Apply, not during a preview. Shell changes
apply only to new panes/tabs; existing jobs are never restarted.

## Meow and shells

The bundled prompt is a small adaptation of Meow, retaining its colored chevrons, username, directory, local clock, Git branch,
staged/dirty/ahead/behind indicators and success/failure colors. Plain Unicode
is used rather than requiring private-use Nerd Font icons. Elapsed-command
timing is not currently included in the bundled adaptation. No framework, network requests
or agent account is required. Existing/custom prompts are the default for
existing users. Git metadata is sanitized and limited; the optional Git status
probe has a short timeout and omits untracked-file traversal.

App-scoped private startup wrappers load normal user configuration, then install
the chosen prompt/hooks. No project script is automatically sourced. Bash/Zsh/
Fish support login/interactive arguments only in automatic setup mode. Other
shells and custom arguments remain available with Existing + integration off.

- Zsh restores the user's effective ZDOTDIR after startup, including changes
  from startup files, so child shells and logout use the user's configuration.
- Bash uses an interactive rc wrapper. For `-l`, it sources `/etc/profile` and
  the first readable personal login profile. It does **not** make that Bash
  process a true login shell (`shopt login_shell` remains false); choose Existing
  with automatic setup off if exact login-shell semantics or logout hooks are
  required. Existing prompt-command arrays are retained on Bash 5.1+.
- Fish loads its normal configuration first. Fish 4+ supplies native OSC 133
  signals; older Fish gets lightweight event hooks when integration is selected.
  See [Fish's native shell-integration notes](https://fishshell.com/blog/new-in-40/).
- macOS Bash 3.2 reports prompt/end status but lacks modern PS0 start markers;
  foreground-job detection still provides the terminal's busy safeguard.

Meow runs only while preparing a shell prompt, not per output byte or rendered
frame. The Settings panel caches its geometry and adds no recurring timer.

## Validation

```
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +1.87.0 check --locked --workspace
cargo run --locked --release -p volt-renderer --example settingscheck
bash scripts/macos/build_app.sh --release
```

Automated checks cover config preservation/conflicts/symlinks/private permissions,
fresh-file round trips, invalid edits, Unicode text editing, shortcut recording,
startup-file preservation and real-PTY failure signals for installed supported
shells. GPU checks compare cached/fresh pixels, hiding, four themes and three
scales. Native smoke testing uses an isolated HOME/XDG configuration.

Screen-reader exposure of custom GPU controls still needs the product-wide
accessibility work tracked in the terminal execution plan. Passing these tests
is not proof of universal shell compatibility or unchanged termbench throughput.
