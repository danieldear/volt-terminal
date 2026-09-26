# Performance experiments and boundaries

Keep these measurements separate:

```text
input -> PTY write -> child -> parser -> CPU geometry -> GPU -> compositor -> photons
         |_________________________|   |_________________|
                  ptybench                 rendercheck
|_______________________________________________________________________________|
                   physical input-to-display experiment (not measured)
```

## Reproducible automated measurements

From the repository root:

```sh
cargo run --release --locked -p volt-core --example ptybench -- 500
cargo run --release --locked -p volt-renderer --example rendercheck -- target/rendercheck --bench-only
python3 scripts/validation/sample_process.py PID --seconds 60 --output target/idle.json
```

`ptybench` runs an isolated `/bin/sh`, disables kernel echo, and waits for a unique
child-generated reply to appear in parsed cells. It excludes 30 warmups and
reports p50/p95/p99/max. Its scoped PTY is terminated/reaped on exit. It measures
no window, GPU, compositor, display or physical keyboard.

The process sampler captures CPU time and RSS, not terminal output, arguments,
environment or clipboard. It guards against PID reuse, uses monotonic wall time,
and reports CPU as a percentage of one core. Sample the isolated validation app,
not an unrelated user session. RSS is resident memory, not a leak diagnosis.

For comparisons: same release profile, machine, AC/battery state, window/cell
size, font, refresh rate and workload. Warm up, alternate execution order, run
multiple trials, retain raw samples and report spread. Shared CI runners should
check correctness, not enforce small noisy latency thresholds.

## Physical latency (external equipment required)

1. Use a hardware input trigger/LED visible to a high-speed camera, or a
   synchronized input injector plus display photodiode/logic analyzer.
2. Show a minimal response workload with no shell plugin/TUI processing delay.
3. Record at least 100 events per condition; alternate terminals/trials.
4. Timestamp physical input and first changed display pixels, documenting camera
   frame-time uncertainty, display refresh rate, position and scanout direction.
5. Report p50/p95 and uncertainty. Never substitute PTY/GPU completion timestamps.

## Power / energy (separate controlled experiment)

1. Fix brightness, refresh rate, window size, cursor blink, workload and network
   state. Allow thermal stabilization; pause unrelated work with user consent.
2. Record matched baseline, idle, one-row updates and full-output runs, each for
   several minutes and repeated in alternating order.
3. macOS `powermetrics` requires administrator access. Use documented samplers
   available on the machine, record its version/options, and label its power
   numbers as estimates rather than wall-meter measurements.
4. Prefer a controlled external power meter for whole-system energy. A battery
   percentage delta or process CPU percentage alone is not an energy benchmark.

This pass was on AC power. Noninteractive administrator access for powermetrics
was unavailable; no power result is claimed. Do not disable system protections
or automate password entry to run the experiment.
