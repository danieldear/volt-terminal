# Deep throughput investigation

## Outcome

Three focused changes reduced a sequential same-machine TermMarkV2 Normal
comparison from **27.1552s to 23.0728s / 23.1060s** (23.0894s mean; 14.97% less
time). Floating inspector enabled: **23.1363s**. This does not establish that
Volt is the fastest terminal or reproduce the user's historical ~20.1451s.

The old build was the previous idle-loop-fixed preview, not the version with
the 100%-CPU wake loop. The earlier pre-card baseline measured 26.5744s;
therefore these improvements go beyond just undoing the card regression.

## Measurement boundaries

Apple M4 Pro 10P+4E, macOS 27.0, release profile, 91 columns x 16 rows,
same configuration, same instrumented termbench executable, sequential runs.
Order: optimized, unchanged, optimized, optimized with floating inspector.
Builds and profiling were not concurrent with these timed comparison runs.
Other user applications were not stopped; these are local samples, not a
controlled laboratory claim. The original run captured with macOS sample took
31.0963s and is deliberately excluded because sampling perturbs timing.

The benchmark summary is copied only after its timed loops; no tee or middleman
was inserted into its output stream. It measures producer/sink throughput,
not final GPU presentation or physical input-to-display latency.

| Case | Unchanged | Optimized 1 | Optimized 2 | Floating card |
|---|---:|---:|---:|---:|
| ManyLine | 9.1301 | 7.7457 | 7.8277 | 7.7936 |
| LongLine | 8.7192 | 7.1582 | 7.1332 | 7.1648 |
| FGPerChar | 2.7906 | 2.4242 | 2.4049 | 2.4244 |
| FGBGPerChar | 6.5152 | 5.7447 | 5.7402 | 5.7534 |
| Total seconds | 27.1552 | 23.0728 | 23.1060 | 23.1363 |

The separately user-reported Alacritty result was 26.1878s. Our new result is
numerically below that, but Alacritty was not rerun under this procedure, so
this is not a matched cross-terminal victory claim.

## Pipeline and findings

```text
child writes
    |
macOS PTY line discipline / buffering
    |
reader thread: readiness -> read -> bounded queue
    |
parser thread: VTE -> cells -> history -> damage
    |
short locked snapshot of visible panes
    |
CPU glyph/geometry cache -> Metal drawable -> compositor
```

### 1. Per-read readiness overhead: substantial, partly recovered

The PTY lifecycle hardening in 264f699 added poll before each read so an idle
reader can be interrupted when its pane closes. That safety is necessary.
However, many small kernel PTY reads make this extra operation expensive.

A diagnostic C program drained the identical workload into a buffer without
parsing, history, UI, or drawing. Individual exploratory totals:

| Drain strategy | Total seconds |
|---|---:|
| poll before each read | 22.1755 |
| blocking read, no cancellation | 17.6667 |
| persistent kqueue | 21.1263 |
| select | 21.1127 |
| nonblocking read, poll on EAGAIN | 21.5424 |

These single-run diagnostic samples isolate a plausible cost; they are not
terminal scores or a strict lower bound. Blocking read was **not shipped**:
removing cancellation would reintroduce stuck reader threads. Nonblocking I/O
was not adopted either; duplicated master descriptors share file status flags,
so changing them also requires a correctly backpressured writer.

The implementation now uses a persistent macOS kqueue with two level-triggered
read filters: PTY output and the shutdown socket. It preserves:
- explicit wakeup on pane close, without periodic idle polling;
- unread output draining before EOF;
- shutdown priority when both descriptors are ready;
- owned descriptors, RAII queue cleanup, close-on-exec, EINTR retry;
- the existing poll implementation on other Unix platforms.

kqueue was preferred to select despite similar exploratory timing because it
does not require imposing FD_SETSIZE bounds on descriptor numbers.

### 2. Repeated damage-range merging per character: reduced

The common ASCII path already marked a row dirty in the grid, then recomputed
the performer's encompassing damage interval for every character. Most writes
remain inside that interval. mark_dirty_row now returns early when the row is
already covered, while still setting display_dirty. Reset and expansion
semantics remain unchanged.

### 3. Historical wide glyph penalized later ASCII: reduced

Grid's has_wide hint is conservative and survives scrolling. Previously one
wide glyph could make all subsequent ASCII writes use the general repair
writer, including after the wide glyph had left the screen.

The ASCII writer now calls the repair path only if the overwritten cell is
actually a wide lead or continuation. It does not clear partners incorrectly,
drop Unicode, or pretend all symbols have width one. A differential test checks
the optimized writer against the general writer at multiple widths, both while
wide pairs are present and after scrolling them into history.

The new parser-only throughputbench separates plain output and color-heavy
sequences, with and without an initial wide glyph. Exploratory 256 MiB samples:

| Pattern | Before MiB/s | Both cell-path changes MiB/s |
|---|---:|---:|
| Many lines, plain | 246.77 | 275.04 |
| Many lines, wide seed | 208.56 | 262.98 |
| Long line, plain | 313.34 | 385.10 |
| Long line, wide seed | 280.43 | 351.00 |
| Foreground color, plain | 409.48 | 422.04 |
| Foreground/background, plain | 413.13 | 416.36 |

These synthetic results are not substituted for the real-app results above.

### 4. Renderer: not the first throughput target

During the sampled plain-output phase, the parser's ground dispatch/cell-update
path dominated its active stacks; scrolling also appeared significantly.
The main thread spent much of its sampled time waiting in CAMetalLayer
nextDrawable. Waiting is not CPU rendering cost, and the performer lock is
released before drawable acquisition. It would be misleading to call this
proof of GPU saturation or to disable presentation merely to improve termbench.

Secondary opportunities observed in source/profile:
- reuse the visible render-grid allocation instead of allocating per frame;
- reduce repeated glyph-cache hashing on frequently changing ASCII rows;
- profile color-heavy phases separately before changing SGR parsing;
- investigate parser ASCII-run batching, with strict UTF-8/escape/chunk-boundary
  differential tests, rather than an unsafe bypass of parser state.

These are **not implemented or claimed as measured wins** in this pass.
Allocation stacks were smaller than parser/transport costs, so a broad renderer
rewrite is not justified by this throughput profile.

## Correctness and latency gates

- 175 Rust tests passed; three existing tests remain ignored.
- Added wide-writer differential, damage-range/reset, kqueue tail/EOF,
  shutdown-priority, and idle-shutdown wakeup coverage.
- Existing child termination, repeated-output tail drain, and worker release
  tests also run against the new reader.
- PTY lifecycle tests passed three additional consecutive runs.
- Floating-card idle CPU measured 0.90% of one core over ten seconds.
- Strict all-target Clippy and formatting pass.
- GPU validation: 60 cursor-background, 27 font/scale/line-height, 27 inspector,
  and 9 search cases pass (123 total).

500 PTY child-response samples after 30 warmups, milliseconds:

| Build | p50 | p95 | p99 | Max |
|---|---:|---:|---:|---:|
| Pre-card 264f699 | 0.038417 | 0.059000 | 0.066708 | 0.097709 |
| Optimized | 0.039041 | 0.047666 | 0.064167 | 0.124083 |

This small sample shows no meaningful median regression, not a guarantee about
all latency tails. It excludes the display/compositor entirely.

## Reproduction and remaining work

```sh
cargo run --release --locked -p volt-core --example throughputbench -- 256 91 16
cargo run --release --locked -p volt-core --example ptybench -- 500
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Local raw artifacts are in target/perf-investigation, including
deep-throughput-results.json, parser-before2.jsonl, parser-both-trial.jsonl,
latency-before.json and latency-after.json. The diagnostic sink C source is
ptydrain.c there; it is deliberately not a production terminal implementation.

To pursue the remaining ~3 seconds, prioritize transport scheduling/batching
experiments with latency and shutdown gates, then ASCII-run parsing. Do not
disable line discipline, discard output, reduce scrollback, hide the card,
raise latency by arbitrary buffering delays, or remove correctness repairs just
to improve a throughput number. Establish a repeatable competitor matrix at
identical grid sizes before advertising a speed lead. Include TUI response,
Unicode/reflow, idle CPU, and sustained memory alongside throughput.

## Second pass

[Snapshot reuse and further throughput experiments](performance-round2.md)
records a measured CPU frame-preparation gain, rejected parser/transport
prototypes, and inconclusive whole-app timing. It does not claim the ~20s
historical score has been recovered.
