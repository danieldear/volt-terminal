# Volt roadmap

Volt is an early public preview (source version: **0.1.11**). This page lists
what works today and what comes next. Status reconciled on **2026-10-07** against
the current source, release metadata and hosted signing validation. Order within a stage is a rough priority,
not a promise or a date. Settings and shortcuts may change before 1.0.

The engineering acceptance gates for each terminal feature (tests, GPU checks,
throughput budget) live in [terminal-feature-plan.md](terminal-feature-plan.md).
A feature moves to "Available today" only when it ships in a release.
The concrete delivery sequence and acceptance criteria are in
[the execution plan](terminal-execution-plan.md). Implemented, validated and
publicly released are separate milestones; a pending check is not missing code.

## Available today

| Area | What works |
| --- | --- |
| Rendering | GPU rendering through wgpu, with a cached glyph atlas and Nerd Font fallback. |
| Tabs and splits | Tabs, horizontal and vertical splits, colored and reorderable tabs. New tabs and splits open in the current folder. |
| Search | ⌘F searches the terminal's full scrollback, plus files and folders in the current project. |
| Project tasks | Build, test and run commands saved in `.volt/tasks.toml`, shown as buttons and run in the current pane. Untrusted task files need review before running. |
| Workspace card | The current folder, Git status and project tasks in one panel (⌘⇧A). |
| Appearance | Built-in themes and a live theme editor. |
| Shell integration | Opt-in Bash, Zsh and Fish hooks: prompt navigation and command/task completion signals. Status depends on integration being active. |
| Links | ⌘-click for plain HTTP(S) links and OSC 8 hyperlinks, with the destination shown before opening. |
| Configuration | `config.toml`, reloaded with ⌘⇧R, single-chord shortcut overrides, and an editor per file type. |
| Security | Automatic/manual secure keyboard entry on macOS, with an indicator when the OS accepts it. This is not output redaction, clipboard protection or universal password-prompt detection. |

## Already implemented; remaining acceptance or publication

| Area | Completed evidence | What remains |
| --- | --- | --- |
| macOS signing and notarization | Developer ID signing, hardened runtime, timestamp, Apple acceptance, stapling and extracted-ZIP verification passed locally and in [hosted validation](https://github.com/danieldear/volt-terminal/actions/runs/37572203978). Repository mode is `notarized` for future releases. | Publish a new reviewed version and test its downloaded public ZIP. Existing [v0.1.10](https://github.com/danieldear/volt-terminal/releases/tag/v0.1.10) is still ad-hoc signed; it was not replaced or relabeled. **Do not redo credential setup.** |
| Rendering optimizations | Local ASCII/extended shape caches and task-strip geometry reuse passed targeted CPU/GPU checks. | Review/merge the local changes; perform matched visible-window benchmarks before claiming a whole-app speedup. See [measurement evidence](performance-intelligent-rendering.md). |
| Project tasks and tab status | Current-pane execution, task import/form, trust review and shell completion handling exist. | Verify directory changes, status transitions and all card/strip routes across supported shells; fix demonstrated regressions rather than rebuild the feature. |

See [distribution evidence](distribution.md) for the distinction between a
validated signing pipeline and a downloadable notarized public release.

### Platforms

| Platform | Status |
| --- | --- |
| macOS 13 and later | Supported. Apple silicon builds; Intel Macs can build from source. |
| Linux (x86_64) | Partial. Clipboard, menu bar and secure keyboard entry are macOS-only for now. |
| Windows | Not planned. |

## Next up

- **Daily-driver regression coverage**: selection/autoscroll, resize/reflow,
  Unicode/emoji/IME, tabs/splits, tasks and status through Neovim, tmux and SSH.
  Accessibility is foundational acceptance work, not merely a cosmetic extra.
- **Performance acceptance**: matched visible-window throughput, input
  responsiveness, idle CPU/RSS and workspace-card on/off measurements. Do not
  infer "fastest" from unmatched fonts, grids or parser-only results.
- **Notarized public release**: release and download/launch verification of the
  already validated pipeline; no new signing implementation is required.
- **Synchronized output and extended keyboard**: prioritize these modern TUI
  capabilities after the baseline regression/performance gates are established.
- **Linux usability**: clipboard integration and platform-appropriate desktop
  behavior. Do not promise macOS Secure Event Input parity on Linux; its threat
  model and compositor support require a separate design.
- **Select command output**: select a whole command's output in one action,
  using shell-integration marks. Prompt marks should also survive reflow.
- **Key sequences and key tables**: multi-key bindings on top of today's
  single-chord shortcuts.
- **Link hover**: highlight a whole link on hover before ⌘-click.

## Planned

- **Synchronized output** (DEC mode 2026): full-screen programs redraw in a
  single frame. A timeout release and resize recovery prevent a stuck screen.
- **Extended keyboard protocol**: precise key, modifier, repeat and release
  reporting for editors and TUIs that request it. Legacy input is unchanged
  when it is off.
- **Clipboard for programs** (OSC 52): copy requests with deny, ask or allow
  policies per pane. Clipboard reads are denied by default, and payloads are
  bounded.
- **Layout restoration** (opt-in): reopen windows, tabs, splits, split sizes
  and local folders. Commands, terminal output, passwords and running
  processes are never restored.

## Later

- **Programming ligatures**: shaped text runs that keep the cursor and
  selection aligned to cells, with no cost for plain ASCII.
- **Inline images**: an inline-image protocol, with bounded decoding,
  uploads and memory. Untrusted file paths are rejected.
- **Quick terminal**: a drop-down terminal on a global shortcut.
- **Automation**: scripting Volt's windows, tabs and panes.
- **Optional AI**: real provider/session transport, attachments, accurate context
  and permissioned MCP tooling. Configuration discovery is not a connected agent.

## Feature usefulness and priorities

Priority is about user impact, not feature count. **No matched speed ranking has
been established.** Completed features below are maintenance/acceptance work,
not instructions to reimplement them.

- **P0 — Release gates:** correctness, security, performance baseline and public
  artifact acceptance. Block release when a reproducible critical defect exists.
- **P1 — Next core work:** modern TUI behavior and important usability fixes.
- **P2 — Workflow expansion:** useful convenience after the core gates pass.
- **P3 — Optional:** defer until there is a demonstrated use case and budget.

| Priority | Feature | What it is useful for | Is it required? | Current status / next action |
| --- | --- | --- | --- | --- |
| P0 | Terminal compatibility | Correct cursor, colors, modes, scrolling and application control sequences. | Common behavior is essential; every extension is not. | Common ANSI/xterm subset implemented. Extend regression coverage and fix confirmed defects; do not claim full conformance. |
| P0 | Performance | Output throughput, responsive typing, consistent redraws and low idle resource use. | Essential without removing features or safety checks. | Local cache improvements tested; matched visible-window verification pending. |
| P0 | Project task and tab reliability | Run commands in the selected pane and show accurate directory, busy, success and failure state. | Essential to existing workflows. | Implemented. Validate all card/strip routes and supported shells; without completion hooks show unknown/sent, never invented success. |
| P0 | Notarized public distribution | Trusted macOS installation and accurately labeled downloads. | Essential for public adoption. | Pipeline implemented and validated. Publish/test a new public release; do not repeat credential setup. |
| P0 audit; P1 fixes | Accessibility / IME | Screen-reader navigation and correct multilingual input composition. | Essential usability work. | Audit native behavior and fix demonstrated blockers. Emoji insertion does not establish full IME support. |
| P0 maintain | Graphical Settings and skippable first-run setup | Configure appearance, shell/prompt, terminal behavior and shortcuts without editing TOML; learn tasks and navigation. | Faster, safer setup for new users while keeping manual configuration. | Implemented in source; native/GPU/config/shell checks documented in [Settings](settings.md). Included starting with v0.1.11; not in older v0.1.10 archives. |
| P0 maintain | Tabs, splits and search | Organize shells/editors and find project files or retained terminal output. | Already part of the product. | Implemented. Preserve selection, focus, modal routing and cache invalidation through changes. |
| P1 | Synchronized output (DEC 2026) | Present a bracketed TUI screen update together instead of exposing partial redraws. | Important for modern TUIs. | Not implemented. Add bounded timeout and resize/close recovery. |
| P1 | Extended keyboard reporting | Unambiguous modifiers and negotiated press/repeat/release events. | Important for modern editors/TUIs. | Not implemented. Preserve legacy defaults, IME and application shortcuts. |
| P1 | Prompt/output navigation | Select a command's output and retain prompt anchors after reflow. | Useful shell-integration completion. | Prompt navigation exists; command-output selection and anchor-preserving reflow remain. |
| P2 | Session/layout restoration | Reopen windows, pane arrangement and local directories, not processes. | Useful everyday convenience. | Not implemented. Opt-in, atomic/versioned state with bounded validation. |
| P2 | Key sequences and key tables | Context-specific and multi-key shortcuts beyond a single chord. | Useful for configurable workflows. | Single-chord overrides exist; sequences/tables remain. |
| P2 | Linux usability | Clipboard integration and appropriate desktop behavior. | Needed before claiming platform parity. | Release archives exist; clipboard/desktop acceptance remains. macOS Secure Event Input is not a portable Linux guarantee. |
| P2 | Link hover | Make the target and full clickable span clear before opening a URL. | Useful interaction polish. | Safe click handling exists; whole-link hover grouping remains. |
| P2, security-gated | OSC 52 clipboard | Allow terminal applications to request clipboard operations. | Optional and sensitive. | Not implemented. Future policy must bound requests, attribute panes and deny reads by default. |
| P3 | Programming ligatures | Combined programming-font shapes without changing underlying text. | Optional polish. | Shaped-run support missing; keep cell/cursor/selection alignment and disableable behavior. |
| P3 | Inline images | Show image previews and plots requested by terminal applications. | Optional specialized capability. | Not implemented. Resource limits, safe decoding and path restrictions come first. |
| P3 | Automation | Programmatic control of windows, tabs, panes and input. | Optional integration surface. | No general terminal-control API. Design attribution and permissions before exposing access. |
| P3 | Quick terminal | Bring up a small terminal with a global shortcut. | Optional convenience. | Not implemented; must handle focus, shortcuts and multiple displays. |
| P3 | AI providers and tooling | Actual model sessions, attachments, context and permissioned tool execution. | Optional strategic direction. | Configuration/rules discovery exists; real agent/session transport is not implemented. |

## Potential ideas — discovery backlog, not delivery commitments

These preserve earlier product discussions without adding them to the P0–P3
execution queue. **Priority is TBD** until usefulness, ownership, security and
performance costs are understood. A potential idea is not a missing release
requirement, an implemented feature, or a promise to build it into Volt.

Some functionality belongs in existing companion projects. **Meow is already
used as the shell prompt/statusline solution.** Prefer compatibility or an
explicit integration over duplicating its rendering, configuration or logic.
The shell owns the prompt; Volt owns terminal behavior and optional workspace UI.
Other companion-project ownership should be checked before implementation.

| Potential idea | Usefulness | Existing capability / boundary | Decision needed before scheduling |
| --- | --- | --- | --- |
| Tree-sitter parsing | Optional syntax previews and syntax-level symbol extraction. | Project detection already uses manifests and bounded file sampling; parsing is not required just to recognize a workspace. | Choose languages and incremental/resource bounds; distinguish syntax extraction from language-server semantics. |
| Semantic symbol search | Find definitions, symbols and references beyond filename/text matching. | Current search covers retained output, filenames, saved text, Git metadata and tasks—not semantic analysis. | Decide syntax-only versus language-server support, indexing scope and ownership; never claim access to undrawn editor buffers. |
| GitHub, branch and worktree actions | Create/switch worktrees or branches and review/create PRs without leaving the workflow. | Status, branches, existing worktrees and PR/check summaries are informational today; opening an existing worktree is not creating one. | Determine whether Volt or a companion tool should own mutations; require explicit action, scope checks, dirty-tree protection and clear authentication boundaries. |
| Local-model/provider adapters | Show genuine availability, selected model and session state for Ollama, LM Studio or other providers. | Installation/configuration discovery does not prove connection or use. | Integrate existing tooling where possible; probe only configured endpoints off-thread with timeouts and user control. |
| Agent enablement and configuration | Understand which agent is enabled for a project and manage configuration deliberately. | Rules/config discovery exists; it neither starts an agent nor establishes a live session. | Choose adapter ownership and scoped configuration controls; avoid accidental startup or credential disclosure. |
| Actual AI context and MCP activity | Show attached/sent files, context/token usage and connected or invoked tools accurately. | Configuration/rules/MCP-name discovery is read-only, not evidence of agent-loaded context or runtime activity. | Require real session events, explicit outbound context and redaction/permissions; no inferred or fabricated state. |
| Remote connection details | Identify a remote session/host and its context when useful. | Local-shell CWD discovery is not reliable remote-session introspection. | Define trusted session metadata and opt-in adapters; do not treat remote paths as local or run commands to discover state automatically. |
| ADB/device information | Show connected devices and selected development targets. | No device adapter is scoped in the execution plan. | Prefer existing device tooling; detect only when relevant and enabled, with bounded refresh and explicit actions. |
| Debugger/watch variables | Inspect values during a real debugging session. | Ordinary terminal output does not provide reliable debugger state. | Assess demand and an explicit debugger adapter; avoid rebuilding an editor/debugger UI without a clear benefit. |
| Multi-step workflows | Task dependencies, cancellation, execution history and useful progress. | Saved current-pane commands already exist; they are not a dependency-aware workflow engine. | Determine whether a companion orchestrator owns this; preserve visible execution, terminal safety and explicit consent. |
| Plugins/hooks | Extend behavior without expanding the core application for every integration. | No general extension API or permission model exists. | Define a small stable surface, trust/sandbox model, lifecycle, compatibility and resource limits before exposing terminal access. |
| Prompt/statusline integration with Meow | Offer a lightweight bundled default while allowing users to retain any existing prompt. | Bundled adaptation and Bash/Zsh/Fish setup are now implemented in source; existing users remain opted out. | Continue preserving custom prompts. Shared metadata is optional future work; a duplicate terminal-owned statusline is not scheduled. |
| Cargo-installable application | Offer a familiar source-install route. | Source builds exist; that does not establish registry publication readiness. | Review package name, manifests, dependency publication, license/package contents, platform requirements and install tests; clarify CLI binary versus native app bundle. |
| Automatic updates / package-manager distribution | Simplify installation and ongoing updates. | Versioned release archives/checksums and notarization infrastructure already exist. | Select channels only after public artifact acceptance; verify update authenticity, permissions, rollback and supported architectures. |

To promote an idea into the execution plan: identify its owner (Volt, Meow or
another companion), show a concrete user workflow, assign a priority, define
security/performance acceptance and deliver it as a separately reviewed unit.
Potential ideas do not delay terminal correctness or the validated release path.

## Not planned

- Windows support.
- Executing anything from terminal output to discover state, such as a
  pane's working directory.

## Feedback

Missing something, or want a feature moved up?
[Open an issue on GitHub](https://github.com/danieldear/volt-terminal/issues).
