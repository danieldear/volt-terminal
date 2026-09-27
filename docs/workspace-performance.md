# Workspace UI performance regression investigation

## Confirmed defect and fix

The inspector tick was passed `state.proxy.clone()` from every
`about_to_wait`, including when the inspector was disabled. Search did the
same while visible. In the pinned winit macOS implementation, proxy cloning
creates/adds a CFRunLoopSource and explicitly calls CFRunLoopWakeUp. Thus:

```text
about_to_wait -> clone proxy -> wake main loop -> about_to_wait -> ...
```

A sampled idle preview spent 3.01 CPU seconds over 3.01 wall seconds
(100.01% of one core); its native stack included proxy clone and
CFRunLoopAddSource. This was not terminal parsing or GPU work.

Both tick APIs now borrow the proxy. An owned clone is created only when an
actual background worker launches. File-preview actions also borrow until a
worker is necessary. No terminal parsing, redraw throttling, or output-dropping
change was made to improve these numbers.

Afterward, measured idle CPU was 0.80% with the card disabled and 1.33% with
the floating card enabled; settled search measured 1.00%. These are short local CPU samples, not battery/power
measurements or guarantees on other machines.

## Controlled termbench comparison

Apple M4 Pro (10P+4E), release builds, same user configuration and 91x16 grid.
Baseline is clean commit 264f699 (before workspace card/search additions).
Old spinning test previews were closed before these runs. Other user apps
were left alone. Benchmark runs were sequential, without concurrent builds.

The benchmark source was copied into ignored target output and instrumented
only to save the final summary *after* the timed loops. No tee or intermediate
output consumer was used. These measurements describe writer/sink throughput,
not completed GPU presentation latency.

| Build / mode | ManyLine | LongLine | FGPerChar | FGBGPerChar | Total |
|---|---:|---:|---:|---:|---:|
| Pre-card 264f699 | 8.7614 | 8.6936 | 2.7589 | 6.3606 | 26.5744 |
| Fixed, card hidden, run 1 | 8.9225 | 8.4892 | 2.7354 | 6.4221 | 26.5692 |
| Fixed, card hidden, run 2 | 9.0593 | 8.6816 | 2.7714 | 6.4091 | 26.9213 |
| Fixed, floating card | 9.1104 | 8.5937 | 2.7973 | 6.4143 | 26.9157 |

All values are seconds; lower is better. Floating mode retained the same grid.
These samples are close to the measured pre-card baseline, not proof of zero
overhead. Earlier 28-29 second comparisons were contaminated by two idle
previews spinning and should not be used as clean baselines.

**The user's historical ~20.15s score has not been reproduced.** Their
Alacritty score of 26.1878s was reported separately, not run under this
controlled setup. Do not claim a restored lead over Alacritty. A further
comparison needs the exact historical binary/configuration, identical terminal
dimensions, benchmark binary, and repeated alternating runs on an otherwise
comparable system. Do not change parser correctness just to chase the score.

## Repeatable idle regression gate

Leave the selected test process at an idle shell prompt, after initial loading.
Repeat with card disabled, enabled, and search results settled. Do not type,
resize, or run a command during the sample.

```sh
python3 scripts/validation/sample_process.py PID --seconds 15 \
  --max-cpu-percent 5 --output target/idle-check.json
python3 -m unittest discover -s scripts/validation -p 'test_*.py'
```

100% means one fully busy core. The optional budget fails with a nonzero exit
status while retaining the measurement JSON. Five percent is a smoke-test
budget for this setup, not a universal specification. Rust unit tests alone
cannot catch a native CFRunLoop wake loop; keep this runtime check when changing
event-loop integration.

## Subsequent parser/PTY optimization

A deeper follow-up improved the real-app result to roughly 23.1 seconds without
disabling the inspector. See [deep throughput investigation](performance-deep-dive.md)
for the comparison, source-level findings, correctness gates, and remaining work.
The 26-second measurements above remain the earlier idle-loop-fix baseline.
