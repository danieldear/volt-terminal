# Second performance pass: snapshot reuse and rejected throughput experiments

## Change retained in the working tree

The UI used to construct and drop a new `Grid` for every rendered frame. It now
retains one render-only snapshot per window. At the same dimensions it resets
and reuses the cell, row-map, wrapping, and dirty buffers; a size change creates
a correctly sized grid. Source terminal grids and their scrollback are untouched.

Resetting removes old grid-local grapheme IDs, colors, wrapping, cursor state,
and content. Thus changing panes/tabs, shrinking the viewport, or switching from
Unicode to ASCII cannot leave stale snapshot content. The existing pane blit and
scrollback offset logic is unchanged. There is no output dropping, feature
switch, or reduced rendering frequency.

## Measured CPU snapshot improvement (not end-to-end terminal speed)

Release, 10,000 iterations per case, fresh/reused/reused/fresh order:

| Grid | Fresh allocation + blit | Reuse + blit | Reduction |
|---|---:|---:|---:|
| 91x16 | 3.116 / 3.015 us | 0.840 / 0.812 us | about 73% |
| 200x60 | 15.599 / 15.393 us | 6.588 / 6.536 us | about 58% |

These are microseconds for one CPU preparation step, not GPU frame times or
TermMark scores. Saving a few microseconds per displayed frame is useful but
cannot by itself explain or recover several seconds in a PTY throughput test.

Reproduce with:

```sh
cargo test --release --locked -p volt-ui benchmark_snapshot_allocation_vs_reuse -- --ignored --nocapture
```

## Whole-app throughput: no proven additional speedup

Same instrumented termbench binary, release apps, 91 columns x 16 rows, reports
written only after the timed loops. A is the saved pre-snapshot `VoltEmojiReady`
binary, copied to a distinct baseline app; B includes snapshot reuse. Other user
applications were not stopped. Our builds and GPU tests were completed before
timed GUI comparisons.

| Order | Build | ManyLine | LongLine | FGPerChar | FGBGPerChar | Total seconds |
|---|---|---:|---:|---:|---:|---:|
| 1 | A | 7.9227 | 7.3880 | 2.4493 | 5.9319 | 23.6919 |
| 2 | B | 8.2251 | 7.9117 | 2.4736 | 5.9719 | 24.5823 |
| 3 | B | 7.8788 | 7.4496 | 2.4935 | 5.8383 | 23.6603 |
| 4 | A | 8.0557 | 7.2438 | 2.4427 | 5.8595 | 23.6016 |
| 5 | B, concurrent external benchmark | 9.1343 | 8.4385 | 2.7148 | 6.3965 | 26.6841 |

During the last run a separate `./termbench_release_clang normal` process was
found running alongside our test, and computer-use also reported user interaction
with the candidate preview. Timing runs were stopped without interrupting that
external process. The last sample is contaminated; it is retained above for
transparency. Do not retroactively assume the same explanation for earlier
samples without evidence. This small, variable set establishes neither an
additional whole-app improvement nor a strict no-throughput-regression claim.
The historical ~20.1451s target remains unrecovered.

## Experiments not retained

1. **Batching VTE's decoded ASCII callbacks.** Kept VTE responsible for UTF-8 and
   escape parsing, then collected printable callbacks into row writes. As
   implemented, buffering overhead outweighed its benefit: many-line parser
   throughput fell from 287.14 to 248.23 MiB/s; long-line throughput from 391.64 to
   321.20 MiB/s. The prototype was removed, not shipped on theoretical grounds.
2. **One codegen unit + full LTO.** No consistent benefit in the parser matrix;
   the release profile remains unchanged.
3. **Recycled PTY buffers + reusable coalescing allocation.** A headless production
   PTY/parser run was 23.3439s versus 22.9848s before, with additional retained
   memory. No benefit established; the original production pipeline was restored.
4. **Readiness strategy diagnostics.** A C sink that only drains bytes measured
   21.0286/21.1198s for blocking reads with kqueue readiness and
   20.3841/20.2978s for nonblocking reads with kqueue on EAGAIN. It drained the
   same 3,104,548,340 bytes but is NOT a terminal or renderer. Approximately
   3 million short successful reads per run expose a transport cost. A FIONREAD
   probe variant reached 20.2404/20.3448s but did not eliminate readiness waits;
   the extra syscall/scheduling effect is not a justified production change.

Nonblocking I/O is not a safe one-line flag change: duplicated PTY descriptors
share status flags, so the writer must correctly handle partial writes, EAGAIN,
bounded backpressure, cancellation, response ordering, and pane closure. That
larger transport redesign remains a possible next experiment, not a completed
or promised speed improvement. Removing wakeable shutdown is not acceptable.

## Correctness checks

- 180 workspace tests passed; four ignored manual benchmarks (one new).
- Snapshot reset is checked against every field of a fresh grid, including
  storage reuse at unchanged dimensions.
- Reused/fresh snapshots match across Unicode, scrollback, pane offsets, gaps,
  and viewport size changes.
- 123 GPU checks passed: 60 cursor/background, 27 fonts/scale/line-height,
  27 inspector, and 9 search cases.
- Strict workspace/all-target Clippy passed.
- Native preview renders the complete benchmark output; no installed app was
  replaced, and no commit or push was made.

A headless production-pipeline driver is provided separately:

```sh
cargo run --release --locked -p volt-core --example termbench_pty -- \
  /absolute/path/to/termbench-report /tmp/volt-termbench-report.txt
```

That driver bounds the wait for completion and has no GUI or display latency
measurement. Its executable must support `VOLT_BENCH_REPORT` to save timed results.
Local raw evidence and rejected prototypes live in ignored
`target/perf-investigation/`, including `round2-gui-results.json` and the parser
JSONL files. Test logs are `/tmp/volt-round2-*.log` and `/tmp/volt-snapshot-bench.log`.
