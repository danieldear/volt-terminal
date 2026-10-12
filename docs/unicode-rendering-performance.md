# Unicode rendering investigation — 2026-10-12

## What was actually measured

The earlier ~19% result was an **offscreen render-frame build/submission
wall-time** comparison. The legacy JSON field is named `cpu_p50_ms`, but the
measurement uses `Instant`, not a thread CPU clock. It includes CPU work and
blocking/scheduling inside GPU API calls. It is not a TermMark throughput result,
not input-to-photon latency, and does not establish a 19% whole-terminal slowdown.

The original three pre-upgrade/v0.1.13 batches exceeded the 5% budget for
Unicode full redraw. Those failed results remain recorded in
[the toolkit integration evidence](ui-toolkit-migration.md). This investigation
retains failed repeats too; it does not weaken the threshold or alter the fixture.

## Verified fix

Each changed row previously dropped all its owned grapheme strings, then
allocated and copied new strings, even when the Unicode suffix was unchanged
and only an ASCII prefix/counter changed. `CachedRow::capture_source` now reuses
those buffers, skips copies when text is equal, updates moved/changed entries,
and truncates removed entries. Grapheme IDs remain grid-local: cache validation
still compares actual text across grids rather than trusting IDs.

This does not change fonts, shaping, emoji, style handling, row-reuse policy,
glyph placement, the UI toolkit, or the rendering backend. No feature is disabled.

Three new CPU-only regression tests cover allocation retention, moved/changed
clusters, removal, and equal local IDs with different grapheme content. A
nine-pair CPU-only bookkeeping benchmark measured 351.4 ns/row for the legacy
capture versus 171.5 ns/row for buffer reuse (-51.2%). That is bookkeeping,
**not** a claimed 51% renderer or terminal speedup.

## All investigation batches

Each row represents seven alternating before/after pairs. Positive values mean
slower build/submission wall-time medians. Neither failing batches nor the
identical-binary control are discarded.

| Comparison | ASCII full | ASCII one-row | Tasks full | Unicode full | Unicode static | 5% CPU-labelled budget |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Unchanged v0.1.13 vs pre-kit baseline | +9.5% | +5.9% | +3.1% | +2.2% | -11.4% | Fail |
| Fix vs v0.1.13, batch 1 | -2.5% | -10.2% | +2.8% | -9.1% | +1.5% | Pass |
| Fix vs pre-kit, batch 1 | -21.2% | -3.6% | +10.8% | -19.6% | +5.8% | Fail |
| Fix vs v0.1.13, batch 2 | +1.5% | +2.4% | -6.8% | -19.8% | +16.6% | Fail |
| Fix vs pre-kit, batch 2 | +14.0% | +3.1% | -3.6% | -18.1% | -8.0% | Fail |
| Identical v0.1.13 binary vs itself | -11.7% | +13.2% | +5.4% | -2.3% | +8.3% | Fail |

The fixed executable has the same SHA-256 in both fix batches:
`3fd49d8fa89b6ce98e289baeaf9679c5e07b450e5517c4a72962548464497f79`.
The pre-kit reference is the saved 0.2.0 development binary, not a reconstruction
from a pure release commit. The v0.1.13 reference uses the released renderer code.

**The identical-binary control also fails.** Its before/after SHA-256 values are
identical. It shows that this measurement setup has substantial system/per-process
variation even without a code change. It does not identify the cause of that
variation, prove every earlier failure was noise, or justify waiving the gate.
The four fix comparisons consistently improve Unicode full redraw, but static,
ASCII and task results are inconsistent. Overall performance acceptance remains
inconclusive; do not call this a blanket no-regression pass.

## Functional acceptance

- 710 Rust tests passed, including the new retained-buffer and cross-grid tests.
- 36 Python checks, strict workspace/all-target Clippy, formatting and Rust 1.87
  workspace/all-target checks passed.
- 512 GPU cases passed: 60 cursor-background, 27 font/scale/line-height pixel
  equality, 27 Settings, 27 cards, 9 search, 2 task UI and 360 tab cases.
- Cached/full and linked/plain pixels match, including emoji, combining text,
  styles, font changes, selection and changed rows.
- Local release-mode app rebuilt and its ad-hoc signature verified. Existing
  published/tagged artifacts are not rewritten and the draft is not promoted.

## Remaining release acceptance

Run matched visible-window TermMark trials with identical grid size, font,
line height, backing scale, scrollback, shell integration and workspace-card
state, after other builds/rendering tests finish. Preserve at least three
complete trials per build and use `scripts/validation/compare_termbench.py`.
Repeat the offscreen comparison with an identical-binary control in stable
conditions. A failed control makes code-level attribution inconclusive; it does
not turn a failed release gate into a pass. No full-app performance result is
claimed by this change.
