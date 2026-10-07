# Color-parser optimization: measured scope

Measured locally on 2026-10-07 against main `71b4cc5` (v0.1.10).

## Kept change

`volt-core/src/performer.rs` now uses a plain SGR parameter iterator instead
of unnecessary peekable state. It synchronizes the grid's erase background
when a background parameter changes, rather than checking it after every
foreground or style sequence. Reset, save/restore and alternate-screen paths
retain their existing synchronization.

No feature, output, scrollback, security polling or rendering-quality setting
was removed. PTY transport, renderer and UI production code are unchanged.

## Results

### Parser only — not a whole-terminal score

The existing `compare_throughput.py` harness ran seven alternating repeats
per executable after warm-up, with 128 MiB per case at 91 columns × 16 rows.
Rates below are median MiB/s for ordinary ASCII with no seeded wide history:

| Workload | Before | After | Change |
| --- | ---: | ---: | ---: |
| ManyLine pattern | 283.06 | 282.64 | -0.15% |
| LongLine pattern | 372.48 | 373.89 | +0.38% |
| Foreground per character | 448.21 | 462.58 | +3.21% |
| Foreground/background per character | 460.75 | 471.50 | +2.33% |

With seeded wide history, color gains were +1.55% to +3.19%. With an OSC 133
prompt anchor seeded, color gains were +2.70% to +3.63%; the non-color cases
were +0.01% to +0.98%. No case exceeded the harness's 5% regression budget.
Initial shorter trials showed larger gains; the seven-repeat results above
are the retained evidence, not the best individual run.

### Automated app timing — presentation unverified

**Follow-up limitation:** later stage diagnostics found that automated window
runs in this session did not successfully acquire GPU drawables. The earlier
runs below did not count presentations, so their GPU/display participation is
unverified. Keep them as app/PTY timing evidence only, not a validated visible-
window rendering comparison. See `performance-intelligent-rendering.md`.

Four runs each, ordered ABBAABBA, used the real app and PTY with
identical font/appearance settings: SF Mono 12, line height 2.0, transparency
and blur enabled, workspace card closed. Every run confirmed the same
91 × 16 terminal grid. Test shells used isolated XDG configuration, not the
user's shell startup files; user settings and sessions were not modified.

| TermMarkV2 Normal | Median | Range |
| --- | ---: | ---: |
| Main baseline | 23.774024 s | 23.659352–23.822492 s |
| Color-parser change | 23.743776 s | 23.646214–23.806734 s |

The 30 ms (0.13%) difference is smaller than run-to-run variation. It is **not
evidence of a meaningful application speedup**, nor recovery of the historical
~22 s result. These concurrent-session baselines must not be directly compared
with older user runs made under different machine load/configuration.

Termbench wrote its summary file only after the timed loops; benchmark output
still went directly into the PTY. No output filtering or line-discipline
changes were used. No builds, profiling or other test workloads ran concurrently
with timed app runs. Other user applications remained running.

## Rejected experiments

Do not repeat these changes based on theoretical savings alone:

| Experiment | Matched whole-app median before → after | Decision |
| --- | --- | --- |
| Copyable cell-style template | 23.126 → 23.439 s | Faster parser microbenchmarks, slower app; removed |
| Ownership-only parse coalescing | 23.787 → 23.725 s | Difference too small to justify added complexity; removed |
| Nonblocking read, readiness on EAGAIN, backpressure-aware writer | 23.748 → 23.822 s | No app benefit; removed |
| Nonblocking reads with cached readiness byte budget | 23.791 → 23.981 s | Slower app; removed |
| Doubled row-map ring / constant-time full-scroll mapping | Parser ManyLine/LongLine about 2–4% slower | Removed before app testing |

A fresh owned-app sampling run showed substantial parser ground dispatch and
scroll-copy activity; the main thread spent most sampled time waiting for a
Metal drawable. Waiting samples alone do not prove GPU saturation. That
profile was excluded from all throughput results.

## Regression checks

- 338 Rust workspace/all-target tests passed; four existing manual benchmarks ignored.
- Strict workspace/all-target Clippy and Rust 1.87 checks passed.
- 21 validation-script tests passed, including shell hooks.
- 60 cursor/background and 27 font/scale/line-height GPU fixtures passed.
- New tests cover every supported background color form, compound resets,
  foreground-only erasure, bytewise SGR, alternate screens, and Unicode/link
  metadata across attribute save/restore.
- Formatting and diff checks passed.

Raw results are retained locally under ignored `target/perf-optimization/`:
`parser.json`, `parser-prompts.json`, `gui-sgr-results.json`, and rejected-trial
JSON files. The parser reports identify executable hashes. This is a local
optimization, not a published release or a claim to be the fastest terminal.
