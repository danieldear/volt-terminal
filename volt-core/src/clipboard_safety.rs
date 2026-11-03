//! Pure clipboard approval rules; no clipboard access or payload logging.
use std::time::{Duration, Instant};
pub const MAX_PASTE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_PREVIEW_CHARS: usize = 4096;
pub const APPROVAL_TTL: Duration = Duration::from_secs(30);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferKind {
    Paste,
    TerminalCopy,
}
pub struct PendingTransfer {
    pub data: Vec<u8>,
    pub pane_pid: Option<u32>,
    pub kind: TransferKind,
    pub created: Instant,
}
impl PendingTransfer {
    pub fn applies_to(&self, pane_pid: Option<u32>, now: Instant) -> bool {
        self.pane_pid.is_some()
            && self.pane_pid == pane_pid
            && now.saturating_duration_since(self.created) <= APPROVAL_TTL
    }
}
pub fn requires_review(bytes: &[u8], bracketed: bool) -> bool {
    std::str::from_utf8(bytes).map_or(true, |text| {
        text.chars()
            .any(|ch| matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
    }) || bytes.iter().any(|&b| match b {
        b'\n' | b'\r' => !bracketed,
        b'\t' => false,
        0..=31 | 127 => true,
        _ => false,
    })
}
pub fn preview(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, ch) in String::from_utf8_lossy(bytes).chars().enumerate() {
        if i == MAX_PREVIEW_CHARS {
            out.push_str("\n… Preview truncated; approval applies to all bytes.");
            break;
        }
        if ch == '\n' || ch == '\t' {
            out.push(ch);
        } else if ch.is_control() || matches!(ch,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}') {
            out.extend(ch.escape_unicode());
        } else {
            out.push(ch);
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bracketed_newlines_are_safe_but_escape_control_data_is_not() {
        assert!(!requires_review(b"hello", false));
        assert!(requires_review(b"echo one\necho two", false));
        assert!(!requires_review(b"echo one\necho two", true));
        assert!(requires_review(b"\x1b[201~\r", true));
        assert!(requires_review(b"\0", true));
        assert!(requires_review("abc\u{202e}def".as_bytes(), true));
        assert!(requires_review(&[0xff], true));
        assert!(preview(b"\x1b[201~").contains("\\u{1b}"));
    }
    #[test]
    fn approval_is_single_target_and_time_bounded() {
        let now = Instant::now();
        let p = PendingTransfer {
            data: vec![],
            pane_pid: Some(42),
            kind: TransferKind::Paste,
            created: now,
        };
        assert!(p.applies_to(Some(42), now));
        assert!(!p.applies_to(Some(43), now));
        assert!(!p.applies_to(Some(42), now + APPROVAL_TTL + Duration::from_secs(1)));
    }
}
