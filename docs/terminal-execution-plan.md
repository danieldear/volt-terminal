# Volt terminal execution plan

Updated **2026-10-07**. This is a plan, not a claim that pending work has been
executed. Feature scope and usefulness live in [ROADMAP.md](ROADMAP.md).
Changes should land as small reviewable units, with evidence recorded separately
for source/tests, GPU fixtures, native UI, benchmarks and public artifacts.

## Status boundaries

- Already implemented: tabs/colors/reordering/splits, selection/autoscroll,
  themes/editor, project card, trusted current-pane tasks, search, shell hooks,
  safe links and macOS secure keyboard entry. Preserve these features.
- Graphical Settings, skippable first-run onboarding and optional bundled Meow
  are implemented in source. Configuration/native/GPU/real-shell evidence and
  compatibility boundaries are in [Settings](settings.md); public release is
  still a separate gate.
- Signing/notarization implementation and credential configuration are complete.
  Local and hosted Apple acceptance, stapling and extracted-ZIP checks passed.
  Future release mode is `notarized`; see [distribution.md](distribution.md).
- Existing public v0.1.10 is still ad-hoc signed. Publication and installation
  acceptance of a new notarized version remain separate release work.
- Recent rendering/parser optimizations are local changes with targeted evidence.
  Visible-window end-to-end acceptance is pending; do not market microbenchmark
  gains as a terminal throughput or display-latency improvement.

## P0 — Establish release gates and validate existing behavior

### A. Baseline and performance contract

- [ ] Identify the exact baseline commit/build and save its executable, config
  and hashes before further implementation. Separate already edited files from
  each new change; do not discard another contributor's work.
- [ ] Record display/backing scale, window/grid size, font, size, line height,
  scrollback, shell hooks and workspace state for each comparison.
- [ ] Run alternating baseline/new parser and offscreen-render repeats using
  `scripts/validation/compare_throughput.py` and `compare_rendering.py`.
- [ ] Run at least three visible-window termbench trials per build, preferably
  five or more, with profiling disabled and no overlapping builds/tests.
  Include closed, floating and docked workspace states.
- [ ] Measure idle CPU/RSS separately; test typing responsiveness during output.
  Confirm successful rendered frames. Locked/occluded zero-frame runs are not
  display benchmarks. Hardware input-to-photon/energy claims need separate tools.

Acceptance: target no reproducible slowdown. Investigate every consistent
regression, even below 5%; block on a reproducible >5% loss in any existing
parser/render workload. The 5% gate is not permission to trade away performance.
Keep per-workload results and ranges; aggregate wins cannot hide a losing case.

### B. Existing-feature regression matrix

- [ ] Bash/Zsh/Fish: directory changes, title updates without a custom title,
  custom-title preservation, command start/end, failure and idle transitions.
- [ ] Tasks: compact strip and expanded card; trusted/run-once/always-confirm;
  active split routing; form/edit/import; file-change invalidation; visible
  output/focus; running/success/failure. Without hooks use unknown/sent status.
- [ ] Safety: running job, alternate screen, read-only pane and detected password
  prompt reject command injection; failed writes are never recorded as success.
- [ ] Selection: multi-scroll drag, edge autoscroll, Shift-click, wide/combining
  characters, resize/reflow and copy. Search navigation retains correct rows.
- [ ] Native UI: tabs/colors/reorder/splits, menu lifetime and context-menu
  selection, focus/modal routing, font/theme/scale changes and hidden cards.
- [ ] Interactive matrix: Neovim, tmux, SSH, less, fzf and representative TUIs.
  Check remote CWDs are not interpreted as local paths or executable discovery.
- [ ] Accessibility/IME audit: composed input, dead keys, emoji insertion,
  marked-text lifecycle, screen-reader focus/navigation and password prompts.
  Record blockers as scoped P1 fixes; critical input/security defects block release.

Acceptance: reproduce suspected defects first; add regression tests for each
confirmed fix. Distinguish finite coverage from full compatibility/security proof.

### C. Release the already validated notarization pipeline

Can proceed in parallel with P1 development **after A/B pass for the release
candidate**; it does not depend on adding every planned protocol.

- [x] Implement Developer ID signing, hardened runtime, timestamp and fail-closed
  credential handling.
- [x] Validate Apple acceptance, stapling and extracted-ZIP trust locally and in
  [hosted CI](https://github.com/danieldear/volt-terminal/actions/runs/37572203978).
- [x] Enable `MACOS_SIGNING_MODE=notarized` for future release tags.
- [ ] Review/merge the candidate changes with all required CI checks. Increment
  the next version once at this release boundary; tag reviewed main only.
- [ ] Publish using the release workflow; verify actual signing-status metadata
  and checksums. Do not replace or relabel the old ad-hoc release.
- [ ] Download the public ZIP, compare SHA-256, extract, verify strict signature,
  staple and Gatekeeper assessment, then install/launch and run a smoke test
  without removing quarantine to bypass the trust check.
- [ ] Update README/site/distribution claims to match that exact public artifact.

No new credential upload or secret disclosure is required by this plan. Do not
export private keys into the repository or include them in validation reports.

## P1 — Modern TUI behavior and essential usability

Implement sequentially against the P0 baseline, not as one large protocol patch.

| Order | Change | Implementation boundary | Acceptance |
| --- | --- | --- | --- |
| 1 | Synchronized output | Core DEC 2026 state/query/reset and UI publication gate. Keep parsing/PTY draining; hold displayed changes only, with bounded timeout and recovery. Avoid new per-cell work. | Chunked/nested/repeated sequences, end/reset, timeout, resize/close, cursor blink, alternate screen and overload tests. Native TUI demo shows no partial update; disabled path passes performance gates. |
| 2 | Extended keyboard | Negotiated flags, bounded per-screen stacks and correct key/modifier/repeat/release encoding. No unsolicited change to legacy input. | Legacy byte-for-byte fixtures; opt-in negotiation, main/alternate restoration, modifiers, application shortcuts, IME/dead keys and read-only/modal routing. Native editor/TUI acceptance. |
| 3 | Command-output selection / prompt reflow | Extend existing shell marks with stable logical anchors through resize/history trimming. Never execute output to infer state. | Soft wrapping, Unicode, resize, erased/trimmed marks, multiple panes and hook-disabled fallback; accurate selected/copied output. |
| 4 | Accessibility / IME fixes | Address the P0 audit findings with a scoped native bridge; expose no hidden-input contents. | Screen-reader/manual acceptance and composition regressions beyond build success. |

See [terminal-feature-plan.md](terminal-feature-plan.md) for existing detailed
protocol boundaries. This plan changes priority, not previously implemented code.

## P2 — Repeatable workspace convenience

After P0/P1 gates hold, scope these separately:

1. **Opt-in layout restoration:** versioned atomic storage of windows, tabs,
   splits/ratios and validated local directories only. Test corruption, oversized
   state, missing paths and schema migration. Never restore commands, passwords,
   terminal output or processes; session state is not a process resume feature.
2. **Key sequences/tables:** build on existing single-chord configuration; test
   conflicts, timeouts, reload, modal focus and accidental command leakage.
3. **Linux usability:** native clipboard and appropriate desktop integration,
   with runtime acceptance on supported display systems. Design security around
   actual platform guarantees, not a promise to reproduce macOS Secure Event Input.
4. **Link hover:** reuse safe URL/span metadata; no continuous full-history scan
   or per-frame file/network work.
5. **Permissioned OSC 52:** only after threat-model review; bounded payloads,
   deny/ask/allow policy, pane attribution, stale-request cancellation and reads
   denied by default. Test spam, malformed encoding and pane changes.

## P3 — Optional capabilities, individually justified

| Feature | Benefit | Required design gate |
| --- | --- | --- |
| Ligatures | Programming-font polish | Cell-span mapping, selection/cursor correctness, grapheme fallback, toggle and no ASCII penalty. |
| Inline graphics | Image previews and plots | Bounded decode/upload/cache, decompression limits, safe data/path handling, deletion and scroll/reflow rules. |
| Quick terminal | Fast global access | Multi-display placement, shortcut conflicts, secure-input and focus/lifecycle behavior. |
| Automation | Scriptable project setup | Permission/attribution model before terminal input/output exposure. |
| Optional AI/MCP | Model sessions and useful project assistance | Explicit outbound context, provider/session transport, cancellation, truthful status and tool permissions; no execution during discovery. |

AI configuration discovery, manifest detection and filename/text search must not
be advertised as live agent context, semantic symbol search or connected tooling.
Keep ordinary shell use independent of AI credentials and services.

## Common acceptance checklist for every change

Earlier exploratory ideas are retained in the roadmap's
[Potential ideas](ROADMAP.md#potential-ideas--discovery-backlog-not-delivery-commitments)
section, not silently added to this delivery queue. Check companion-project
ownership first; Meow already supplies the user's shell prompt/statusline, so
compatibility/integration is preferred over duplicating that functionality.

```sh
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo +1.87.0 check --locked --workspace --all-targets
python3 -m unittest discover -s scripts/validation -p 'test_*.py'
```

Also run dependency/security CI, relevant GPU fixtures, native interaction checks
and the matched performance gate. Keep 16-byte cells, no new I/O/discovery or
per-cell allocations in the ASCII/SGR hot path, bounded resources and disabled-path
overhead checks. Discovery stays off the input/render loop and cancellable on
directory/pane changes. No feature or security guard is removed to win a benchmark.

Record the commit, environment, commands, results, failures, known limitations and
release status for each unit. Passing compilation alone does not complete a feature.
No calendar promises: each stage advances only after its acceptance gates pass.
