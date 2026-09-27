# Workspace search (initial release)

Open with **Cmd+Shift+P**, or **View → Search Workspace**. This is a modal floating
palette, not another terminal pane. It reserves no terminal columns or rows and
works above a shell or a full-screen TUI such as nvim. It does not depend on the
workspace inspector being visible.

```
+---------------------------------------------------------------+
| SEARCH WORKSPACE             /path/to/project             [x]  |
| query |                                                       |
| All    Output    Files    Text    Git    Tasks                  |
|---------------------------------------------------------------|
| FILE    src/workspace_search.rs                                |
|         Saved file - read-only preview                         |
| TEXT    fn search_workspace()                                  |
|         src/main.rs:42                                         |
|---------------------------------------------------------------|
| Selected result preview                                       |
| Local results / truncation status                             |
| Up/Down select   Tab scope   Enter inspect   Esc close          |
+---------------------------------------------------------------+
```

## Controls and behavior

- Type a query; Up/Down or the mouse wheel select results. Tab/Shift+Tab cycle
  scopes. Click a scope to filter, or click a result to select/preview it.
- Enter previews saved files or inspects branch/worktree/task metadata. It
  **never checks out a branch, executes a task, or types an editor command**.
- For output results, Enter verifies the retained row is still unchanged, then
  hands off to terminal Find highlighting/scrolling. It does not move nvim's
  cursor. Changed/stale rows require a query refresh instead of an unsafe jump.
- Esc, Cmd+Shift+P, the close control, or clicking outside dismisses the palette.
  The dismissing click is consumed, not delivered to the application below.
- Cmd+V pastes into the query. Cmd+A selects it all; typing replaces the selection. Cmd+Backspace clears it. Left/Right/Home/End edit
  the query cursor. Query length is capped at 256 characters.
- Switching active PTYs or changing the owned shell's local directory dismisses
  search. Reopening binds to the new scope. Late worker results cannot replace a
  newer query or directory.
- The header identifies the actual search root: closest discovered project
  manifest (subject to inspector scope), otherwise the current local directory.

## Providers

| Scope | Matching | What is searched |
| --- | --- | --- |
| Output | Case-insensitive literal | Last 2,000 retained rows, or current alternate-screen TUI grid |
| Files | Case-insensitive fuzzy subsequence | Local workspace-relative paths |
| Text | Case-insensitive literal, not regex | Saved UTF-8 text files |
| Git | Fuzzy subsequence | Local branches and worktree metadata, no network |
| Tasks | Fuzzy subsequence | Discovered project task definitions; review only |

Results are grouped by provider, then ranked within each provider. File/Text
search respects Git/ignore rules, excludes hidden files and common generated
folders (target, node_modules, build, dist, vendor, venv, etc.), and rejects
symlinks, binary text, and files larger than 1 MiB. Common key/credential filenames
are excluded as well; this is **not** a comprehensive secret detector. All search
and preview data remains local and is not sent to AI services.

## Runtime boundaries

- 130 ms debounce; one worker per palette, generation-based cancellation.
- No directory walking or file reading on the rendering thread.
- Terminal snapshot uses a non-blocking lock and a 2 MiB budget; matching occurs
  after releasing the lock. A busy terminal reports that it needs a refresh.
- Filesystem walk: at most 10,000 regular-file candidates, depth 24, approximately
  16 MiB of source content, and a 2-second search budget. Caps are reported.
- At most 80 file, 100 text, and 60 output results (300 combined maximum).
- Local Git subprocesses use bounded output and a 3-second timeout each, with
  cancellation checked between the two commands. Slow filesystems/syscalls are
  not forcibly interrupted, but remain on the single background worker.
- File previews revalidate regular-file type and workspace containment before
  reading. Preview is a bounded excerpt, not a full editor.
- GPU palette geometry is cached; separate compositing keeps terminal and
  inspector glyphs behind the palette. Search does no polling/work when closed.

## Explicitly not included

Tree-sitter/LSP symbol indexes, semantic search, multiline output matching, remote
SSH filesystem search, and unsaved nvim buffer access are not implemented.
Terminal output search sees displayed cells, not the application's internal
buffer. Hidden MCP/rules files remain in the inspector's scoped configuration
view, not in broad text search. Task execution stays in the inspector's explicit
review/run flow.

## Validation

```
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --release --locked -p volt-renderer --example searchcheck
cargo run --release --locked -p volt-renderer --example cardcheck
cargo run --release --locked -p volt-renderer --example rendercheck -- target/search-rendercheck
bash scripts/macos/build_app.sh --release
```

Search tests cover Unicode ranking/editing, literal matching, ignores, generated
and common credential exclusions, binary/size/symlink/containment guards,
cancellation, stale query/directory rejection, and inert task definitions.
GPU checks span 1x/1.5x/2x scale and line heights 1.0/1.2/1.8: unchanged grid,
opaque search over both terminal and inspector, cache equivalence, clean hide,
and the Find handoff's compositing. Live macOS smoke tests use a separate preview
bundle, a shell, and `nvim --clean -n`; they are not a substitute for testing every
TUI or a cross-terminal performance benchmark.

### Local verification, 2026-09-26

- Workspace suite: **170 passed, 0 failed, 3 existing ignored**; strict Clippy and
  formatting checks clean.
- GPU suites: **9 search/Find + 27 inspector + 87 terminal = 123 cases passed**.
- Release bundle built and ad-hoc signature verified. Test-only launch at
  `target/inspector-preview/VoltSearchReady.app`; installed Volt was not replaced.
- Native UI: keyboard/scope clicks, fuzzy paths, source snippets, local branch and
  worktree discovery, inert task review, output Find handoff, select-all query
  replacement, and alpha → beta directory isolation verified.
- Live nvim (`--clean -n`, mouse reporting enabled): after search and close-click,
  `&modified` was 0, saved dimensions still matched, and cursor remained (1,1).
- Clipboard automation reported a timeout waiting for the application itself to
  consume the clipboard (Volt reads it through `pbpaste`); visual inspection
  confirmed the pasted query appeared in search and nvim remained unmodified.
