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
