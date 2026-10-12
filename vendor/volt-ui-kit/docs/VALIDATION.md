# Validation and contribution gates

Run from the repository root. Rust 1.87 remains the minimum; optional native/GPU
features must not change default/legacy widget contracts.

## Automated gates

```sh
cargo fmt --all --check
cargo test --locked
cargo test --locked --all-features --all-targets
cargo test --locked --all-features --doc
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo +1.87.0 check --locked --all-targets --all-features
RUSTDOCFLAGS='-D warnings' cargo doc --locked --all-features --no-deps
cargo check --locked --all-features --all-targets --target x86_64-unknown-linux-gnu
cargo package --locked --allow-dirty
cargo build --release --locked --all-features --example catalog
cargo audit
```

Install cross targets/toolchains explicitly if needed. Cross-compilation is not
native runtime acceptance. `--allow-dirty` permits a local pre-commit package
verification, not publication or release approval. Audit results are time-sensitive.

Run hardware/probe tests separately on a GPU-capable machine:

```sh
cargo test --locked --all-features --lib native::gpu -- --ignored --nocapture
cargo test --locked --all-features --example catalog -- --ignored --nocapture
cargo test --locked --all-features --example northbound -- --ignored --nocapture
VOLT_UI_RENDERER=gpu cargo run --release --locked --features native-gpu,native-desktop --example catalog
```

The explicit suites cover presentation alpha, fonts/ink, clips/opacity, images,
frame admission, large GPU strips and data-view frame borders. Some ignored cases
are timing probes, not correctness thresholds. See [GPU policy](GPU_RENDERING.md).

## Native acceptance

New demonstrations and acceptance use **Catalog**, not UI Lab. Check narrow/long
text, fractional DPI, Unicode, rounded borders, scroll reachability, disabled
input, pointer capture/cancellation, keyboard navigation, modal scope, rejected
updates and focus retention. Keep pixel-level regressions for renderer changes.

Separately validate real IME composition, VoiceOver/Narrator/Orca, clipboard,
trackpads, resize/monitor changes and Windows/Linux native runtime. Device-loss
recovery, notification permissions/display/removal, tray/menu delivery and
filesystem providers need their own platform witnesses.

For native glass, check desktop-through alpha/blur, light/dark appearance,
Reduce Transparency and repeated Material/Compositor switches. Window-only
screenshots do not certify the external desktop blur boundary. The reported
Material -> Compositor square-corner defect is **not certified fixed**.

## Performance evidence

Record renderer/build/device, physical dimensions/DPI, workload, warm-up count,
p50/p95, cache misses, allocations and redraws. Do not benchmark alongside builds.
Headless GPU readback time is not native presentation latency; CPU stage timing is
not GPU utilization, energy or input-to-photon latency. See [measurement](PERFORMANCE.md).

## Volt compatibility

Before an authorized integration: check the vendored snapshot hash inventory,
full terminal workspace tests, GPU/visual fixtures and measured terminal/idle
behaviour. Do not silently bless changed hashes or edit the vendored copy directly.
Toolkit/default checks alone are not a new Volt integration acceptance run.

## Known coverage limits

- API availability is not complete composite keyboard/accessibility acceptance.
  Range-thumb and combo/palette navigation limits are in [components](COMPONENTS.md).
- Private compositor blur is experimental; generic in-app GPU backdrop blur is
  separate pending work. Public Material is the normal macOS background path.
- Notification settings/actions/foreground delegates, multi-window restoration,
  full OS termination and signed distribution remain separate application work.
- Current dependency maintenance advisories and filesystem/resource limits are
  recorded in [security and performance](SECURITY_AND_PERFORMANCE.md).

Record repeatable evidence with changes, not an accumulation of per-session
Markdown reports. Local logs/bundles/snapshots belong in ignored `work/`; durable
API contracts belong in the maintained guides. Historical receipts remain in Git.
