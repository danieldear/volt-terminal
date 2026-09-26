# Terminal hardening plan (post-v0.1.2)

Scope: terminal correctness, reliability, measurement, distribution, and CI.
Linux UI and the AI prototype remain deliberately deferred. Passing this plan
is not a claim of complete xterm compatibility or exhaustive daily-driver QA.

1. Protocol regressions: chunk-independent parsing, combined DEC modes,
   scroll margins/origin/wrap, cursor save/restore, erase history, colour syntax,
   unsupported-sequence isolation. Fix confirmed failures, not speculative APIs.
2. Reliability: deterministic seeded resize/edit/Unicode/history stress,
   bounded grapheme retention, PTY exit/drop/reaping tests and fixes.
3. Measurement: repeatable PTY round-trip and isolated idle CPU/RSS tools,
   retain GPU benchmarks, define physical latency/power experiment separately.
4. Distribution: Developer ID/hardened-runtime signing and notarization script,
   optional credential-gated CI with safe failure and accurate artifact labels.
5. CI: verify actual Rust 1.87 support; locked dependency vulnerability audit,
   modern pinned Actions, automated update tracking, regression fixtures.
6. Validate: fmt, tests, strict Clippy, MSRV, audit, GPU fixtures, release build,
   targeted performance checks; record outcomes and remaining external gates.

External gates: Apple notarization credentials and CI signing secrets;
physical input-to-photon equipment and controlled power/energy experiment.
Do not equate CPU/GPU/PTY timings with those measurements.

## Implemented and verified

### Protocol

15 focused integration cases cover combined DECSET/DECRST, DECSTBM defaults,
origin-relative cursor positioning/reports, autowrap off including combining
marks at the last column, BEL/deferred wrap, ED3 history erasure, IND/NEL,
colon truecolour with an optional colour-space slot, unsupported sequence
isolation, line edits outside margins, DL without history pollution, separate
main/alternate cursor saves, SGR restoration, and arbitrary byte chunking.
Independent review additionally caught HPA/deferred-wrap and DECAWM/restore
interactions; both were reproduced then fixed. DECSC preserves saved cursor
attributes/origin/pending position, not the global DECAWM mode.

Reference: [xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).
This is a bounded subset. Advanced DCS, additional OSC features and full xterm
conformance remain outside this implementation; unsupported requests are not
advertised as implemented. OSC 52 clipboard access remains intentionally absent.

### Reliability

- Eight seeded runs x 5,000 operations = 40,000 mixed resize/Unicode/edit/history
  operations check cursor bounds, scroll regions, wide pairs and history limits.
- 20,000 unique grapheme overwrites check bounded intern-pool reclamation.
- Malformed byte/control streams recover after reset; zero dimensions clamp to
  an addressable 1x1 grid.
- 12 PTY spawn/output/exit cycles each drain 2,000 lines plus a final marker,
  retain only configured history and release parser workers.
- Dropping a PTY wakes its blocking reader via a Unix socket without idle polling,
  stops queued parsing, sends HUP to its owned child, escalates after 200 ms if
  necessary, and reaps asynchronously rather than blocking the UI. A HUP-ignoring
  child regression checks exit, reaping and release of the shared performer.
  This is owned-child cleanup, not a guarantee to kill deliberately detached jobs.

### Measurement

- Added a repeatable PTY child/parse round-trip benchmark and CPU/RSS sampler.
- Initial 500-exchange local PTY run after 30 warmups: p50 0.041 ms,
  p95 0.050 ms, p99 0.056 ms. A final portable-pty 0.9 run measured
  p50 0.041 ms, p95 0.051 ms, p99 0.070 ms; max 0.131 ms. Not GPU or display latency.
- Ten alternating parser runs per version against the v0.1.2 tagged baseline:
  median 228.4 -> 229.15 MiB/s (overlapping ranges, not a significant improvement).
  No evidence of a parser-throughput regression in this workload.
- New native Neovim validation used an isolated bundle and config: Unicode,
  statusline, splits, scrolling and zoom/unzoom were inspected; `:messages` was
  empty. User terminal sessions/configuration were not modified.
- One 60-second visible Neovim idle sample (blink disabled): 0.24 CPU seconds,
  0.40% of one core; RSS 115856-119504 KiB. Not a battery result or proof against
  every long-duration memory leak.

### Distribution

Optional credential-gated notarization is implemented; the local Developer ID
signing/hardened-runtime/timestamp path passed strict verification. Apple service
acceptance is **not** verified without credentials. See [distribution](distribution.md).

### CI / dependencies

- Modern SHA-pinned checkout/upload/download/cache/toolchain Actions.
- Read-only CI/build tokens; release publishing alone gets contents write.
- A pinned macOS-15 job checks Rust 1.87 against the locked workspace/all targets
  and runs core/config tests. This complements stable macOS/Linux CI rather than
  claiming every target is tested on the minimum compiler.
- Scheduled/PR vulnerability audit rejects vulnerabilities, unsoundness and
  yanked crates. Dependabot tracks Cargo and Actions updates.
- Targeted updates: quick-xml via wayland-scanner, anyhow, memmap2; portable-pty
  0.9 removes the unmaintained serial dependency. Disable unused muda GTK defaults
  (muda is macOS-only here), removing glib/proc-macro-error exposure from the lock.
- Audit after updates: zero vulnerabilities/unsound/yanked failures. Three visible
  unmaintained warnings remain: rustybuzz 0.12.1 and ttf-parser 0.20/0.25, inherited
  from font shaping. They are not suppressed. Migrating that shaping stack needs
  a separate glyph/ligature/performance validation pass, not a blind major update.
  [rustybuzz advisory](https://rustsec.org/advisories/RUSTSEC-2026-0206),
  [ttf-parser advisory](https://rustsec.org/advisories/RUSTSEC-2026-0192).

## Validation commands

```sh
cargo fmt --all -- --check
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +1.87.0 check --locked --workspace --all-targets
cargo +1.87.0 test --locked -p volt-core -p volt-config
cargo audit --deny unsound --deny yanked
cargo run --release --locked -p volt-renderer --example rendercheck -- target/rendercheck-hardening --fixtures-only
bash scripts/macos/build_app.sh --release
```

Local Rust 1.87 required `SDKROOT` selecting Xcode's macOS 26.5 SDK: its linker
could not parse the separately installed macOS 27 SDK's new architecture names.
With that environment correction the locked workspace check passed. This is an
old-toolchain/new-SDK setup issue, not evidence for changing the Rust MSRV.

Full test target: 121 passing tests, plus three opt-in benchmarks ignored by the
ordinary suite. Existing GPU validation: 27 font/scale/line-height cases and
60 cursor/background cases passed. Strict Clippy also passes on Rust 1.98.
Actionlint 1.7.12 validated the workflows; shell syntax checks passed. The final
app built and passed ad-hoc and Developer ID signing. Missing notarization
credentials and an invalid signing mode were both rejected as expected.

## Still external or explicitly deferred

- Notarization profile/API credentials, CI signing secrets, Apple acceptance.
- Hardware input-to-display measurement and controlled energy/power experiment
  (noninteractive powermetrics privileges were unavailable).
- Exhaustive terminal conformance and multi-day real-world reliability cannot be
  certified by a finite smoke/stress suite.
- Font-stack maintenance migration noted above; Linux UI and AI transport remain
  deliberately deferred.
