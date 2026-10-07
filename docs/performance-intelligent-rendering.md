# Rendering optimization and diagnostic evidence

Measured on 2026-10-07. These changes are local, not a new release. The baseline
includes the earlier SGR optimization; the comparison isolates this rendering
and instrumentation work. No rendering quality, terminal input/output,
scrollback, task safety, or secure-input checks were removed.

## Retained changes

- A fixed 128 × 4 ASCII glyph-shape table replaces hashing on the common ASCII
  path. Its four slots separate regular, bold, italic and bold-italic. Non-ASCII
  scalar text retains randomized HashMap hashing and its existing cache limit.
- Extended glyph shapes use borrowed `str` lookup in style-specific caches.
  A hit does not allocate a key, clone the glyph vector, or perform a second
  hash lookup. Misses move the shaped result into the cache after drawing.
- Unchanged task buttons reuse geometry rather than reshaping labels each
  terminal frame. Status, labels, notices, theme, font, size, scale and position
  participate in invalidation. Atlas resets clear the geometry cache.
- Opt-in aggregate phase timing distinguishes PTY reads, queue operations,
  parsing/lock waits, snapshot work, drawable acquisition, geometry and CPU
  submission/presentation. Failed surface acquisition is counted explicitly.

Font/scale updates clear both ASCII and extended shape caches. Cache keys do not
include color, since shaping does not depend on per-cell foreground/background.
Full GPU repaint remains unchanged; swapchain contents are never assumed to be
preserved. No new dependency or unsafe code was added.

## Offscreen CPU rendering results

`cachebench` renders a 200 × 60 grid to a real offscreen GPU texture and waits
for completion each frame. Each workload discards 30 warm-up frames and measures
300 frames. `compare_rendering.py` warms both saved binaries, alternates their
order for seven repeats, validates workload/frame counts and records executable
SHA-256 hashes. Builds, other tests and profiling did not overlap timing.

The table is the median of each run's CPU build+submit p50, in milliseconds.
It is **not display latency, PTY throughput, or a termbench speedup**.

| Workload | Before | After | CPU time change |
| --- | ---: | ---: | ---: |
| Full-screen ASCII churn | 1.676625 | 1.396125 | -16.73% |
| One ASCII row changing | 0.820084 | 0.860583 | +4.94% |
| Full-screen churn with task buttons | 1.801958 | 1.613042 | -10.48% |
| Full-screen Unicode churn | 1.207250 | 1.079292 | -10.60% |
| Static Unicode screen | 0.368625 | 0.361375 | -1.97% |

The retained version passed the existing 5% CPU-p50 regression gate. The
one-row case is near that budget; do not describe every workload as faster.
CPU-p95 changes were -4.00%, +3.20%, -7.08%, -8.09%, and +0.64%, respectively.
These sub-millisecond/millisecond CPU metrics vary with driver and machine load.
No estimate of a visible-window termbench gain is justified by this table.

A row-bookkeeping buffer-reuse prototype triggered a static-screen CPU
regression gate in two trials. It was removed rather than relaxing the gate.
The raw prototype reports are retained alongside the final results.

## Important visible-window verification limitation

New diagnostics caught a problem with automated window trials in this session:
the app processed PTY bytes and snapshots but recorded **zero successfully
rendered frames**. CoreVideo reported no active display-link scheduler. Those
runs are excluded as evidence of GPU/presentation performance, even though the
app/PTY timings appeared repeatable. A visible-window comparison on the user's
Mac is still required. Earlier app trials without presentation counters should
not be promoted as validated display/rendering measurements.

The final diagnostic run explicitly counted 865,724 occluded-surface attempts
and zero rendered frames. It read and parsed the same 3,104,548,258 bytes. Its aggregate
counts showed approximately three million small reads/batches, modest performer
lock waiting, and substantial snapshot work in the no-drawable environment.
Do not extrapolate those snapshot costs to a working display, or alter frame
pacing/security checks based on that environment alone.

## Checks and manual verification

- Workspace/all-target Rust tests, strict Clippy and Rust 1.87 checks.
- Core tests with diagnostics enabled as well as disabled.
- Validation-script unit tests.
- 60 cursor/background and 27 font/scale/line-height GPU fixture groups.
- Font-size cache invalidation round trips and linked/plain pixel equality.
- Task-button GPU checks, including all six states in two themes at two scales,
  cache-hit/fresh-atlas pixel equality, confirmation occlusion and clean hiding.

The prepared local test bundle is ad-hoc signed and passed strict signature
verification; this is not a notarized release. Build the executable/bundle with
`bash scripts/macos/build_app.sh --release`. Quit the
older running Volt before opening `target/macos/Volt.app`, to avoid testing the
wrong executable. Compare at least three termbench runs per build with matching
window/grid size, font, line height, appearance, scrollback and workspace-card
state. Do not enable profiling for timed comparisons. Also exercise Neovim,
selection, cursor blinking, emoji/combining text and task status changes.

For content-free phase aggregates (printed when the pane/window exits):

```sh
VOLT_PERF=1 target/macos/Volt.app/Contents/MacOS/Volt
```

Only counters, fixed component/stage labels and durations are recorded by these
metrics. They do not capture terminal contents, commands, paths or credentials.
Other ordinary application errors on stderr are separate from these metrics.
`queue_send` includes buffer allocation and sending; it is not a pure measure
of queue blocking. `pending_bytes_peak` includes queued buffers plus at most one
pending send, not the parser's active batch. CPU phase durations overlap across
threads and must not be summed into application wall time. CPU submit timing is
not GPU completion or input-to-display latency. Profiling changes scheduling.

Raw reports and saved binaries live locally under ignored
`target/perf-intelligent/`; the final CPU report is `rendering.json`.
