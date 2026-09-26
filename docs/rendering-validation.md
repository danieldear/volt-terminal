# Terminal rendering correctness and performance validation

Validation date: 2026-09-25 (America/New_York). Local machine: Apple M4 Pro,
macOS 27.0. This report describes the working tree, the v0.1.2 release candidate.
Existing unrelated working-tree changes were preserved.

## Implemented checklist

- [x] Preserve physical glyph x/y offsets and font bearings in terminal,
  overlay and tab-title placement.
- [x] Draw SGR underlines at the font baseline, including underlined spaces.
- [x] Generate cell-sized Powerline U+E0B0..U+E0BF and common block masks,
  independent of font fallback and line height. Shared rounded cell edges avoid
  fractional-scale gaps. Other icons still use the selected font/fallback.
- [x] Preserve extended graphemes (combining marks, variation selectors, ZWJ,
  modifiers and flag pairs), including sequences split across PTY reads.
- [x] Represent wide-cell continuations explicitly; repair pairs on overwrite,
  erase, insertion/deletion and clipped pane copies.
- [x] Preserve grapheme text in clipboard/search and remap grid-local text IDs
  during pane composition. Keep `Cell` Copy and 16 bytes for the ASCII path.
- [x] Prevent oversized atlas allocations; invalidate cached geometry on atlas
  recycling; upload only the changed atlas rectangle.
- [x] Preserve color emoji with an RGBA atlas rather than monochrome silhouettes.
- [x] Reuse unchanged CPU row geometry with invalidation for font, theme, scale,
  geometry, selection/search, cursor state and extended text changes.
- [x] Avoid cache storage overhead during mostly changing frames.
- [x] Add real GPU offscreen fixtures and cache-on/cache-off pixel comparisons.
- [x] Wire offscreen fixture validation and PNG artifacts into Linux CI.

The rendering pipeline remains:

```text
PTY bytes -> parser -> compact cells + interned graphemes
                     -> pane composition (ID remapping)
                     -> changed-row CPU geometry / reused row geometry
                     -> dirty atlas upload
                     -> full GPU clear + draw -> presentation
```

This is CPU geometry reuse, NOT partial swapchain repaint. Full GPU clear/draw
avoids relying on undefined retained contents of acquired surface textures.
RGBA atlas storage costs 16 MiB at 2048x2048 versus 4 MiB for the former R8 atlas.

## Reproduce

Run from the repository root:

```sh
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo run --release --locked -p volt-renderer --example rendercheck -- target/rendercheck
bash scripts/macos/build_app.sh --release
```

`rendercheck` also accepts `--fixtures-only` or `--bench-only` after its output
path. Artifacts are written under `target/rendercheck/`; they are generated,
not checked-in golden screenshots. A missing named font is skipped explicitly.
Linux CI installs DejaVu and software Vulkan, so named macOS fonts are not a
prerequisite. Remote CI is a release gate; see PR #3 checks for its current status.

## Correctness evidence

- 101 unit tests passed; three separately invoked benchmarks are ignored in the
  ordinary test suite. Formatting, strict Clippy and release app build passed.
- Local offscreen matrix: 27 cases = three font families (monospace, SF Mono,
  JetBrainsMono Nerd Font Mono) x scales 1/1.5/2 x line heights 1/1.2/1.8.
- For each matrix case, cached and forced-full rendering are pixel-identical
  for a static frame, one-row update and selection transition. Assertions also
  verify row reuse actually occurred.
- PNGs were visually inspected for separators, Unicode, underline and color
  emoji. Pixel equality alone proves cache equivalence, not an independent
  typographic correctness oracle.
- Added tests cover grapheme width changes and copying a pane slice whose edge
  intersects a wide glyph, including overwriting an existing half-pair.

## Measured performance

Measurements are local samples, not cross-terminal comparisons or universal
performance guarantees. Raw CSVs are in `target/rendercheck/`.

GPU harness: 200x60 cells, 330 frames per workload, first 30 excluded.
Cache-off/cache-on use separate warm renderers; execution order alternates every
frame to reduce drift from thermal/load changes. CPU time
covers frame construction/submission; completion includes waiting for the GPU.
No compositor, screen scanout or physical keyboard latency is included.

| Workload | Row cache | CPU p50 / p95 ms | CPU+GPU completion p50 / p95 ms |
|---|---|---|---|
| Static | off | 1.588 / 2.060 | 3.595 / 6.888 |
| Static | on | 0.459 / 0.680 | 1.782 / 5.528 |
| One row | off | 1.472 / 1.914 | 3.833 / 6.586 |
| One row | on | 0.445 / 0.680 | 1.780 / 5.651 |
| Full frame | off | 1.109 / 1.523 | 2.589 / 6.587 |
| Full frame | on | 1.103 / 1.496 | 2.659 / 6.336 |

Final paired parser samples (nine per version, alternating execution order,
~73 MiB ASCII capture): median **209.7 -> 230.1 MiB/s**,
approximately 9.7% higher. The program labels these MB/s but
computes binary MiB/s.

Final paired grid-copy samples (20 per version): median **4999.00 -> 5111.05
million cells/s** (+2.24%). Ranges overlap substantially (before
4309.1-5257.7; after
4531.5-5374.5). This reduces the earlier
2.2% median difference but does not establish zero regression or a statistically
significant change. These are CPU memory-copy results, not GPU throughput.

The baseline was a snapshot of this task's starting working tree, not clean
`main`; the tree already contained other changes. Final paired parser/blit CSVs
are recorded separately as `parser-final.csv` and `blit-final.csv`.

## Follow-up: remaining terminal work

- Main-screen resize now reflows one combined history + visible stream, carrying
  soft-wrap metadata through ring-buffer rotations. Shrinking the viewport
  pushes displaced rows into bounded history; growing can pull them back.
- Hard newlines remain separate. Long lines spanning the history/visible boundary
  stay joined. History retention is still bounded by the configured row limit;
  narrowing can legitimately evict oldest rows when that limit is exceeded.
- One-column wide cells retain their intrinsic width in a compact flag and regain
  valid lead/continuation pairs after widening, including emoji graphemes first
  entered while one column wide.
- Alternate screens use rectangular resize, not shell-history reflow. They and
  partial-height scroll regions no longer accumulate shell history. Returning
  from an alternate screen after resize restores the reflowed main cursor.
- Updating a populated scrollback limit now safely rebuilds the ring, keeping the
  newest rows rather than changing its modulus under existing data.
- Neovim smoke tests found missing `CSI 5 n` ready replies (startup delay) and
  missing background-colour erase behavior (patchy TUI backgrounds). Both are
  fixed and regression-tested. Erased spaces inherit background, not underline
  or bold attributes; resetting SGR also resets the erase template.

### Real application validation

Used the exact release executable in an isolated bundle with a distinct bundle
identifier/process name to avoid selecting the user's existing Volt instance.
Executable hashes were checked equal when staging. Test config and fixtures are
under `target/tui-validation/`; the user's config and terminal sessions were not
changed. Neovim 0.12.2 and tmux 3.7c were used (tmux installed via Homebrew for
this validation, with its managed dependency updates).

Observed directly in the native GUI:

- Neovim Unicode/colour emoji, Powerline statusline, vertical split and scrolling.
- Window zoom/unzoom caused actual terminal size changes and correct redraws.
- Reload Settings switched SF Mono / line height 1.8 to JetBrainsMono Nerd Font
  Mono / line height 1.0 while the TUI was running.
- After fixes, Neovim `:messages` was empty (no DSR startup warning) and background
  clears covered the pane instead of leaving jagged text-width blocks.
- tmux split panes, scrollback copy mode and resizing with Neovim in one pane.
  `capture-pane` verified a Unicode line plus all 120 numbered fixture lines.
- Native triple-click selection + Cmd+C exactly preserved `Unicode: 日本語 x́̈ 👩‍💻`
  (except the expected trailing newline). The user's previous clipboard text
  was restored immediately after the comparison.

Idle sample with visible Neovim splits and blinking disabled: 0.03 CPU seconds
in 20.014 wall seconds, approximately **0.15% of one CPU core**, RSS 107168 KiB.
This is one process sample, not a system energy/battery benchmark.

### Limits

- Real display input-to-photon latency and battery/power consumption are still
  unmeasured. They require a separate instrumented experiment; GPU completion
  time and process CPU must not be presented as substitutes.
- Smoke tests are not exhaustive daily-driver or terminal-protocol conformance.
- A resize is bounded by configured history retention and viewport semantics,
  not an unbounded archival guarantee. TUI alternate-screen content is redrawn
  by the application rather than reflowed as shell output.
- Per-cell grapheme extensions are bounded at 4096 UTF-8 bytes; excess zero-width
  extensions are discarded to prevent unbounded combining-mark streams.
- Fallback icons other than procedural separators may differ between fonts.
- This scoped audit is not a general security/dependency audit.

## Follow-up: dark cell behind underline cursor

A user screenshot exposed an opaque full-cell fill behind the underline cursor
when terminal background opacity was below 1.0. The cursor branch drew
`resolved_bg` (alpha 1) before drawing the thin stroke, overriding the translucent
clear. Beam cursors shared the same defect; selection/search backgrounds were
also overridden at non-block cursor cells.

The fix draws the ordinary cell background only when required by a custom
background, reverse video, selection or search, then overlays the cursor stroke.
Block cursors retain their full-cell fill.

A new real GPU regression first failed on the old implementation with the
non-stroke pixel changing from RGBA `[24,24,37,217]` to `[24,24,37,255]` at opacity
0.85. The harness now checks 60 combinations: underline/beam x opacity 0.85/1.0
x scales 1/1.5/2 x default/custom/reverse/selection/search backgrounds. It checks
that changed pixels are confined to the cursor stroke, the stroke is present,
cached output matches uncached output, and hiding the cursor restores the exact
original pixels. These checks run with `--fixtures-only`, including in CI.

Local strict Clippy also passed with Rust 1.98.0 (the CI toolchain). Native-only
menu entry points are conditionally compiled on macOS, and every workspace
package now inherits the declared Rust 1.87 MSRV.
