# Search

Open with **Cmd+F** (Cmd+Shift+P also works), **Edit → Find…** or **View →
Search…**. One panel searches this terminal's output across the full
scrollback, and the project's files, text, Git branches, worktrees and tasks.
A one-line selection becomes the query. This is a modal floating
panel, not another terminal pane. It reserves no terminal columns or rows and
works above a shell or a full-screen TUI such as nvim. It does not depend on the
workspace inspector being visible.

```
+--------------------------------------------------------+
| (search) invoice_                          ~/code/atlas |
|--------------------------------------------------------|
| [All]  Terminal  Files  Text  Git  Tasks               |
|--------------------------------------------------------|
|>File    src/lib/invoice.ts                             |
|         Modified 2 hours ago                           |
| Text    const invoice = await createInvoice(order)     |
|         src/routes/billing.ts:42                       |
| Output  PASS src/lib/invoice.test.ts                   |
|         Terminal output, line 812                      |
|--------------------------------------------------------|
| 4 results, searched locally              Enter to open |
+--------------------------------------------------------+
```

The panel is a compact list (at most 640 points wide) that grows with its
results, up to eight rows, and scrolls to keep the selection in view. It uses
fixed black-and-amber colors, independent of the terminal theme, and highlights
the matched characters in each result.

## Controls and behavior

- Type a query; Up/Down or the mouse wheel select results. Tab/Shift+Tab cycle
  scopes. Click a scope to filter, or click a result to select it.
- **Enter on a file or text result opens the file in your editor**, after
  re-checking that it is a regular file inside the searched project. The editor
  comes from `[editor]` in `config.toml` (per file type, then the default), else
  `$VISUAL` / `$EDITOR`. Volt types the command into the current terminal tab,
  so a text match opens at its line (`nvim +42 'src/main.rs'`). The file path is
  shell-quoted, and paths with control characters are refused. This is refused
  in a read-only pane or while a full-screen app is running; half-typed input is
  cleared first (Ctrl-Y restores it). With no editor configured, the file opens
  in the system's default app (`open` / `xdg-open`) without a shell.
- For output results, Enter verifies the retained row is still unchanged, then
  hands off to terminal Find: every match of the query in this terminal is
  highlighted, positioned on the one picked, and Enter / Shift+Enter step to the
  next / previous match. It does not move nvim's cursor. Changed/stale rows
  require a query refresh instead of an unsafe jump.
- Branch, worktree and task results are informational. Search **never checks out
  a branch, executes a task, or types an editor command**.
- Esc, Cmd+F, or clicking outside dismisses the panel. The dismissing
  click is consumed, not delivered to the application below.
- Cmd+V pastes into the query. Cmd+A selects it all; typing replaces the selection. Cmd+Backspace clears it. Left/Right/Home/End edit
  the query cursor. Query length is capped at 256 characters.
- Switching active PTYs or changing the owned shell's local directory dismisses
  search. Reopening binds to the new scope. Late worker results cannot replace a
  newer query or directory.
- The search root, shown at the top right, is the enclosing Git checkout, so a
  search started in `crate/src` covers the whole workspace. Outside a checkout it
  is the closest project manifest, otherwise the current directory. The home
  directory and `/` are never walked: Files and Text ask you to `cd` into a
  project instead, while Terminal, Git and Tasks still work.

## Providers

| Scope | Matching | What is searched |
| --- | --- | --- |
| Terminal | Case-insensitive literal | The full scrollback and screen (up to 32 MiB of text), or the current full-screen app's grid. Every matching line is counted; the latest 60 are listed |
| Files | File-name substring, then path substring, then letters in the file name; scattered letters across folders only when close together | Local workspace-relative paths |
| Text | Case-insensitive literal, not regex | Saved UTF-8 text files |
| Git | Fuzzy subsequence | Local branches and worktree metadata, no network |
| Tasks | Fuzzy subsequence | Saved `.volt/tasks.toml` commands and unimported project suggestions. Enter opens the Tasks card for review; search never executes a command |

Results are grouped by provider, then ranked within each provider; text matches
stay in reading order (file by file, line by line). File/Text
search respects Git/ignore rules, excludes hidden files and common generated
folders (target, node_modules, build, dist, vendor, venv, etc.), and rejects
symlinks, binary text, and files larger than 1 MiB. Common key/credential filenames
are excluded as well; this is **not** a comprehensive secret detector. All search
data remains local and is not sent to AI services.

## Runtime boundaries

- 130 ms debounce; one worker per palette, generation-based cancellation.
- No directory walking or file reading on the rendering thread.
- Terminal snapshot uses a non-blocking lock and a 32 MiB budget; matching occurs
  after releasing the lock. A busy terminal reports that it needs a refresh.
- Filesystem walk: at most 10,000 regular-file candidates, depth 24, approximately
  16 MiB of source content, and a 2-second search budget. Caps are reported.
- At most 80 file, 100 text, and 60 output results (300 combined maximum).
- Local Git subprocesses use bounded output and a 3-second timeout each, with
  cancellation checked between the two commands. Slow filesystems/syscalls are
  not forcibly interrupted, but remain on the single background worker.
- Reading a file for Text search, and opening one with Enter, both revalidate
  regular-file type and workspace containment first.
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

Search tests cover Unicode ranking/editing, file-name ranking and highlight
ranges, whole-checkout roots from a subfolder, the home-directory guard,
reading-order text results, open-file containment, literal matching, ignores, generated
and common credential exclusions, binary/size/symlink/containment guards,
cancellation, stale query/directory rejection, and inert task definitions.
GPU checks span 1x/1.5x/2x scale and line heights 1.0/1.2/1.8: unchanged grid,
opaque search over both terminal and inspector, cache equivalence, clean hide,
and the Find handoff's compositing. Live macOS smoke tests use a separate preview
bundle, a shell, and `nvim --clean -n`; they are not a substitute for testing every
TUI or a cross-terminal performance benchmark.

### Local verification, 2026-10-05 (panel redesign)

- Workspace suite: **273 passed, 0 failed, 4 existing ignored**; strict Clippy
  (macOS and Linux targets) and formatting checks clean.
- GPU: 9 search/Find cases at 1x/1.5x/2x and three line heights, plus the
  inspector, theme editor and tab checks.
- Engine probed on this repository: file queries in ~4 ms, mixed scopes in
  50–90 ms; a search from `~` no longer walks the home directory.
- Not yet re-verified by hand in the native app after the redesign.

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
