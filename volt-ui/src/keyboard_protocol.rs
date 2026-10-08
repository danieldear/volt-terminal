//! Opt-in encoder; None means use Volt's unchanged legacy encoder.
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolKey {
    pub code: u32,
    pub suffix: char,
    pub shifted: Option<u32>,
    pub keypad: Option<u32>,
    pub base_layout: Option<u32>,
}
impl ProtocolKey {
    /// IME/picker commits have no physical key or reliable modifier state.
    /// Applications requesting all-key reporting receive the defined pure
    /// text event (key 0); otherwise UTF-8 remains on the legacy path.
    pub fn committed_text(flags: u16, text: &str) -> Option<Vec<u8>> {
        if flags & 8 == 0 || text.is_empty() {
            return None;
        }
        Self {
            code: 0,
            suffix: 'u',
            shifted: None,
            keypad: None,
            base_layout: None,
        }
        .encode(flags, 0, 1, Some(text))
    }
    pub fn from_event(base: &Key, logical: &Key, physical: PhysicalKey) -> Option<Self> {
        let shifted = match logical {
            Key::Character(s) if s.chars().count() == 1 => s.chars().next().map(u32::from),
            _ => None,
        };
        let (code, suffix) = match base {
            Key::Character(s) if s.chars().count() == 1 => (u32::from(s.chars().next()?), 'u'),
            Key::Named(n) => match n {
                NamedKey::Escape => (27, 'u'),
                NamedKey::Enter => (13, 'u'),
                NamedKey::Tab => (9, 'u'),
                NamedKey::Backspace => (127, 'u'),
                NamedKey::ArrowUp => (1, 'A'),
                NamedKey::ArrowDown => (1, 'B'),
                NamedKey::ArrowRight => (1, 'C'),
                NamedKey::ArrowLeft => (1, 'D'),
                NamedKey::Home => (1, 'H'),
                NamedKey::End => (1, 'F'),
                NamedKey::Insert => (2, '~'),
                NamedKey::Delete => (3, '~'),
                NamedKey::PageUp => (5, '~'),
                NamedKey::PageDown => (6, '~'),
                NamedKey::F1 => (1, 'P'),
                NamedKey::F2 => (1, 'Q'),
                NamedKey::F3 => (13, '~'),
                NamedKey::F4 => (1, 'S'),
                NamedKey::F5 => (15, '~'),
                NamedKey::F6 => (17, '~'),
                NamedKey::F7 => (18, '~'),
                NamedKey::F8 => (19, '~'),
                NamedKey::F9 => (20, '~'),
                NamedKey::F10 => (21, '~'),
                NamedKey::F11 => (23, '~'),
                NamedKey::F12 => (24, '~'),
                NamedKey::F13 => (57376, 'u'),
                NamedKey::F14 => (57377, 'u'),
                NamedKey::F15 => (57378, 'u'),
                NamedKey::F16 => (57379, 'u'),
                NamedKey::F17 => (57380, 'u'),
                NamedKey::F18 => (57381, 'u'),
                NamedKey::F19 => (57382, 'u'),
                NamedKey::F20 => (57383, 'u'),
                NamedKey::F21 => (57384, 'u'),
                NamedKey::F22 => (57385, 'u'),
                NamedKey::F23 => (57386, 'u'),
                NamedKey::F24 => (57387, 'u'),
                NamedKey::F25 => (57388, 'u'),
                NamedKey::F26 => (57389, 'u'),
                NamedKey::F27 => (57390, 'u'),
                NamedKey::F28 => (57391, 'u'),
                NamedKey::F29 => (57392, 'u'),
                NamedKey::F30 => (57393, 'u'),
                NamedKey::F31 => (57394, 'u'),
                NamedKey::F32 => (57395, 'u'),
                NamedKey::F33 => (57396, 'u'),
                NamedKey::F34 => (57397, 'u'),
                NamedKey::F35 => (57398, 'u'),
                NamedKey::MediaPlay => (57428, 'u'),
                NamedKey::MediaPause => (57429, 'u'),
                NamedKey::MediaPlayPause => (57430, 'u'),
                NamedKey::MediaStop => (57432, 'u'),
                NamedKey::MediaFastForward => (57433, 'u'),
                NamedKey::MediaRewind => (57434, 'u'),
                NamedKey::MediaTrackNext => (57435, 'u'),
                NamedKey::MediaTrackPrevious => (57436, 'u'),
                NamedKey::MediaRecord => (57437, 'u'),
                NamedKey::AudioVolumeDown => (57438, 'u'),
                NamedKey::AudioVolumeUp => (57439, 'u'),
                NamedKey::AudioVolumeMute => (57440, 'u'),
                NamedKey::CapsLock => (57358, 'u'),
                NamedKey::ScrollLock => (57359, 'u'),
                NamedKey::NumLock => (57360, 'u'),
                NamedKey::PrintScreen => (57361, 'u'),
                NamedKey::Pause => (57362, 'u'),
                NamedKey::ContextMenu => (57363, 'u'),
                NamedKey::Shift | NamedKey::Control | NamedKey::Alt | NamedKey::Super => {
                    match physical {
                        PhysicalKey::Code(KeyCode::ShiftLeft) => (57441, 'u'),
                        PhysicalKey::Code(KeyCode::ControlLeft) => (57442, 'u'),
                        PhysicalKey::Code(KeyCode::AltLeft) => (57443, 'u'),
                        PhysicalKey::Code(KeyCode::SuperLeft) => (57444, 'u'),
                        PhysicalKey::Code(KeyCode::ShiftRight) => (57447, 'u'),
                        PhysicalKey::Code(KeyCode::ControlRight) => (57448, 'u'),
                        PhysicalKey::Code(KeyCode::AltRight) => (57449, 'u'),
                        PhysicalKey::Code(KeyCode::SuperRight) => (57450, 'u'),
                        _ => return None,
                    }
                }
                _ => return None,
            },
            _ => return None, // Composition/dead keys belong to the IME.
        };
        let keypad = match physical {
            PhysicalKey::Code(KeyCode::Numpad0) => Some(57399),
            PhysicalKey::Code(KeyCode::Numpad1) => Some(57400),
            PhysicalKey::Code(KeyCode::Numpad2) => Some(57401),
            PhysicalKey::Code(KeyCode::Numpad3) => Some(57402),
            PhysicalKey::Code(KeyCode::Numpad4) => Some(57403),
            PhysicalKey::Code(KeyCode::Numpad5) => Some(57404),
            PhysicalKey::Code(KeyCode::Numpad6) => Some(57405),
            PhysicalKey::Code(KeyCode::Numpad7) => Some(57406),
            PhysicalKey::Code(KeyCode::Numpad8) => Some(57407),
            PhysicalKey::Code(KeyCode::Numpad9) => Some(57408),
            PhysicalKey::Code(KeyCode::NumpadDecimal) => Some(57409),
            PhysicalKey::Code(KeyCode::NumpadDivide) => Some(57410),
            PhysicalKey::Code(KeyCode::NumpadMultiply) => Some(57411),
            PhysicalKey::Code(KeyCode::NumpadSubtract) => Some(57412),
            PhysicalKey::Code(KeyCode::NumpadAdd) => Some(57413),
            PhysicalKey::Code(KeyCode::NumpadEnter) => Some(57414),
            PhysicalKey::Code(KeyCode::NumpadEqual) => Some(57415),
            PhysicalKey::Code(KeyCode::NumpadComma) => Some(57416),
            _ => None,
        };
        let keypad = if keypad.is_some() {
            match base {
                Key::Named(NamedKey::ArrowLeft) => Some(57417),
                Key::Named(NamedKey::ArrowRight) => Some(57418),
                Key::Named(NamedKey::ArrowUp) => Some(57419),
                Key::Named(NamedKey::ArrowDown) => Some(57420),
                Key::Named(NamedKey::PageUp) => Some(57421),
                Key::Named(NamedKey::PageDown) => Some(57422),
                Key::Named(NamedKey::Home) => Some(57423),
                Key::Named(NamedKey::End) => Some(57424),
                Key::Named(NamedKey::Insert) => Some(57425),
                Key::Named(NamedKey::Delete) => Some(57426),
                _ => keypad,
            }
        } else {
            None
        };
        let base_layout = match physical {
            PhysicalKey::Code(KeyCode::KeyA) => Some(97),
            PhysicalKey::Code(KeyCode::KeyB) => Some(98),
            PhysicalKey::Code(KeyCode::KeyC) => Some(99),
            PhysicalKey::Code(KeyCode::KeyD) => Some(100),
            PhysicalKey::Code(KeyCode::KeyE) => Some(101),
            PhysicalKey::Code(KeyCode::KeyF) => Some(102),
            PhysicalKey::Code(KeyCode::KeyG) => Some(103),
            PhysicalKey::Code(KeyCode::KeyH) => Some(104),
            PhysicalKey::Code(KeyCode::KeyI) => Some(105),
            PhysicalKey::Code(KeyCode::KeyJ) => Some(106),
            PhysicalKey::Code(KeyCode::KeyK) => Some(107),
            PhysicalKey::Code(KeyCode::KeyL) => Some(108),
            PhysicalKey::Code(KeyCode::KeyM) => Some(109),
            PhysicalKey::Code(KeyCode::KeyN) => Some(110),
            PhysicalKey::Code(KeyCode::KeyO) => Some(111),
            PhysicalKey::Code(KeyCode::KeyP) => Some(112),
            PhysicalKey::Code(KeyCode::KeyQ) => Some(113),
            PhysicalKey::Code(KeyCode::KeyR) => Some(114),
            PhysicalKey::Code(KeyCode::KeyS) => Some(115),
            PhysicalKey::Code(KeyCode::KeyT) => Some(116),
            PhysicalKey::Code(KeyCode::KeyU) => Some(117),
            PhysicalKey::Code(KeyCode::KeyV) => Some(118),
            PhysicalKey::Code(KeyCode::KeyW) => Some(119),
            PhysicalKey::Code(KeyCode::KeyX) => Some(120),
            PhysicalKey::Code(KeyCode::KeyY) => Some(121),
            PhysicalKey::Code(KeyCode::KeyZ) => Some(122),
            _ => None,
        };
        Some(Self {
            code,
            suffix,
            shifted: shifted.filter(|&s| s != code),
            keypad,
            base_layout: base_layout.filter(|&b| b != code),
        })
    }
    pub fn encode(
        self,
        flags: u16,
        modifiers: u8,
        event: u8,
        text: Option<&str>,
    ) -> Option<Vec<u8>> {
        if flags == 0 {
            return None;
        }
        let all = flags & 8 != 0;
        if self.suffix == 'u' && self.code < 57344 && flags & 9 == 0 {
            return None;
        }
        let has_text = text.is_some_and(|s| !s.is_empty() && !s.chars().any(char::is_control));
        let keypad = self.keypad.filter(|_| all || flags & 1 != 0 && !has_text);
        let raw_code = keypad.unwrap_or(self.code);
        let suffix = if keypad.is_some() { 'u' } else { self.suffix };
        let simple = suffix == 'u' && matches!(raw_code, 9 | 13 | 127);
        let modifier_key = raw_code >= 57441;
        if !all && (modifier_key || simple && modifiers == 0 || has_text && modifiers & !1 == 0) {
            return None;
        }
        if event == 3 && (flags & 2 == 0 || !all && simple) {
            return None;
        }
        let mut code = raw_code.to_string();
        if suffix == 'u' && flags & 4 != 0 && keypad.is_none() {
            if let Some(shifted) = self.shifted {
                code.push_str(&format!(":{shifted}"));
            }
            if let Some(base) = self.base_layout {
                if self.shifted.is_none() {
                    code.push(':');
                }
                code.push_str(&format!(":{base}"));
            }
        }
        let mut result = format!("\x1b[{code};{}", u16::from(modifiers) + 1);
        if flags & 2 != 0 {
            result.push_str(&format!(":{event}"));
        }
        if flags & 24 == 24 && event != 3 {
            if let Some(text) = text.filter(|s| !s.is_empty() && !s.chars().any(char::is_control)) {
                result.push(';');
                result.push_str(
                    &text
                        .chars()
                        .map(|c| u32::from(c).to_string())
                        .collect::<Vec<_>>()
                        .join(":"),
                );
            }
        }
        result.push(suffix);
        Some(result.into_bytes())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn key(code: u32) -> ProtocolKey {
        ProtocolKey {
            code,
            suffix: 'u',
            shifted: None,
            keypad: None,
            base_layout: None,
        }
    }
    #[test]
    fn legacy_and_plain_typing_remain_plain() {
        assert_eq!(key(97).encode(0, 4, 1, Some("a")), None);
        assert_eq!(key(97).encode(3, 0, 1, Some("a")), None);
        assert_eq!(key(13).encode(3, 0, 1, Some("\r")), None);
        assert_eq!(key(13).encode(3, 0, 3, None), None);
    }
    #[test]
    fn disambiguation_repeats_releases_and_text() {
        assert_eq!(key(97).encode(1, 4, 1, None), Some(b"\x1b[97;5u".to_vec()));
        assert_eq!(
            key(97).encode(3, 4, 2, None),
            Some(b"\x1b[97;5:2u".to_vec())
        );
        assert_eq!(
            key(97).encode(3, 4, 3, None),
            Some(b"\x1b[97;5:3u".to_vec())
        );
        assert_eq!(
            key(97).encode(27, 0, 1, Some("a")),
            Some(b"\x1b[97;1:1;97u".to_vec())
        );
        assert_eq!(
            key(97).encode(27, 0, 3, Some("a")),
            Some(b"\x1b[97;1:3u".to_vec())
        );
    }
    #[test]
    fn composition_is_pure_text_and_control_codes_are_not_associated_text() {
        assert_eq!(ProtocolKey::committed_text(3, "é"), None);
        assert_eq!(
            ProtocolKey::committed_text(24, "é👩"),
            Some(b"\x1b[0;1;233:128105u".to_vec())
        );
        assert_eq!(
            ProtocolKey::committed_text(26, "é"),
            Some(b"\x1b[0;1:1;233u".to_vec())
        );
        assert_eq!(
            key(13).encode(24, 0, 1, Some("\r")),
            Some(b"\x1b[13;1u".to_vec())
        );
        assert_eq!(
            key(97).encode(24, 4, 1, Some("\x01")),
            Some(b"\x1b[97;5u".to_vec())
        );
    }
}

#[cfg(test)]
mod extended_tests {
    use super::*;
    #[test]
    fn alternate_layout_has_empty_shift_slot_and_us_base() {
        let key = ProtocolKey::from_event(
            &Key::Character("ф".into()),
            &Key::Character("ф".into()),
            PhysicalKey::Code(KeyCode::KeyA),
        )
        .unwrap();
        assert_eq!(
            key.encode(5, 4, 1, None),
            Some(b"\x1b[1092::97;5u".to_vec())
        );
    }
    #[test]
    fn keypad_navigation_is_distinct_and_legacy_stays_legacy() {
        let key = ProtocolKey::from_event(
            &Key::Named(NamedKey::ArrowLeft),
            &Key::Named(NamedKey::ArrowLeft),
            PhysicalKey::Code(KeyCode::Numpad4),
        )
        .unwrap();
        assert_eq!(key.encode(0, 0, 1, None), None);
        assert_eq!(key.encode(1, 0, 1, None), Some(b"\x1b[57417;1u".to_vec()));
        let key = ProtocolKey::from_event(
            &Key::Character("4".into()),
            &Key::Character("4".into()),
            PhysicalKey::Code(KeyCode::Numpad4),
        )
        .unwrap();
        assert_eq!(key.encode(1, 0, 1, Some("4")), None);
        assert_eq!(
            key.encode(8, 0, 1, Some("4")),
            Some(b"\x1b[57403;1u".to_vec())
        );
    }
    #[test]
    fn dead_keys_do_not_become_raw_escape_sequences() {
        assert!(ProtocolKey::from_event(
            &Key::Dead(Some('´')),
            &Key::Dead(Some('´')),
            PhysicalKey::Code(KeyCode::Quote)
        )
        .is_none());
    }
}
