# Contextual workspace inspector

Toggle with **View > Toggle Workspace Panel** / **Cmd+Shift+A**. Enablement is
separate from visibility: ordinary folders have no card; relevant project/Git/rules
context brings it back. Explicitly closing/toggling off disables discovery.

## Docked and floating

Set the persistent preference in Volt's settings (Cmd+,), then reload settings:

```toml
[workspace]
layout = "floating" # or "docked" (default)
```

The header pin (beside minimize/refresh/close) toggles **pinned = docked** and
**unpinned = floating**. The pinned icon is blue; floating uses a tilted outline.
It works expanded or collapsed and remembers the last floating position. The
collapsed header keeps the colored status dot to leave room for all four controls.

The pin and **View > Switch Workspace: Docked / Floating** change the current window only;
it does not rewrite the settings file. In floating mode, drag the left portion of
the header. Position is retained per window in memory, clamped below window chrome
and inside the drawable surface, and uses logical pixels across scale changes.
It is not restored across app restarts. Floating never reserves terminal columns
or rows: nvim and other TUIs keep the full grid. Moving/collapsing the card does
not resize the PTY. It intentionally covers underlying content; Cmd+Shift+A hides
it. Pointer/wheel events on the card do not leak into the TUI. On very small
windows the card auto-hides. Switching modes may resize the PTY once.

Docked reserves the right gutter; collapse only hides the body without moving the
prompt. Closing or automatic hiding returns that gutter to the terminal.

The GPU submits inspector backgrounds/text **after** terminal glyphs, so the
floating card actually occludes text. Geometry remains cached between updates.

## Project discovery and tasks

Discovery walks at most 16 ancestors, bounded by the current Git worktree (or
home boundary), and picks the nearest project manifest. The enclosing worktree
root remains separate from the active subproject root.

- Rust: Cargo manifests, build/test tasks.
- Python: pyproject/requirements/setup.py markers; uv/Poetry lockfile badges;
  pytest task only when a pytest configuration is present. No setup.py execution.
- C/C++: CMake/configuration/compilation-database markers. Configure is explicit;
  build task appears for an existing build/CMakeCache.txt. No implicit configure.
- JavaScript/TypeScript: package.json dependencies, framework/tool badges (Vite,
  React, Vue, Next.js, Svelte), real named scripts. Package-manager declarations
  and ancestor lockfiles choose npm/pnpm/yarn/bun. No invented test scripts.
- Go: go.mod/go.work; build/test for module roots.
- Swift: Package.swift; SwiftPM build/test. Discovery never evaluates the manifest.
- Make: marker only; dynamic targets are not guessed.
- Bounded immediate source-file sampling identifies standalone language folders.

Project disclosure shows detected evidence and task scope. In **Tasks**, saved
commands come first. **Suggested tasks** stays collapsed until opened; select
one to inspect its command, directory, and source, then choose **Add to tasks
file**. Import does not run it. The command appears among saved tasks (and
asks before running), and exact duplicates stop appearing as suggestions.
**Add task**, **Edit tasks file**, and trust review share the same section.
Imported arguments are shell-quoted when written to `.volt/tasks.toml`.
Build/package scripts are still executable project code: only run reviewed
tasks. Saved tasks are typed into the active pane of the current tab, with
visible output and keyboard focus returned to the terminal. They are not
launched in a background runner. A foreground command, full-screen app,
read-only pane, or detected password prompt blocks dispatch. A compact notice
explains why; it is not another task. Shell integration supplies start/exit
status; without it, the task is marked **Sent**, not falsely **Running**.
Use `./program` for a compiled executable, not `zsh ./program`. Shell scripts
may use their interpreter explicitly. A task command containing `&` can itself
request background execution; Volt does not add it.

Compact task buttons run trusted tasks directly (`confirm = true` still asks).
A shield marks untrusted tasks: clicking opens a small read-only command
preview, including the working directory, rather than opening the workspace
card. Enter runs **only that command once**; Escape cancels. This does not
trust the file or its other tasks. File-wide **Trust these tasks** remains in
the card after reviewing all commands. Editing the file or switching panes
invalidates a pending confirmation.

## Git, worktrees, GitHub

Changes use NUL-delimited status and include staged/unstaged/untracked files.
Line totals compare HEAD to working tree; untracked/binary counts are not invented.
Branches show current branch, upstream, ahead/behind against **local refs**, and
an expandable local branch list (bounded to 64). No automatic fetch or checkout.

Worktrees appear when multiple checkouts exist (bounded to 32), with branch/path,
current/locked/prunable states. Clicking an available worktree opens a new shell
tab there. This does not create, delete, prune, or switch branches.

For github.com remotes with `gh` installed, a separate worker uses existing gh
credentials and gh's current-branch resolver, including its fork/default-repository
handling. PR title/state, draft status, review decision, and up to 30 checks are
shown. Open on GitHub accepts only validated github.com PR URLs. Missing PRs do
not create an empty card section; query failures are shown inside Branches and
are never treated as passing checks. Refresh is at most once per minute unless
explicitly refreshed or branch/commit/scope changes. No login, push, PR creation,
merge, or check rerun occurs. GitHub Enterprise/custom hosts are not supported yet.

## Rules, configuration and MCP

Read-only discovery follows the active directory up to its project/worktree root:
AGENTS.md, CLAUDE.md, .rules (file or directory), .claude/rules, .cursor/rules,
.codex/config.toml, .claude/settings.json, and .mcp.json. Files have bounded reads;
symlink escapes outside the root are excluded. The disclosure shows source path
and scope, not a claim about which agent has loaded or overridden which rule.

Rule text has a bounded in-card preview. Tool-config values are withheld;
.mcp.json exposes server **names**, not commands, URLs, tokens, headers, or env.
Malformed JSON is labelled, not launched. Configured does not mean connected.
Nothing is sent to an AI service or automatically staged in Git.

Machine-wide Ollama/LM Studio installation does not make a directory relevant.
Provider status requires a future real session adapter. AI session transports,
actual sent context/attachments, token usage, runtime MCP state, Tree-sitter
syntax previews, language-server semantics, and debugger variables are **not**
implemented by this inspector iteration.

## Responsiveness and verification

Local Git/project discovery runs off-thread at five-second intervals (15 collapsed).
Owned-shell cwd checks run independently every 250 ms while enabled. Scope changes
clear stale data; late worker results are discarded. At most one local-discovery
and one GitHub worker run per window. No work runs in the terminal render loop.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --release --locked -p volt-renderer --example cardcheck
cargo run --release --locked -p volt-renderer --example rendercheck -- target/workspace-features-rendercheck
bash scripts/macos/build_app.sh --release
```

Tests cover project scope, real script extraction, non-execution of configs,
argument injection, MCP value hiding, actual temporary Git worktrees, gh response
states, mode settings, clamped hit geometry, full-grid floating, text occlusion,
cache equivalence, collapse stability, and clean hiding at multiple scales and
line heights. Custom GPU controls do not yet expose native accessibility elements;
keyboard selection is supported. Header glyphs are 11.2 pt with larger hit targets.

The idle-loop regression fix, measured throughput comparison, and optional CPU
budget gate are documented in [workspace performance](workspace-performance.md).
