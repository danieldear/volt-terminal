//! Compile once at configuration load; keyboard dispatch performs no parsing.
use volt_config::keybindings::{Action, KeyBinding};
use winit::keyboard::{KeyCode, PhysicalKey};
#[derive(Default)]
pub struct Bindings(Vec<(KeyCode, u8, Action)>);
impl Bindings {
    pub fn compile(input: &[KeyBinding]) -> Self {
        Self(
            input
                .iter()
                .map(|b| (key_code(&b.key.key), b.key.modifiers, b.action))
                .collect(),
        )
    }
    pub fn lookup(
        &self,
        key: PhysicalKey,
        ctrl: bool,
        alt: bool,
        shift: bool,
        command: bool,
    ) -> Option<Action> {
        let PhysicalKey::Code(key) = key else {
            return None;
        };
        let modifiers = u8::from(ctrl)
            | (u8::from(alt) << 1)
            | (u8::from(shift) << 2)
            | (u8::from(command) << 3);
        self.0
            .iter()
            .rev()
            .find(|(k, m, _)| *k == key && *m == modifiers)
            .map(|(_, _, a)| *a)
    }
}
/// Text inputs and the keyboard-focused card keep their established handlers.
/// A custom printable/Enter binding must not run while editing a query/title.
pub fn terminal_owns_keys(
    prompt: bool,
    search: bool,
    card_visible: bool,
    card_focused: bool,
) -> bool {
    !(prompt || search || card_visible && card_focused)
}

fn key_code(s: &str) -> KeyCode {
    use KeyCode::*;
    const LETTERS: [KeyCode; 26] = [
        KeyA, KeyB, KeyC, KeyD, KeyE, KeyF, KeyG, KeyH, KeyI, KeyJ, KeyK, KeyL, KeyM, KeyN, KeyO,
        KeyP, KeyQ, KeyR, KeyS, KeyT, KeyU, KeyV, KeyW, KeyX, KeyY, KeyZ,
    ];
    const DIGITS: [KeyCode; 10] = [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ];
    const FUNCTIONS: [KeyCode; 24] = [
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20,
        F21, F22, F23, F24,
    ];
    if s.len() == 1 {
        let b = s.as_bytes()[0];
        if b.is_ascii_lowercase() {
            return LETTERS[(b - b'a') as usize];
        }
        if b.is_ascii_digit() {
            return DIGITS[(b - b'0') as usize];
        }
    }
    if let Some(n) = s.strip_prefix('f').and_then(|s| s.parse::<usize>().ok()) {
        if (1..=24).contains(&n) {
            return FUNCTIONS[n - 1];
        }
    }
    match s {
        "enter" => Enter,
        "escape" => Escape,
        "tab" => Tab,
        "space" => Space,
        "backspace" => Backspace,
        "delete" => Delete,
        "home" => Home,
        "end" => End,
        "pageup" => PageUp,
        "pagedown" => PageDown,
        "left" => ArrowLeft,
        "right" => ArrowRight,
        "up" => ArrowUp,
        "down" => ArrowDown,
        "equal" => Equal,
        "minus" => Minus,
        "comma" => Comma,
        "period" => Period,
        "slash" => Slash,
        "semicolon" => Semicolon,
        "quote" => Quote,
        "backquote" => Backquote,
        "bracketleft" => BracketLeft,
        "bracketright" => BracketRight,
        "backslash" => Backslash,
        _ => unreachable!("validated key chord"),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editable_surfaces_keep_keys_and_hidden_card_does_not_steal_them() {
        for prompt in [false, true] {
            for search in [false, true] {
                for visible in [false, true] {
                    for focused in [false, true] {
                        assert_eq!(
                            terminal_owns_keys(prompt, search, visible, focused),
                            !(prompt || search || (visible && focused))
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn override_uses_last_binding_and_exact_modifiers() {
        let bindings = Bindings::compile(&[
            KeyBinding {
                key: "cmd+f".to_string().try_into().unwrap(),
                action: Action::Find,
            },
            KeyBinding {
                key: "cmd+f".to_string().try_into().unwrap(),
                action: Action::Unbind,
            },
        ]);
        assert_eq!(
            bindings.lookup(PhysicalKey::Code(KeyCode::KeyF), false, false, false, true),
            Some(Action::Unbind)
        );
        assert_eq!(
            bindings.lookup(PhysicalKey::Code(KeyCode::KeyF), true, false, false, true),
            None
        );
        assert_eq!(
            bindings.lookup(PhysicalKey::Code(KeyCode::KeyF), false, false, true, true),
            None
        );
    }
    #[test]
    fn all_accepted_names_compile() {
        for key in [
            "a",
            "z",
            "0",
            "9",
            "f1",
            "f24",
            "enter",
            "escape",
            "space",
            "backslash",
            "bracketright",
            "pageup",
        ] {
            let _ = Bindings::compile(&[KeyBinding {
                key: key.to_string().try_into().unwrap(),
                action: Action::Ignore,
            }]);
        }
    }
}

use std::collections::HashMap;
use std::time::{Duration, Instant};
use volt_config::keybindings::{AdvancedBindingsConfig, BindingTarget};

#[derive(Default)]
struct SequenceNode {
    edges: HashMap<(KeyCode, u8), usize>,
    target: Option<CompiledTarget>,
}
#[derive(Clone, Copy)]
enum CompiledTarget {
    Action(Action),
    Table(usize),
}

/// Compile once on config load. No strings, parsing, or allocation during dispatch.
pub struct AdvancedBindings {
    nodes: Vec<SequenceNode>,
    timeout: Duration,
}
impl AdvancedBindings {
    pub fn compile(config: &AdvancedBindingsConfig) -> Result<Self, String> {
        config.validate()?;
        let roots: HashMap<_, _> = config
            .tables
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.as_str(), i + 1))
            .collect();
        let mut nodes: Vec<_> = (0..=config.tables.len())
            .map(|_| SequenceNode::default())
            .collect();
        for (root, bindings) in std::iter::once(&config.bindings)
            .chain(config.tables.iter().map(|t| &t.bindings))
            .enumerate()
        {
            for binding in bindings {
                let mut node = root;
                for chord in &binding.sequence {
                    let key = (key_code(&chord.key), chord.modifiers);
                    node = if let Some(next) = nodes[node].edges.get(&key) {
                        *next
                    } else {
                        let next = nodes.len();
                        nodes.push(SequenceNode::default());
                        nodes[node].edges.insert(key, next);
                        next
                    };
                }
                nodes[node].target = Some(match &binding.target {
                    BindingTarget::Action(a) => CompiledTarget::Action(*a),
                    BindingTarget::Table(name) => CompiledTarget::Table(roots[name.as_str()]),
                });
            }
        }
        Ok(Self {
            nodes,
            timeout: Duration::from_millis(config.timeout_ms),
        })
    }
}

/// Change focus_epoch whenever the focused pane/window/surface changes, even
/// between two eligible terminals. Set eligible=false for modal/text/IME input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DispatchContext {
    pub focus_epoch: u64,
    pub eligible: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelReason {
    Escape,
    Mismatch,
    Timeout,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchResult {
    /// Only this result permits legacy bindings / normal terminal input.
    PassThrough,
    Pending,
    Action(Action),
    /// The current press AND prior prefix are consumed; never replay either.
    Cancelled(CancelReason),
    /// Auto-repeat of a consumed press must not advance the sequence.
    Consumed,
}
impl DispatchResult {
    /// Host convenience for gating fallback; retained for integration callers.
    #[allow(dead_code)] // The current internal host matches variants directly.
    pub fn is_consumed(self) -> bool {
        self != Self::PassThrough
    }
}

/// Owns compiled bindings so reload cannot leave stale trie indices.
/// Tables are exclusive, one-shot, non-stacking; table-to-table activation replaces
/// the active table. Completion, mismatch, timeout, Escape, or reset returns idle.
pub struct KeyboardDispatcher {
    bindings: AdvancedBindings,
    node: usize,
    deadline: Option<Instant>,
    context: Option<DispatchContext>,
}
impl KeyboardDispatcher {
    pub fn new(bindings: AdvancedBindings) -> Self {
        Self {
            bindings,
            node: 0,
            deadline: None,
            context: None,
        }
    }
    pub fn reset(&mut self) {
        self.node = 0;
        self.deadline = None;
    }
    pub fn replace_bindings(&mut self, bindings: AdvancedBindings) {
        self.reset();
        self.bindings = bindings;
    }
    /// Call on focus/modal transitions, not just on the next key event.
    pub fn set_context(&mut self, context: DispatchContext) {
        if self.context != Some(context) || !context.eligible {
            self.reset();
        }
        self.context = Some(context);
    }
    /// O(1), None while idle: hosts need no idle polling or periodic timer.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }
    /// O(1). Expiration discards the prefix without producing input or actions.
    pub fn expire(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|d| now >= d) {
            self.reset();
            true
        } else {
            false
        }
    }
    /// Feed key presses only. The host suppresses release events for consumed
    /// presses; auto-repeat may trigger single-step root actions, never sequences.
    pub fn dispatch(
        &mut self,
        key: PhysicalKey,
        modifiers: u8,
        repeat: bool,
        now: Instant,
        context: DispatchContext,
    ) -> DispatchResult {
        self.set_context(context);
        if !context.eligible {
            return DispatchResult::PassThrough;
        }
        if self.expire(now) {
            return DispatchResult::Cancelled(CancelReason::Timeout);
        }
        let active = self.deadline.is_some();
        if active && key == PhysicalKey::Code(KeyCode::Escape) {
            self.reset();
            return DispatchResult::Cancelled(CancelReason::Escape);
        }
        if active && repeat {
            return DispatchResult::Consumed;
        }
        let next = match key {
            PhysicalKey::Code(code) => self.bindings.nodes[self.node]
                .edges
                .get(&(code, modifiers))
                .copied(),
            _ => None,
        };
        let Some(next) = next else {
            if active {
                self.reset();
                return DispatchResult::Cancelled(CancelReason::Mismatch);
            }
            return DispatchResult::PassThrough;
        };
        match self.bindings.nodes[next].target {
            Some(CompiledTarget::Action(action)) => {
                self.reset();
                DispatchResult::Action(action)
            }
            target => {
                // A repeat must not start a prefix or enter a table from idle.
                if repeat {
                    return DispatchResult::Consumed;
                }
                self.node = match target {
                    Some(CompiledTarget::Table(root)) => root,
                    _ => next,
                };
                self.deadline = Some(now + self.bindings.timeout);
                DispatchResult::Pending
            }
        }
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    use volt_config::keybindings::{KeyTable, SequenceBinding};
    const CTX: DispatchContext = DispatchContext {
        focus_epoch: 1,
        eligible: true,
    };
    fn binding(keys: &[&str], target: BindingTarget) -> SequenceBinding {
        SequenceBinding {
            sequence: keys
                .iter()
                .map(|s| s.to_string().try_into().unwrap())
                .collect(),
            target,
        }
    }
    fn engine() -> KeyboardDispatcher {
        let c = AdvancedBindingsConfig {
            timeout_ms: 100,
            bindings: vec![
                binding(&["ctrl+k", "c"], BindingTarget::Action(Action::Copy)),
                binding(&["ctrl+p"], BindingTarget::Table("pane".into())),
                binding(&["ctrl+f"], BindingTarget::Action(Action::Find)),
            ],
            tables: vec![KeyTable {
                name: "pane".into(),
                bindings: vec![
                    binding(&["h"], BindingTarget::Action(Action::FocusLeft)),
                    binding(&["j", "j"], BindingTarget::Action(Action::FocusDown)),
                    binding(&["p"], BindingTarget::Table("pane".into())),
                ],
            }],
        };
        KeyboardDispatcher::new(AdvancedBindings::compile(&c).unwrap())
    }
    fn press(d: &mut KeyboardDispatcher, k: KeyCode, m: u8, now: Instant) -> DispatchResult {
        d.dispatch(PhysicalKey::Code(k), m, false, now, CTX)
    }
    #[test]
    fn sequence_exact_modifiers_and_mismatch_never_retry_root() {
        let mut d = engine();
        let now = Instant::now();
        assert_eq!(
            press(&mut d, KeyCode::KeyK, 5, now),
            DispatchResult::PassThrough
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyK, 1, now),
            DispatchResult::Pending
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, now),
            DispatchResult::Action(Action::Copy)
        );
        assert!(d.deadline().is_none());
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 1, now),
            DispatchResult::Cancelled(CancelReason::Mismatch)
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(
            press(&mut d, KeyCode::KeyF, 1, now),
            DispatchResult::Cancelled(CancelReason::Mismatch)
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyF, 1, now),
            DispatchResult::Action(Action::Find)
        );
    }
    #[test]
    fn tables_are_exclusive_one_shot_and_replace_without_stack() {
        let mut d = engine();
        let now = Instant::now();
        assert_eq!(
            press(&mut d, KeyCode::KeyH, 0, now),
            DispatchResult::PassThrough
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyP, 1, now),
            DispatchResult::Pending
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyP, 0, now),
            DispatchResult::Pending
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyH, 0, now),
            DispatchResult::Action(Action::FocusLeft)
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyH, 0, now),
            DispatchResult::PassThrough
        );
        press(&mut d, KeyCode::KeyP, 1, now);
        assert_eq!(
            press(&mut d, KeyCode::KeyJ, 0, now),
            DispatchResult::Pending
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyJ, 0, now),
            DispatchResult::Action(Action::FocusDown)
        );
        press(&mut d, KeyCode::KeyP, 1, now);
        assert_eq!(
            press(&mut d, KeyCode::KeyF, 1, now),
            DispatchResult::Cancelled(CancelReason::Mismatch)
        );
    }
    #[test]
    fn timeout_boundary_escape_and_reset() {
        let mut d = engine();
        let now = Instant::now();
        assert!(!d.expire(now));
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(d.deadline(), Some(now + Duration::from_millis(100)));
        assert!(!d.expire(now + Duration::from_millis(99)));
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, now + Duration::from_millis(100)),
            DispatchResult::Cancelled(CancelReason::Timeout)
        );
        press(&mut d, KeyCode::KeyP, 1, now);
        assert_eq!(
            press(&mut d, KeyCode::Escape, 15, now),
            DispatchResult::Cancelled(CancelReason::Escape)
        );
        assert_eq!(
            press(&mut d, KeyCode::Escape, 0, now),
            DispatchResult::PassThrough
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        assert!(d.expire(now + Duration::from_millis(100)));
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, now),
            DispatchResult::PassThrough
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        d.reset();
        assert!(d.deadline().is_none());
    }
    #[test]
    fn focus_modal_reload_repeat_and_unidentified_keys() {
        let mut d = engine();
        let now = Instant::now();
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(
            d.dispatch(
                PhysicalKey::Code(KeyCode::KeyC),
                0,
                false,
                now,
                DispatchContext {
                    focus_epoch: 2,
                    ..CTX
                }
            ),
            DispatchResult::PassThrough
        );
        press(&mut d, KeyCode::KeyP, 1, now);
        assert_eq!(
            d.dispatch(
                PhysicalKey::Code(KeyCode::KeyH),
                0,
                false,
                now,
                DispatchContext {
                    eligible: false,
                    ..CTX
                }
            ),
            DispatchResult::PassThrough
        );
        assert!(d.deadline().is_none());
        d.set_context(CTX);
        assert_eq!(
            d.dispatch(PhysicalKey::Code(KeyCode::KeyK), 1, true, now, CTX),
            DispatchResult::Consumed
        );
        assert!(d.deadline().is_none());
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(
            d.dispatch(PhysicalKey::Code(KeyCode::KeyC), 0, true, now, CTX),
            DispatchResult::Consumed
        );
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, now),
            DispatchResult::Action(Action::Copy)
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        assert_eq!(
            d.dispatch(
                PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
                0,
                false,
                now,
                CTX
            ),
            DispatchResult::Cancelled(CancelReason::Mismatch)
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        d.replace_bindings(AdvancedBindings::compile(&AdvancedBindingsConfig::default()).unwrap());
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, now),
            DispatchResult::PassThrough
        );
    }
    #[test]
    fn compile_revalidates_programmatic_config() {
        assert!(!DispatchResult::PassThrough.is_consumed());
        assert!(DispatchResult::Cancelled(CancelReason::Mismatch).is_consumed());
        assert!(DispatchResult::Action(Action::Unbind).is_consumed());
        let c = AdvancedBindingsConfig {
            timeout_ms: 0,
            ..Default::default()
        };
        assert!(AdvancedBindings::compile(&c).is_err());
    }
    #[test]
    fn shared_prefix_siblings_and_progress_deadline() {
        let config = AdvancedBindingsConfig {
            timeout_ms: 100,
            bindings: vec![
                binding(&["ctrl+k", "c", "a"], BindingTarget::Action(Action::Copy)),
                binding(&["ctrl+k", "c", "b"], BindingTarget::Action(Action::Paste)),
            ],
            ..Default::default()
        };
        let mut d = KeyboardDispatcher::new(AdvancedBindings::compile(&config).unwrap());
        let now = Instant::now();
        assert_eq!(
            press(&mut d, KeyCode::KeyK, 17, now),
            DispatchResult::PassThrough
        );
        press(&mut d, KeyCode::KeyK, 1, now);
        let later = now + Duration::from_millis(90);
        assert_eq!(
            press(&mut d, KeyCode::KeyC, 0, later),
            DispatchResult::Pending
        );
        assert_eq!(d.deadline(), Some(later + Duration::from_millis(100)));
        assert_eq!(
            press(&mut d, KeyCode::KeyB, 0, later),
            DispatchResult::Action(Action::Paste)
        );
    }
}
