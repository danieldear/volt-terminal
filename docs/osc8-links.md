# OSC 8 hyperlinks (terminal batch 2)

Themes and theme UI are untouched. This extends the on-demand plain-URL feature,
not an AI/context-card feature.

## Behavior

```text
OSC 8 opens a destination
    -> printed cells remember that destination
    -> scrolling/reflow/insert/delete move the cells and their association
    -> Cmd-click / Ctrl-click resolves the cell, not the visible label
    -> a read-only destination confirmation opens
    -> Enter opens HTTP(S); Escape cancels
```

The confirmation starts at the URL's beginning so the host is visible. Left/Right
and Home/End inspect long URLs. Typing, paste and IME input cannot modify the
captured destination; later terminal output cannot swap it under the dialog.
Plain HTTP(S) URLs without OSC 8 metadata still open directly on modifier-click.
An unsupported explicit destination does not fall back to opening its label.

Try the fixture in Volt:

```sh
sh scripts/validation/osc8-demo.sh
```

Both BEL and ESC-backslash terminators work. URI semicolons and optional `id=`
parameters are retained; unknown OSC parameters are ignored. Closing OSC 8 clears
the active link; SGR reset does not. Switching between main/alternate screens
clears the active link so unclosed links cannot leak into the other screen;
already printed links survive returning to the main screen. RIS resets everything.

Only HTTP(S) destinations can be opened. File, command, JavaScript and other
schemes are not supported. Credentials, control/whitespace characters and bidi
formatting characters fail validation. No shell interpolation or automatic launch
occurs. Opening still needs an OS URL handler (`open` or `xdg-open`). Hover
underlining and link grouping highlights are not implemented.

## Storage and performance

`Cell` remains **16 bytes and Copy**. Linked characters use the existing grid-local
extended-text IDs with a lazily allocated side table. Ordinary output does not
allocate a link table. Copying, reflow, scrollback, erasure and overwrite use the
same cell data as before. Cross-grid copies remap IDs; text-pool compaction keeps
link identity distinct even when two labels have identical text.

The renderer recognizes single-scalar extended entries and uses the same scalar
cache/procedural-symbol path as unlinked text. GPU checks compare linked and
unlinked fixtures pixel-for-pixel, including Powerline symbols, Unicode and emoji
at three fonts, scales and line heights.

Bounds per grid:

- URI: 2,048 UTF-8 bytes; `id`: 128 bytes; parameter string: 1,024 bytes.
- 4,096 distinct link identities, 65,536 linked text entries, 8 MiB linked text.
- At capacity, new associations degrade to ordinary text; no per-character full
  history scan is used to reclaim them. Existing associations remain valid.
- Pool compaction/explicit scrollback cleanup or terminal reset reclaim metadata.
- Commands at VTE's 16-parameter ceiling are rejected conservatively, so parser
  truncation cannot turn an over-segmented URI into a different clickable URL.

These are metadata bounds. They are **not** a claim that the upstream VTE parser's
raw OSC receive buffer is globally bounded; general escape-string buffering is a
separate hardening concern.

## Validation

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo run --release --locked -p volt-renderer --example rendercheck -- \
  target/terminal-links/rendercheck --fixtures-only
cargo run --release --locked -p volt-renderer --example linkcheck
```

Core tests cover byte-fragmented input, BEL/ST, URI semicolons, SGR, combining marks,
wide cells, wrapping, history eviction, reflow, insert/delete/erase, main/alternate
screens, RIS, malformed/oversized input, compaction, import and metadata budgets.
UI tests cover destination precedence, unsafe schemes, Unicode cell hits and
immutable confirmation input. GPU tests add linked/plain equality and preview bounds.

209 Rust tests passed (4 ignored), strict Clippy and Rust 1.87 all-target checks
passed. 132 GPU cases passed, including linked/plain equality in 27 font/scale/
line-height fixtures and nine long-preview bounds checks. Release app built and
ad-hoc signature verified.

Native smoke testing verified rendered labels, custom F6 Find and searching linked
text in an isolated app. Foreground idle sampling measured **0.73% of one core over
30 seconds**, passing the 1% gate. The full native modifier-click/Enter/Escape/
browser path remains manual: the available UI automation API has no modifier-click
operation. No full native acceptance is inferred from unit/GPU checks.

Render-only snapshots discard URL metadata and convert linked scalars back to
ordinary scalar cells; actual terminal grids retain all link associations. This
avoids allocating and rehashing destination tables every rendered frame.

Performance experiments rejected an inline linked-ASCII branch (about 6–10% loss
on ASCII workloads) and splitting Unicode/wrap printing out of line (about 3–5%
ASCII loss). Neither is in the final source. The retained implementation leaves
ordinary ASCII writes on the existing fast path with one active-link guard.
Final measurements follow below. Parser benchmarks do not replace matched
whole-app TermMark runs.

Protocol references: [OSC 8 specification](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda),
[iTerm2 escape-code documentation](https://iterm2.com/documentation-escape-codes.html).


### Final parser comparison (2026-09-27)

Seven alternating baseline/new repeats, 256 MiB per case, 91×16 cells, with warmup
and no concurrent build or test app. Baseline is the saved batch-1 release executable.
Median throughput changes ranged from **-3.26% to +0.62%**. All cases
passed the 5% per-case gate, but this is not a claim of literally zero overhead
or whole-app TermMark parity. The ASCII cases show a small measurable cost.

| Case | Wide seed | Before MiB/s | After MiB/s | Change |
|---|---|---:|---:|---:|
| fg | No | 402.30 | 402.05 | -0.06% |
| fg | Yes | 402.44 | 401.30 | -0.28% |
| fgbg | No | 398.76 | 400.36 | +0.40% |
| fgbg | Yes | 394.67 | 397.13 | +0.62% |
| long | No | 381.25 | 370.12 | -2.92% |
| long | Yes | 349.16 | 337.77 | -3.26% |
| many | No | 272.09 | 263.85 | -3.03% |
| many | Yes | 254.13 | 251.72 | -0.95% |

Raw samples and executable SHA-256 digests: `target/terminal-links/performance-final.json`
(local ignored build artifact). The final report, not a best-of-run selection, is summarized above.
