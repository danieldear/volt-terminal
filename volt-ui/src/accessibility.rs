//! Pure, immutable output snapshots. No terminal, PTY, or native dependencies.
//!
//! All offsets are UTF-16 code units in the published (possibly truncated)
//! string, not grid columns, UTF-8 bytes, or retained scrollback coordinates.

pub const MAX_UTF16: usize = 32 * 1024;
pub const MAX_ROWS: usize = 256;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sensitivity {
    /// Parent has positively established that output may be exposed.
    KnownSafe,
    #[default]
    Unknown,
    NoEcho,
    ManualSecure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange {
    pub location: usize,
    pub length: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccessibilityScope {
    #[default]
    Redacted,
    TerminalOutput,
    ModalSummary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AccessibilitySnapshot {
    text: String,
    label: String,
    // UTF-16 offset -> UTF-8 boundary; surrogate interiors are usize::MAX.
    boundaries: Vec<usize>,
    lines: Vec<TextRange>,
    selection: Option<TextRange>,
    scope: AccessibilityScope,
}

impl AccessibilitySnapshot {
    /// Only active displayed output: callers must not supply hidden panes,
    /// background terminal content, or offscreen scrollback. Unknown is denied.
    pub fn terminal_output(
        displayed_text: &str,
        sensitivity: Sensitivity,
        selection: Option<TextRange>,
    ) -> Self {
        if sensitivity != Sensitivity::KnownSafe {
            return Self::default();
        }
        Self::build(
            displayed_text,
            "Terminal output",
            AccessibilityScope::TerminalOutput,
            selection,
        )
    }

    /// Explicit safe-text boundary: the parent supplies a public modal name and
    /// read-only summary, never form values or the obscured terminal. Background
    /// terminal sensitivity does not prevent an independently safe modal summary.
    pub fn modal_summary(safe_name: &str, safe_summary: &str) -> Self {
        if safe_name.trim().is_empty() {
            return Self::default();
        }
        Self::build(
            safe_summary,
            safe_name,
            AccessibilityScope::ModalSummary,
            None,
        )
    }

    fn build(
        source: &str,
        label: &str,
        scope: AccessibilityScope,
        selection: Option<TextRange>,
    ) -> Self {
        let mut result = Self {
            label: bounded_text(label, 256, 1),
            scope,
            text: bounded_text(source, MAX_UTF16, MAX_ROWS),
            ..Self::default()
        };
        result.boundaries.push(0);
        let mut line_start = 0;
        for (byte, ch) in result.text.char_indices() {
            if ch.len_utf16() == 2 {
                result.boundaries.push(usize::MAX);
            }
            result.boundaries.push(byte + ch.len_utf8());
            if ch == '\n' {
                let end = result.boundaries.len() - 1;
                result.lines.push(TextRange {
                    location: line_start,
                    length: end - line_start,
                });
                line_start = end;
            }
        }
        if !result.text.is_empty() {
            result.lines.push(TextRange {
                location: line_start,
                length: result.number_of_characters() - line_start,
            });
        }
        // Reject rather than clip stale/outside/half-surrogate selections.
        result.selection = selection.filter(|range| result.string_for_range(*range).is_some());
        result
    }

    pub fn scope(&self) -> AccessibilityScope {
        self.scope
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub fn number_of_characters(&self) -> usize {
        self.boundaries.len().saturating_sub(1)
    }
    pub fn visible_range(&self) -> TextRange {
        TextRange {
            location: 0,
            length: self.number_of_characters(),
        }
    }
    pub fn selected_range(&self) -> Option<TextRange> {
        self.selection
    }
    pub fn selected_text(&self) -> Option<&str> {
        self.selection
            .and_then(|range| self.string_for_range(range))
    }
    pub fn string_for_range(&self, range: TextRange) -> Option<&str> {
        let end = range.location.checked_add(range.length)?;
        // The redacted snapshot exposes only the valid empty range.
        if self.boundaries.is_empty() {
            return (range.location == 0 && range.length == 0).then_some("");
        }
        let start_byte = *self.boundaries.get(range.location)?;
        let end_byte = *self.boundaries.get(end)?;
        self.text.get(start_byte..end_byte)
    }
    /// Newline belongs to the preceding line. EOF belongs to the final line;
    /// an index inside a surrogate pair still identifies its containing line.
    pub fn line_for_index(&self, index: usize) -> Option<usize> {
        if index > self.number_of_characters() || self.lines.is_empty() {
            return None;
        }
        Some(
            self.lines
                .iter()
                .rposition(|line| line.location <= index)
                .unwrap_or(0),
        )
    }
    pub fn range_for_line(&self, line: usize) -> Option<TextRange> {
        self.lines.get(line).copied()
    }
}

fn bounded_text(source: &str, max_units: usize, max_rows: usize) -> String {
    let mut units = 0;
    let mut rows = 1;
    let mut end = 0;
    for (byte, ch) in source.char_indices() {
        if units + ch.len_utf16() > max_units || (ch == '\n' && rows == max_rows) {
            break;
        }
        units += ch.len_utf16();
        if ch == '\n' {
            rows += 1;
        }
        end = byte + ch.len_utf8();
    }
    source[..end].to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn safe(text: &str) -> AccessibilitySnapshot {
        AccessibilitySnapshot::terminal_output(text, Sensitivity::KnownSafe, None)
    }
    #[test]
    fn unicode_ranges_and_selection_use_utf16() {
        let s = AccessibilitySnapshot::terminal_output(
            "a😀e\u{301}\n👩‍💻\0",
            Sensitivity::KnownSafe,
            Some(TextRange {
                location: 1,
                length: 2,
            }),
        );
        assert_eq!(s.number_of_characters(), 12);
        assert_eq!(s.selected_text(), Some("😀"));
        assert_eq!(
            s.string_for_range(TextRange {
                location: 2,
                length: 1
            }),
            None
        );
        assert_eq!(
            s.string_for_range(TextRange {
                location: 1,
                length: 1
            }),
            None
        );
        assert_eq!(s.string_for_range(s.visible_range()), Some(s.text()));
        assert_eq!(
            s.string_for_range(TextRange {
                location: 6,
                length: 6
            }),
            Some("👩‍💻\0")
        );
    }
    #[test]
    fn privacy_is_fail_closed() {
        for sensitivity in [
            Sensitivity::Unknown,
            Sensitivity::NoEcho,
            Sensitivity::ManualSecure,
        ] {
            let s = AccessibilitySnapshot::terminal_output(
                "secret",
                sensitivity,
                Some(TextRange {
                    location: 0,
                    length: 6,
                }),
            );
            assert_eq!(s, AccessibilitySnapshot::default());
            assert_eq!(s.selected_text(), None);
            assert_eq!(s.line_for_index(0), None);
        }
    }
    #[test]
    fn modal_replaces_terminal_and_is_named() {
        let s = AccessibilitySnapshot::modal_summary("Settings", "Read-only summary");
        assert_eq!(s.scope(), AccessibilityScope::ModalSummary);
        assert_eq!(s.label(), "Settings");
        assert_eq!(s.text(), "Read-only summary");
        assert_eq!(s.selected_range(), None);
        assert_eq!(
            AccessibilitySnapshot::modal_summary(" ", "secret"),
            AccessibilitySnapshot::default()
        );
    }
    #[test]
    fn bounds_and_overflow_are_rejected() {
        let s = safe("😀x");
        for range in [
            TextRange {
                location: usize::MAX,
                length: 1,
            },
            TextRange {
                location: 0,
                length: 4,
            },
            TextRange {
                location: 4,
                length: 0,
            },
        ] {
            assert_eq!(s.string_for_range(range), None);
        }
        assert_eq!(
            s.string_for_range(TextRange {
                location: 3,
                length: 0
            }),
            Some("")
        );
        assert_eq!(s.line_for_index(4), None);
        let invalid = AccessibilitySnapshot::terminal_output(
            "😀",
            Sensitivity::KnownSafe,
            Some(TextRange {
                location: 1,
                length: 0,
            }),
        );
        assert_eq!(invalid.selected_text(), None);
    }
    #[test]
    fn rows_newlines_and_eof_are_consistent() {
        let s = safe("😀\n\nx\n");
        assert_eq!(s.line_for_index(1), Some(0));
        assert_eq!(s.line_for_index(2), Some(0));
        assert_eq!(s.line_for_index(3), Some(1));
        assert_eq!(s.line_for_index(6), Some(3));
        assert_eq!(
            s.string_for_range(s.range_for_line(0).unwrap()),
            Some("😀\n")
        );
        assert_eq!(
            s.range_for_line(3),
            Some(TextRange {
                location: 6,
                length: 0
            })
        );
        assert_eq!(s.range_for_line(4), None);
        assert_eq!(safe("").line_for_index(0), None);
    }
    #[test]
    fn truncation_is_bounded_and_never_splits_surrogates() {
        let s = safe(&("a".repeat(MAX_UTF16 - 1) + "😀secret"));
        assert_eq!(s.number_of_characters(), MAX_UTF16 - 1);
        assert!(!s.text().contains("secret"));
        let s = safe(&"x\n".repeat(1000));
        assert_eq!(s.lines.len(), MAX_ROWS);
        assert_eq!(s.text().matches('\n').count(), MAX_ROWS - 1);
        let s = safe(&"😀".repeat(MAX_UTF16));
        assert_eq!(s.number_of_characters(), MAX_UTF16);
        assert_eq!(s.string_for_range(s.visible_range()), Some(s.text()));
    }
}
