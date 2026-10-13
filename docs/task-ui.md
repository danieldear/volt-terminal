# Task UI

Task execution and permissions remain owned by Volt. The toolkit supplies the
shared theme, rounded surfaces, vector icons and buttons; the existing renderer
hosts them without enabling another native window or a continuously running UI
GPU renderer.

## Compact workspace view

The Tasks section lists saved tasks and their actual run status. Commands and
working-directory previews no longer occupy separate, non-actionable rows.
Add task, Edit tasks file and collapsed suggestions remain available. An
untrusted file offers Review permissions instead of silently granting trust.

## Review dialog

- **Run once:** one-time permission for an untrusted task; no persistent trust.
- **Run task:** asks on each launch when `confirm = true`, even for a trusted file.
- **Trust tasks:** reviews every configured command, directory and confirmation
  policy, then approves the exact tasks-file bytes. It does not execute anything.

The configured command and directory are separate read-only fields. Long
commands wrap and scroll without discarding the end. Directional formatting and
control characters are represented visibly. Mouse wheel, Up/Down, Page Up/Down
and Home/End scroll. Tab switches button focus, Enter activates the focused
button, and Escape cancels. File-wide trust initially focuses Cancel; single-run
reviews retain Enter-to-run. A window too small to show the dialog cannot approve
it via keyboard.

Task-file changes invalidate trust and pending approval. Switching terminals
invalidates pending approval. Launch still re-reads the file and applies the
busy-shell, alternate-screen, remote, and secure-input guards.

## Performance and future sidebar

Review geometry and shaped text are cached until content, focus, scroll, backing
scale, viewport, font atlas or theme changes. When closed, there is no review
layout, text shaping, polling or GPU work. No new runtime dependency is added.
Whole-app benchmark acceptance remains a separate test.

`volt-ui/src/tasks.rs` provides task review view-models, while
`volt-renderer/src/task_review.rs` defines presentation geometry and hit testing.
This allows a future Workspace / Tasks / AI sidebar to reuse task data and
execution guards. The complete sidebar and AI integration are not implemented
by this change; the floating/docked workspace card remains available.

## Validation

```sh
RUST_MIN_STACK=33554432 cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --features volt/ui-gpu-prototype -- -D warnings
cargo run --locked --release -p volt-renderer --example taskcheck
```

The offscreen fixture checks modal occlusion, steady-state cached pixels, theme
behavior and clean dismissal at 1× / 2× backing scales. Native testing additionally
checks task-button launch, cancellation, file approval, `confirm = true`, direct
trusted launch and completion status in the real PTY.
