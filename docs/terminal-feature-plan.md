# Terminal feature plan (themes explicitly excluded)

Goal: close the terminal gaps identified after v0.1.3 without sacrificing
throughput, input responsiveness, correctness, or the existing workspace tools.
Themes and theme UI are owned by the separate Claude work and are out of scope.

## Delivery order and acceptance gates

1. **Configurable shortcuts** — validated, reloadable action bindings; default
   shortcuts remain available; explicit unbind and safe modal routing; no parser
   or renderer hot-path work. Test invalid config, exact modifiers, overrides,
   read-only routing, and command leakage into Find/workspace search.
2. **Clickable links** — modifier-click plain HTTP(S) links, then OSC 8 metadata
   that survives wrapping, erasure, scrolling and reflow. Never execute URLs as
   shell commands. Reject unsafe schemes/control bytes, expose destinations,
   and preserve the 16-byte Cell layout. Test Unicode cell hit mapping.
3. **Shell integration** — inherited working directories, prompt/command marks,
   jump to prompts, command-output selection, and bounded Bash/Zsh/Fish hooks.
   Preserve existing startup files/hooks and exit codes. Do not execute text
   from a pane to discover its working directory; remote paths are not local.
4. **Synchronized output** — DEC 2026 negotiation, atomic displayed snapshots,
   timeout release, resize/close recovery, no stale redraw on cursor blink.
5. **Kitty keyboard protocol** — negotiated progressive flags, bounded per-screen
   stacks, key/shift/alternate reporting and repeat/release handling. Preserve
   legacy input when disabled; test IME, dead keys, application shortcuts/TUIs.
6. **OSC 52** — bounded clipboard payloads with deny/ask/allow policies and
   window/pane attribution; deny reads by default, never silently disclose the
   clipboard to terminal output. Test malformed base64, spam, stale panes.
7. **Layout restoration** — opt-in atomic/versioned state; windows, tabs, splits,
   ratios and validated local CWDs only. Never restore executable commands,
   terminal output, passwords or running processes. Recover corrupt state.
8. **Programming ligatures** — shaped runs with cell-span mapping, cursor and
   selection clipping, grapheme-safe fallback, no ASCII parsing penalty. Keep
   cached/full GPU equality at multiple DPI/font/line-height combinations.
9. **Kitty graphics** — protocol/resource design first; bounded decode/upload,
   placement, deletion, scroll/reflow rules and capability negotiation. Reject
   untrusted file paths, decompression bombs and unbounded allocations.

Secondary items: quick terminal, secure keyboard entry, automation, accessibility,
IME/platform validation, and Linux desktop parity need separately scoped work.
They must not be described as delivered by the phases above.

## Performance contract

- Keep `Cell` at 16 bytes and keep the ASCII/SGR parser free of new I/O, per-cell
  heap allocation or background discovery.
- Parse settings at load/reload, not on every keystroke. Link detection is on
  explicit interaction, not in the print/render loop.
- Save a pre-change release throughput executable; run alternating baseline/new
  repeats on identical deterministic data and grid dimensions. Investigate >5%
  median throughput loss; do not hide a regression behind an average win.
- Run workspace tests, strict Clippy, Rust 1.87 checks, offscreen terminal/card/
  search GPU fixtures, and the opt-in idle CPU gate.
- Parser microbenchmarks are NOT whole-app TermMark or input/presentation latency.
  GUI benchmarking needs the same display/grid/config and a quiet machine.
- No feature is complete on a stub, advertised-but-unsupported escape response,
  or build pass alone. Track source/unit/GPU/native-UI/benchmark evidence separately.

## Status

### Batch 1: implemented; native UI acceptance pending

- Validated TOML single-chord action bindings, compiled on load/reload. Exact
  modifier matching, last override wins, explicit `ignore` and `unbind`, modal
  routing and read-only paste guard preserved. No arbitrary-command action.
- On-demand Cmd-click/Ctrl-click for plain HTTP(S) links, with bounded Unicode
  cell mapping through soft-wrapped scrollback, safe-scheme validation, and no
  shell interpolation. This is not OSC 8 support or automatic hover detection.
- New tabs, splits and windows inherit the owned local shell's OS-reported CWD.
  Real PTY tests verify child directory inheritance without changing parent CWD.
  Remote OSC 7 paths are deliberately not used for local process launches.
- Reusable alternating parser-throughput regression gate in
  `scripts/validation/compare_throughput.py`, including malformed-result tests.

### Remaining implementation (not advertised as complete)

- Phase 1 extensions: sequences/key tables and the rest of the legacy actions.
  Single-chord action overrides are implemented; this is not full Ghostty binding parity.
- Phase 2 native acceptance: manual modifier-click/confirmation/browser checks.
  OSC 8 implementation and core/GPU validation are complete; hover grouping is not implemented.
- Phase 3: prompt navigation and opt-in hooks implemented in batch 3. Output
  selection, anchor-preserving reflow and broader shell acceptance remain.
- Phases 4–9: synchronized output, Kitty keyboard, permissioned OSC 52,
  restoration, ligatures and Kitty graphics.
- Secondary features listed above remain separately scoped.

### Validation of batch 1 (2026-09-27)

- Workspace: 195 tests passed, 4 opt-in tests ignored.
- Strict Clippy, formatting, Rust 1.87 workspace/all-target compilation passed.
- GPU: 60 cursor + 27 terminal + 27 card + 9 search cases passed (123 total).
- Release app built and ad-hoc signed; signature verification passed.
- Parser benchmark: seven alternating baseline/new repeats, 256 MiB per case,
  91×16 cells, 10,000-line history, both ordinary and wide-seeded grids.
  Median throughput deltas range from **−1.02% to +0.64%**; all eight cases pass
  the 5% regression budget. No printing/parsing/rendering hot path was changed.
- Native UI smoke test could not proceed because the Mac was locked. The isolated
  test app was stopped; existing user sessions were not terminated. Keyboard
  handling, external browser launch, and tab/split inheritance still need hands-on
  acceptance. The foreground idle CPU gate and full-app TermMark remain unrun.

See `docs/terminal-foundations-validation.md` for reproducible commands and limits.
This batch does **not** complete the entire feature roadmap.


### Batch 2: OSC 8 implementation and validation

- Bounded labeled-link metadata through existing extended-cell IDs; Cell remains
  16 bytes. Wrapping, reflow, erasure, scrollback, cross-grid copies and text-pool
  compaction retain the correct destination without linking identical plain text.
- Read-only destination confirmation (Enter opens, Escape cancels); long URLs page
  with cursor keys. Only HTTP(S), no shell interpolation, and no fallback to a
  deceptive visible label when an explicit destination is unsupported.
- Link tables allocate lazily; URLs are fingerprinted once per OSC command.
  Render-only snapshots strip metadata and restore scalar text, preserving the
  fast scalar/procedural-symbol rendering paths.
- 209 Rust tests passed (4 opt-in ignored), strict Clippy and Rust 1.87 checks
  passed. 132 GPU cases passed, including additional linked/plain pixel equality
  and long-destination bounds checks. Release app built and ad-hoc signature verified.
- Native rendering and custom F6 Find/search of linked labels were verified in an
  isolated instance. Foreground idle CPU: 0.73% of one core over 30 seconds; 1% gate
  passed. Full native modifier-click/confirmation/browser acceptance is still
  pending (the current UI automation API does not expose modifier-click).
- Final parser throughput (seven alternating repeats): -3.26% to +0.62%
  against batch 1. All cases passed the 5% gate; there is a small measured ASCII
  cost, not a proven zero-overhead result. Full-app TermMark remains unverified.
  Raw measurements and rejected experiments are recorded in `docs/osc8-links.md`.

Next implementation batch is shell integration and command/prompt navigation.
The protocol/restoration/ligature/graphics phases are still not delivered.

### Batch 3: prompt navigation and opt-in shell hooks

- Main-screen OSC 133 prompt marks, command start/completion timing, previous/next
  prompt actions and Cmd+Shift+Up/Down. No per-character or renderer changes.
- Sparse lazy 1024-anchor cap, constant-time scroll origin tracking and history
  eviction handling. Resize invalidates anchors rather than guessing coordinates.
- Opt-in Zsh/Bash hooks preserve existing startup files, prompt text and hooks;
  real PTY tests verify status preservation and duplicate-source safety.
- Fish 4.9.3 emits compatible markers natively; real PTY, core parser and
  isolated Volt UI tests passed without a `volt.fish` hook. Command-output
  selection and retained anchors across reflow remain pending.
- Final validation: 219 Rust tests, 9 Python tests, strict Clippy, Rust 1.87 and
  132 GPU cases passed. Native default/custom navigation, Find ownership,
  alternate-screen input isolation and a real Zsh hook were verified.
- SGR dispatch was separated from cursor/edit handling after an initial build
  failed the performance gate. With tracking enabled, final seven-repeat parser
  comparison vs batch 2: color-heavy +8.09% to +11.01%, long-line -0.20% to -0.35%,
  many-line -1.88% to -2.12%. All cases pass the 5% gate; not zero cost and not a
  whole-app TermMark claim. Themes and existing functionality remain enabled.

- Fish 4.9.3 compatibility was verified after installation: no extra hook is
  needed or shipped, avoiding duplicate OSC 133 markers. F7/F8 navigated real
  Fish command output in an isolated Volt app.
