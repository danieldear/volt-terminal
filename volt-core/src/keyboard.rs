//! Negotiated per-screen keyboard state. Legacy input remains the default.
const SUPPORTED: u16 = 31;
const MAX_DEPTH: usize = 32;
#[derive(Default)]
pub struct KeyboardModes {
    flags: u16,
    stack: Vec<u16>,
}
impl KeyboardModes {
    pub fn flags(&self) -> u16 {
        self.flags
    }
    pub fn set(&mut self, flags: u16, mode: u16) {
        let flags = flags & SUPPORTED;
        match mode {
            0 | 1 => self.flags = flags,
            2 => self.flags |= flags,
            3 => self.flags &= !flags,
            _ => {}
        }
    }
    pub fn push(&mut self, flags: u16) {
        if self.stack.len() == MAX_DEPTH {
            self.stack.remove(0);
        }
        self.stack.push(self.flags);
        self.flags = flags & SUPPORTED;
    }
    pub fn pop(&mut self, count: u16) {
        for _ in 0..usize::from(count.max(1)).min(MAX_DEPTH + 1) {
            self.flags = self.stack.pop().unwrap_or(0);
        }
    }
}
// ── Legacy (xterm) key encoding, shared by Volt Terminal and Volt Editor ─────

/// A key by its position, named as the W3C UI Events `code` ("KeyA",
/// "Digit2", "BracketLeft", "ArrowUp", "F5"), as windowing libraries report
/// it. Terminals encode by position, so Control-C works on any layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Enter,
    NumpadEnter,
    Backspace,
    NumpadBackspace,
    Tab,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowRight,
    ArrowLeft,
    Home,
    End,
    Insert,
    Delete,
    PageUp,
    PageDown,
    /// F1 to F24.
    F(u8),
    /// `b'A'` to `b'Z'`.
    Letter(u8),
    /// 0 to 9.
    Digit(u8),
    Space,
    BracketLeft,
    BracketRight,
    Backslash,
    Slash,
    Minus,
    Other,
}

impl Key {
    pub fn from_code(code: &str) -> Self {
        match code {
            "Enter" => Self::Enter,
            "NumpadEnter" => Self::NumpadEnter,
            "Backspace" => Self::Backspace,
            "NumpadBackspace" => Self::NumpadBackspace,
            "Tab" => Self::Tab,
            "Escape" => Self::Escape,
            "ArrowUp" => Self::ArrowUp,
            "ArrowDown" => Self::ArrowDown,
            "ArrowRight" => Self::ArrowRight,
            "ArrowLeft" => Self::ArrowLeft,
            "Home" => Self::Home,
            "End" => Self::End,
            "Insert" => Self::Insert,
            "Delete" => Self::Delete,
            "PageUp" => Self::PageUp,
            "PageDown" => Self::PageDown,
            "Space" => Self::Space,
            "BracketLeft" => Self::BracketLeft,
            "BracketRight" => Self::BracketRight,
            "Backslash" => Self::Backslash,
            "Slash" => Self::Slash,
            "Minus" => Self::Minus,
            c => {
                let letter = c.strip_prefix("Key").filter(|l| l.len() == 1).map(|l| l.as_bytes()[0]);
                let digit = c.strip_prefix("Digit").and_then(|d| d.parse::<u8>().ok()).filter(|d| *d <= 9);
                let function = c.strip_prefix('F').and_then(|f| f.parse::<u8>().ok()).filter(|f| (1..=24).contains(f));
                match (letter, digit, function) {
                    (Some(l), _, _) if l.is_ascii_uppercase() => Self::Letter(l),
                    (_, Some(d), _) => Self::Digit(d),
                    (_, _, Some(f)) => Self::F(f),
                    _ => Self::Other,
                }
            }
        }
    }
}

fn xterm_modifier_param(shift: bool, alt: bool, ctrl: bool) -> Option<u8> {
    let mut value = 1u8;
    if shift {
        value = value.saturating_add(1);
    }
    if alt {
        value = value.saturating_add(2);
    }
    if ctrl {
        value = value.saturating_add(4);
    }
    (value > 1).then_some(value)
}

fn ss3_function_sequence(letter: char, modifier: Option<u8>) -> Vec<u8> {
    match modifier {
        Some(m) => format!("\x1b[1;{}{}", m, letter).into_bytes(),
        None => format!("\x1bO{}", letter).into_bytes(),
    }
}

fn csi_tilde_sequence(code: u16, modifier: Option<u8>) -> Vec<u8> {
    match modifier {
        Some(m) => format!("\x1b[{};{}~", code, m).into_bytes(),
        None => format!("\x1b[{}~", code).into_bytes(),
    }
}

/// What a terminal sends for a key that isn't text (Enter, arrows, F5…), or
/// `None` for keys that type text.
pub fn special_sequence(
    key: Key,
    shift: bool,
    alt: bool,
    ctrl: bool,
    application_cursor_keys_mode: bool,
) -> Option<Vec<u8>> {
    let modifier = xterm_modifier_param(shift, alt, ctrl);
    let cursor = |letter: char| {
        if let Some(m) = modifier {
            format!("\x1b[1;{}{}", m, letter).into_bytes()
        } else if application_cursor_keys_mode {
            format!("\x1bO{}", letter).into_bytes()
        } else {
            format!("\x1b[{}", letter).into_bytes()
        }
    };
    let home_end = |letter: char| match modifier {
        Some(m) => format!("\x1b[1;{}{}", m, letter).into_bytes(),
        None => format!("\x1b[{}", letter).into_bytes(),
    };
    match key {
        Key::Enter | Key::NumpadEnter => {
            let mut bytes = Vec::with_capacity(2);
            if alt {
                bytes.push(0x1b);
            }
            bytes.push(b'\r');
            Some(bytes)
        }
        Key::Backspace | Key::NumpadBackspace => {
            let mut bytes = Vec::with_capacity(2);
            if alt {
                bytes.push(0x1b);
            }
            bytes.push(0x7f);
            Some(bytes)
        }
        Key::Tab => match modifier {
            Some(2) => Some(b"\x1b[Z".to_vec()),
            Some(m) => Some(format!("\x1b[1;{}Z", m).into_bytes()),
            None => Some(b"\t".to_vec()),
        },
        Key::Escape => {
            let mut bytes = Vec::with_capacity(2);
            bytes.push(0x1b);
            if alt {
                bytes.push(0x1b);
            }
            Some(bytes)
        }
        Key::ArrowUp => Some(cursor('A')),
        Key::ArrowDown => Some(cursor('B')),
        Key::ArrowRight => Some(cursor('C')),
        Key::ArrowLeft => Some(cursor('D')),
        Key::Home => Some(home_end('H')),
        Key::End => Some(home_end('F')),
        Key::Insert => Some(csi_tilde_sequence(2, modifier)),
        Key::Delete => Some(csi_tilde_sequence(3, modifier)),
        Key::PageUp => Some(csi_tilde_sequence(5, modifier)),
        Key::PageDown => Some(csi_tilde_sequence(6, modifier)),
        Key::F(n @ 1..=4) => Some(ss3_function_sequence(['P', 'Q', 'R', 'S'][n as usize - 1], modifier)),
        Key::F(n @ 5..=24) => {
            const CODES: [u16; 20] = [15, 17, 18, 19, 20, 21, 23, 24, 25, 26, 28, 29, 31, 32, 33, 34, 42, 43, 44, 45];
            Some(csi_tilde_sequence(CODES[n as usize - 5], modifier))
        }
        _ => None,
    }
}

/// The control byte Control makes with a letter or symbol key.
pub fn ctrl_code(key: Key) -> Option<u8> {
    match key {
        Key::Letter(l @ b'A'..=b'Z') => Some(l - b'@'),
        Key::Space | Key::Digit(2) => Some(0x00),
        Key::Digit(3) | Key::BracketLeft => Some(0x1b),
        Key::Digit(4) | Key::Backslash => Some(0x1c),
        Key::Digit(5) | Key::BracketRight => Some(0x1d),
        Key::Digit(6) => Some(0x1e),
        Key::Digit(7) | Key::Slash | Key::Minus => Some(0x1f),
        Key::Digit(8) => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys_encode_by_position_as_xterm_does() {
        let k = Key::from_code;
        assert_eq!(k("KeyC"), Key::Letter(b'C'));
        assert_eq!(k("Digit2"), Key::Digit(2));
        assert_eq!(k("F12"), Key::F(12));
        assert_eq!(k("Keyboard"), Key::Other);
        assert_eq!(ctrl_code(k("KeyC")), Some(3));
        assert_eq!(ctrl_code(k("BracketLeft")), Some(0x1b));
        assert_eq!(special_sequence(k("Enter"), false, true, false, false), Some(b"\x1b\r".to_vec()));
        assert_eq!(special_sequence(k("ArrowUp"), false, false, false, false), Some(b"\x1b[A".to_vec()));
        assert_eq!(special_sequence(k("ArrowUp"), false, false, false, true), Some(b"\x1bOA".to_vec()));
        assert_eq!(special_sequence(k("ArrowUp"), true, false, false, true), Some(b"\x1b[1;2A".to_vec()));
        assert_eq!(special_sequence(k("Tab"), true, false, false, false), Some(b"\x1b[Z".to_vec()));
        assert_eq!(special_sequence(k("F5"), false, false, false, false), Some(b"\x1b[15~".to_vec()));
        assert_eq!(special_sequence(k("F24"), false, false, true, false), Some(b"\x1b[45;5~".to_vec()));
        assert_eq!(special_sequence(k("KeyA"), false, false, false, false), None);
    }
    #[test]
    fn bounded_stack_set_modes_and_underflow() {
        let mut k = KeyboardModes::default();
        k.set(1, 1);
        k.set(2, 2);
        assert_eq!(k.flags(), 3);
        k.set(1, 3);
        assert_eq!(k.flags(), 2);
        k.set(31, 9);
        assert_eq!(k.flags(), 2);
        for _ in 0..100 {
            k.push(65535);
        }
        assert_eq!(k.stack.len(), MAX_DEPTH);
        assert_eq!(k.flags(), 31);
        k.pop(65535);
        assert_eq!(k.flags(), 0);
    }
}
