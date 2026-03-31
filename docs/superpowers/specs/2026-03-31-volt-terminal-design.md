# Volt Terminal — Design Spec
_Date: 2026-03-31_

## Overview

Volt is a GPU-accelerated terminal emulator for macOS and Linux. It is fast, modern, and built around three ideas that no existing terminal does well together:

1. **Terminal-owned statusline** — context (cwd, git, AI session) lives in the terminal, not the shell prompt
2. **First-class AI integration** — multiple AI providers (Claude, Copilot, Codex) authenticated via OAuth, with shared session deduplication and MCP as the tool layer
3. **True multitasking** — per-pane isolation with independent PTY, statusline, AI session, and MCP context

Primary inspiration: Ghostty (speed, native feel), Kitty (powerline tabs), Warp (AI integration), Zed (provider auth model), Zellij (plugin architecture).

---

## Crate Architecture

Rust workspace. Each crate has one responsibility and communicates via async channels — no shared mutable state across crate boundaries.

```
volt/
├── volt-core        # PTY, VTE parsing, grid state
├── volt-renderer    # wgpu GPU rendering, font/glyph rasterization
├── volt-ui          # winit windowing, tab bar, pane layout, event loop, statusline
├── volt-ai          # AI providers, OAuth, MCP client, session registry
├── volt-workflow    # workflow engine: step execution, branching, sharing
├── volt-config      # TOML config, theme engine, keybindings
└── volt             # binary entrypoint
```

### `volt-core`
- Spawns PTY via `portable-pty`
- Feeds bytes through `vte` parser
- Maintains per-pane grid (cell = character + fg/bg + attributes)
- Emits events: `GridUpdated`, `CwdChanged`, `TitleChanged`, `CommandFinished { exit_code, duration_ms }`
- Watches process cwd: `proc_pidinfo` on macOS, `/proc/<pid>/cwd` on Linux

### `volt-renderer`
- Single wgpu render loop for all panes in a window
- Metal backend on macOS, Vulkan/GL on Linux (via wgpu automatic selection)
- `cosmic-text` for font shaping, ligatures, glyph rasterization
- Shared glyph atlas across all panes
- Nerd Font and powerline glyphs are first-class
- Only redraws dirty regions — target <2ms per frame
- Renders: pane grids, tab bar, per-pane statuslines, AI sidebar panels

### `volt-ui`
- `winit` for native windowing on macOS and Linux
- **Tab bar** (top): kitty-style powerline segments, per-tab state (title, active indicator, AI provider badge)
- **Pane layout manager**: split horizontal/vertical, resize, focus management
- **Per-pane statusline** (bottom of each pane): independent state, async updates
- **AI sidebar**: slides in alongside pane output, per-pane, shows conversation for that pane's session
- **Settings panel**: UI over `volt-config`, toggle via keybind

### `volt-ai`
- Provider abstraction trait — each provider implements: `authenticate()`, `complete()`, `stream()`
- OAuth auth flows per provider (see Auth Flow section)
- **Session registry** — keyed by `(repo_root, provider)`, deduplicates sessions across panes sharing the same cwd/repo
- Session **fork**: user can explicitly fork a shared session into an isolated copy
- MCP client layer (see MCP section)
- Inline suggestion engine: intercepts shell input, queries active provider, renders ghost text

### `volt-config`
- Single source of truth: `~/.config/volt/config.toml`
- Live reload on file change
- Theme engine: loads `.toml` theme files from `~/.config/volt/themes/`
- Keybinding system
- Statusline segment configuration
- MCP server configuration (global + per-project via `.volt.toml` in repo root)

---

## Layout

```
┌─────────────────────────────────────────────────────────┐
│  ❯ tab1   ❯ tab2   ❯ tab3                               │  ← tab bar (powerline style)
├──────────────────────────┬──────────────────────────────┤
│                          │                              │
│   pane 1 output          │   pane 2 output              │
│                          │                              │
├──────────────────────────┼──────────────────────────────┤
│  ~/agent007  main [...]  │  ~/other-project  feat       │  ← per-pane statusline
└──────────────────────────┴──────────────────────────────┘
```

When AI sidebar is open for a pane:
```
├──────────────────────────┬──────────────────────────────┤
│                          │  pane 2 output   │ AI panel  │
│   pane 1 output          │                  │ (Claude)  │
│                          │                  │ ...       │
├──────────────────────────┼──────────────────┴───────────┤
│  ~/agent007  main [...]  │  ~/other-project  feat  ◉Claude│
└──────────────────────────┴──────────────────────────────┘
```

---

## Statusline

The statusline is **terminal-owned**, not shell-owned. It works the same regardless of which shell is running (zsh, bash, fish).

**Data sources (all async, push-based):**
- `cwd` — from `volt-core` `CwdChanged` events
- `git_branch`, `git_status` — lightweight git watcher on repo root, debounced, runs on change only
- `ai_session` — from `volt-ai` session registry
- `mcp_servers` — active server count from MCP layer
- `exit_code`, `exec_time` — from `volt-core` `CommandFinished` events

**Configuration:**
```toml
[statusline]
enabled = true
segments = ["cwd", "git_branch", "git_status", "exec_time", "exit_code", "ai_session", "ai_session_id", "mcp_servers"]

[[statusline.custom_segments]]
name = "k8s_context"
command = "kubectl config current-context"
interval_ms = 5000
position = "right"
```

**Shell prompt** becomes minimal — just `❯` with exit code color. All context lives in the statusline.

---

## AI Integration

### Providers
- Claude (Anthropic)
- GitHub Copilot
- OpenAI / Codex
- Cursor

### Auth Flow (per provider)
All tokens stored in OS keychain (`keyring` crate — macOS Keychain / Linux Secret Service).

**Browser OAuth (Claude, OpenAI, Cursor):**
1. User triggers "Connect [Provider]" in settings
2. Volt spawns local HTTP server on random port
3. Opens browser to provider OAuth URL with localhost callback
4. Provider redirects back → token captured
5. Token stored in keychain, session registry notified

**Device flow (GitHub Copilot):**
1. Volt requests device code from GitHub
2. Displays code + URL to user in settings panel
3. User confirms on github.com
4. Volt polls for token, stores in keychain

Token refresh handled transparently in background.

### Session Registry
Keyed by `(repo_root, provider)`. Multiple panes in the same repo share one underlying AI session — same context, no redundant token usage. Each session gets a short human-readable ID (e.g. `volt-a3f2`) displayed in the pane's statusline.

```
pane 1 (~/agent007)  ──┐
pane 2 (~/agent007)  ──┼──► AI Session [volt-a3f2] (Claude, context = agent007 repo)
pane 3 (~/agent007)  ──┘

pane 4 (~/other)     ──────► AI Session [volt-b91c] (Codex, context = other repo)
```

**Session joining behavior:**
- New pane/tab in the same repo → auto-joins the existing session for that repo + provider
- User can manually join any session by ID from the AI sidebar (type `> join volt-a3f2`) — enables cross-repo collaboration or joining a session from a pane in a different folder
- User can explicitly fork a session to get an isolated copy (new ID, cloned context)
- Session ID shown in statusline as a clickable segment — clicking opens the join/fork menu

### Inline Suggestions
- `volt-ui` intercepts shell input line before sending to PTY
- Sends `InlineSuggestRequest { input, cwd, history }` to `volt-ai` over async channel
- `volt-ai` streams token response back as `InlineSuggestChunk` events
- `volt-renderer` renders ghost text (dimmed) ahead of cursor, updated as chunks arrive
- Tab sends accepted text to PTY as if typed; Esc clears ghost text and resumes normal input
- Suggestion is cancelled automatically if user types before response completes

### AI Sidebar
- Per-pane, slides in alongside pane output
- Full conversation UI with the active provider
- Pane's cwd, git state, and MCP tools automatically included as context
- Provider badge shown in that pane's statusline

---

## MCP Layer

Volt is an **MCP host**. Each pane's AI session has an MCP client that connects to configured MCP servers.

**Global config (`~/.config/volt/config.toml`):**
```toml
[[mcp.servers]]
name = "filesystem"
command = "/usr/local/bin/mcp-filesystem"
args = ["--root", "/home/user"]
auto_connect = true
```

**Per-project config (`.volt.toml` in repo root):**
```toml
[[mcp.servers]]
name = "github"
command = "mcp-github"
auto_connect = true
```

Any MCP server the community builds (Kubernetes, Docker, AWS, databases) works automatically. The AI provider receives MCP tools as part of its context — no Volt-specific integration needed.

---

## Plugin / Scripting System

Three tiers, increasing integration depth:

| Tier | How | Use case |
|------|-----|----------|
| Shell / binary | Configure `command`, Volt spawns + reads stdout | Quick statusline segments, hooks, any language |
| WASM plugin | Compile to `.wasm`, get full Volt API access, sandboxed | Rich plugins needing pane state / AI / events |
| Native dylib | Compile Rust as `.so`, loaded directly | Maximum performance, no sandbox |

Most users use tier 1 only. WASM and dylib are for plugin authors.

**Event hooks:**
```toml
[[hooks]]
on = "pane_created"
command = "~/.config/volt/hooks/pane-created.sh"

[[hooks]]
on = "command_finished"
command = "~/.config/volt/hooks/notify.sh"
```

Plugin API surface (WASM/dylib): read pane state, write to statusline segments, listen to events, call MCP tools, access AI session.

_Full plugin API spec to be designed in a future iteration._

---

## Theme System

Themes are `.toml` files in `~/.config/volt/themes/`. Ships with: Tokyo Night, Catppuccin, Gruvbox, Nord, Dracula.

Each theme defines:
- 16 ANSI base colors + 256 extended
- Background, foreground, cursor, selection
- Tab bar colors + powerline segment style (`angle` | `round` | `slanted`)
- Statusline colors per segment

Active theme set in config:
```toml
[theme]
name = "tokyo-night"
```

---

## Config Overview

`~/.config/volt/config.toml` — single source of truth, live reload on change. Settings UI panel is a visual editor over this file.

Key sections: `[theme]`, `[font]`, `[keybindings]`, `[statusline]`, `[[statusline.custom_segments]]`, `[[mcp.servers]]`, `[[hooks]]`, `[ai]`.

Per-project overrides via `.volt.toml` in repo root (MCP servers, AI provider preference).

---

## Platform Notes

- **macOS**: Metal via wgpu, `kqueue` for file watching, `proc_pidinfo` for cwd tracking, macOS Keychain for secrets
- **Linux**: Vulkan/OpenGL via wgpu, `inotify` for file watching, `/proc/<pid>/cwd` for cwd tracking, Secret Service for secrets
- **Windows**: Not supported

---

## Intelligence Features

### Command Failure Intelligence
When a command exits non-zero, the statusline shows a `?` indicator. One keypress opens the AI sidebar with the error already loaded — explanation, root cause, and the exact fix command suggested. No copy-pasting. The terminal already captured the output.

### Semantic Command History
History is indexed semantically, not just as strings. Search by meaning across all sessions: *"that docker command I used to debug the network issue last week"*. Persisted per-project or globally. Powered by the active AI provider.

### Runbook Generation
After a series of commands, ask the AI to summarize the session into a runbook/markdown doc. The AI has full session context — output, exit codes, timing. Exported to a file or committed to the repo.

### Destructive Command Guard
AI intercepts commands that are potentially destructive — `rm -rf`, `git push --force` to main, `DROP TABLE` — and presents a warning with explanation of consequences before execution. Opt-in, on by default.

### Persistent AI Memory Per Project
Each project's AI session accumulates a knowledge file (`.volt-memory.md` in repo root or `~/.config/volt/memory/<repo>.md`). Volt auto-primes new sessions with it. The AI remembers conventions, past decisions, recurring issues. Gets smarter over time.

### Resource Statusline Segments
Built-in CPU/memory segments scoped to the process running in the current pane — not system-wide. Shows resource usage inline without needing a separate monitor tool.

### Smart Notifications
Long-running command finishes in a background tab → native OS notification (macOS `NSUserNotification`, Linux `libnotify`) with exit code and duration. Works even when Volt is not the focused app.

### Collaborative Sessions
Share a pane's session with another person over the network — they see your terminal output live and share the same AI session context. Built-in, no ngrok needed. Useful for pair debugging and remote assistance.

---

## Workflow Engine (`volt-workflow`)

A new crate — `volt-workflow` — handles definition, execution, and sharing of multi-step automated workflows. A workflow is a directed graph of steps that runs in a pane, with AI steps as first-class citizens alongside shell commands.

### Step Types

| Type | Description |
|------|-------------|
| `command` | Run a shell command in the pane's PTY |
| `ai_task` | Send a prompt + previous step output to the AI session |
| `wait` | Pause for user signal (e.g. Ctrl+C to stop a long-running process) |
| `loop` | Jump back to a named step |

### Data Flow
Each step's stdout/AI output is passed as context to the next step. Steps can reference previous output via `$step.N.output`.

### Branching

**Condition-based** (on exit code):
```toml
[[workflow.steps]]
id = "build"
type = "command"
command = "cargo build --release"
on_success = "install"
on_failure = "bug_analysis"
```

**AI-evaluated** (AI decides next step based on its output):
```toml
[[workflow.steps]]
id = "bug_analysis"
type = "ai_task"
prompt = "Analyze the build failure and determine severity"
ai_branch = [
  { condition = "critical bug found", next = "generate_bug_report" },
  { condition = "no bug found",       next = "restart" },
]
```

### Example: Android Debug Loop
```toml
name = "android-debug-loop"
description = "Capture logs, analyze, fix bug, rebuild, repeat"

[[workflow.steps]]
id = "capture_logs"
type = "command"
command = "adb logcat"
stop_on = "ctrl_c"
on_stop = "analyze_logs"

[[workflow.steps]]
id = "analyze_logs"
type = "ai_task"
prompt = "Analyze these Android logs for errors and root causes"
on_success = "generate_report"

[[workflow.steps]]
id = "generate_report"
type = "ai_task"
prompt = "Generate a structured bug report from the analysis"
on_success = "update_memory"

[[workflow.steps]]
id = "update_memory"
type = "ai_task"
prompt = "Update project memory with new findings"
on_success = "gap_analysis"

[[workflow.steps]]
id = "gap_analysis"
type = "ai_task"
prompt = "Identify gaps and missing test coverage"
on_success = "fix_bug"

[[workflow.steps]]
id = "fix_bug"
type = "ai_task"
prompt = "Fix the identified bug in the codebase"
on_success = "build"

[[workflow.steps]]
id = "build"
type = "command"
command = "./gradlew assembleDebug"
on_success = "install"
on_failure = "fix_bug"

[[workflow.steps]]
id = "install"
type = "command"
command = "adb install app/build/outputs/apk/debug/app-debug.apk"
on_success = "capture_logs"
```

### Workflow UI
- Visual workflow editor in the settings panel — drag steps, connect arrows, set conditions
- Active workflow shows progress inline in the pane: current step highlighted, completed steps checked
- Pause/resume/cancel controls
- Step output collapsible in pane history

### Sharing Workflows
Workflows are portable `.volt-workflow.toml` files:
- **Export**: save any workflow to a file from the UI
- **Import**: load from a local file or URL
- **Community marketplace** (future): a hub where users share workflows, discoverable and installable directly from Volt

---

## What Volt Is Not

- Not a shell replacement (no block system like Warp) — respects your existing shell
- Not opinionated about your workflow — everything is opt-in via config
- Not cloud-dependent — AI providers are your own accounts, no Volt cloud service
