# Terminal foundations: batch 1 validation

Themes are excluded. This change adds configurable shortcut overrides, plain web
link clicks, and local working-directory inheritance. It does not implement the
remaining protocol/rendering features in `terminal-feature-plan.md`.

## Automated checks

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
# On this machine Rust 1.87 needs the Xcode 26.5 SDK rather than the newer CLT SDK:
SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.5.sdk \
  cargo +1.87.0 check --locked --workspace --all-targets
python3 -m unittest discover -s scripts/validation -p 'test_*.py'

cargo build --release --locked -p volt-renderer --examples
target/release/examples/rendercheck target/terminal-foundations/rendercheck --fixtures-only
target/release/examples/cardcheck
target/release/examples/searchcheck
bash scripts/macos/build_app.sh --release
codesign --force --deep --sign - target/macos/Volt.app
codesign --verify --deep --strict target/macos/Volt.app
```

195 Rust tests passed, 4 opt-in tests ignored; 123 GPU fixtures passed. The nine
Python tests passed. MSRV is compile validation on macOS, not Linux runtime proof.
No new production dependency; the existing VTE dependency is also used for UI
link tests. Cell layout and parser/render hot paths are unchanged.

## Speed gate

Save a release `throughputbench` executable **before changing source**, build the
new one, then run with builds/GPU checks finished and no concurrent benchmark:

```sh
cargo build --release --locked -p volt-core --example throughputbench
# Before editing: cp target/release/examples/throughputbench /tmp/volt-before
python3 scripts/validation/compare_throughput.py \
  /tmp/volt-before target/release/examples/throughputbench \
  --repeats 7 --mib 256 --cols 91 --rows 16 \
  --output target/terminal-foundations/performance-report.json
```

The script warms both executables, alternates order, compares each workload's
median, and fails if any loses more than 5% throughput. It records raw samples,
executable paths and SHA-256 digests. Do not average away a regressing case.

Measured on 2026-09-27 (MiB/s, larger is better):

| Workload | Wide seed | Before | After | Change |
|---|---|---:|---:|---:|
| Many lines | No | 284.86 | 281.95 | −1.02% |
| Many lines | Yes | 268.22 | 266.84 | −0.51% |
| Long lines | No | 390.16 | 389.59 | −0.15% |
| Long lines | Yes | 358.34 | 358.93 | +0.16% |
| FG per char | No | 424.22 | 420.88 | −0.79% |
| FG per char | Yes | 417.46 | 419.25 | +0.43% |
| FG/BG per char | No | 417.41 | 416.97 | −0.11% |
| FG/BG per char | Yes | 417.45 | 420.14 | +0.64% |

The v0.1.4 pre-release rerun also passed the 5% gate (7 alternating repeats,
256 MiB, 91×16): ManyLine −3.82%/−2.98% (plain/wide), LongLine
−2.97%/−3.32%, FG +7.34%/+7.02%, and FG/BG +11.50%/+11.56%.
The abandoned zero-copy row-storage prototypes were slower in a separate
headless PTY test and are **not** in this release. The headless TermMark runs
were noisy, so they do not establish an end-to-end speed improvement.

All eight passed. These are **parser-only** measurements, not PTY throughput,
compositor latency, energy use, or the user's TermMark result. Raw local artifacts
are under ignored `target/terminal-foundations/`; this table is the durable summary.

## Hands-on acceptance still pending

The isolated ad-hoc-signed app launched, but UI automation reported the Mac locked.
No successful native smoke test is claimed. Existing Volt sessions were preserved.
When testing, use a separate instance/config and do not change the user's theme.

1. Add the documented custom bindings to a disposable config; reload. Verify new
   action, unchanged defaults, `ignore`, `unbind`, exact Shift/Ctrl modifiers and
   last duplicate wins. Verify invalid bindings produce diagnostics, not a crash.
2. Open Find and workspace search; typing and Enter must edit/navigate the query,
   not trigger custom terminal actions. A focused card retains its keyboard input.
3. Toggle read-only; custom Paste must not send input to the process.
4. Print a wrapped HTTP(S) link with Japanese text before it; Cmd-click it in
   live output and scrollback. Check wide-character hits, prose punctuation,
   ordinary selection, and TUI mouse handling. Unsafe schemes must not launch.
5. `cd` to a local directory with spaces; open a tab, split and new window. `pwd`
   must show that local directory without an injected `cd` line. Repeat during SSH:
   a new local shell must not adopt the remote OSC 7 path.
6. Run the idle CPU gate on a foreground idle instance and matched before/after
   TermMark (same executable/workload, cell columns/rows, fonts, line height,
   window/display, opacity, scrollback, card state, warmup and alternating repeats).

The later roadmap items need their own protocol, resource-limit and UI gates;
a passing build is not permission to advertise unimplemented capabilities.
