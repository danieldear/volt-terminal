//! Shared modal text-prompt overlay: a single-line text input rendered
//! centered over the terminal. Used for Find (Cmd+F) and the
//! Change Tab Title.../Change Terminal Title... context-menu actions.
//!
//! The buffer is a `Vec<char>` rather than raw byte offsets into a `String`
//! specifically to avoid UTF-8 char-boundary bugs when inserting/deleting at
//! an arbitrary cursor position with multi-byte input.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
/// A single match location for an active search: a half-open column range on
/// one row of the pane's combined scrollback+live view (row 0 = oldest
/// scrollback line, matching the indexing `Grid::scrollback_cell` uses,
/// followed by the live grid rows).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchMatch {
    /// Row index into scrollback (if `< scrollback_len`) or, after
    /// subtracting `scrollback_len`, into the live grid.
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Find,
    RenameTab,
    RenameTerminal,
}

pub struct TextPrompt {
    pub kind: PromptKind,
    chars: Vec<char>,
    pub cursor: usize,
    /// Populated live as the user types, when `kind == Find`.
    pub matches: Vec<SearchMatch>,
    /// True when search stopped after the bounded result limit.
    pub matches_truncated: bool,
    /// Index into `matches` of the currently-highlighted match.
    pub current_match: usize,
    // Preserve a location while the old result list is hidden during refresh.
    refresh_anchor: Option<SearchMatch>,
}

impl TextPrompt {
    pub fn new(kind: PromptKind, initial: &str) -> Self {
        let chars: Vec<char> = initial.chars().collect();
        let cursor = chars.len();
        Self {
            kind,
            chars,
            cursor,
            matches: Vec::new(),
            matches_truncated: false,
            current_match: 0,
            refresh_anchor: None,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.kind {
            PromptKind::Find => "Find",
            PromptKind::RenameTab => "Change Tab Title",
            PromptKind::RenameTerminal => "Change Terminal Title",
        }
    }

    pub fn text(&self) -> String {
        self.chars.iter().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    pub fn insert_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.chars.remove(self.cursor - 1);
        self.cursor -= 1;
    }

    pub fn delete_forward(&mut self) {
        if self.cursor < self.chars.len() {
            self.chars.remove(self.cursor);
        }
    }

    pub fn move_left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.chars.len());
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.chars.len();
    }

    /// Drop results and selection when the query or target changes.
    pub fn clear_matches(&mut self) {
        self.matches.clear();
        self.matches_truncated = false;
        self.current_match = 0;
        self.refresh_anchor = None;
    }

    /// Hide stale highlights without forgetting the selected location. Multiple
    /// output notifications can arrive before the throttled scan runs.
    pub fn invalidate_matches(&mut self) {
        let anchor = self
            .matches
            .get(self.current_match)
            .copied()
            .or(self.refresh_anchor);
        self.clear_matches();
        self.refresh_anchor = anchor;
    }

    /// Keep the selected location if it is still a match in the new snapshot.
    /// This is coordinate-based, not a persistent line ID: if history is evicted
    /// or the location disappears, selection falls back to the first result.
    pub fn replace_matches(&mut self, matches: Vec<SearchMatch>, truncated: bool) {
        let anchor = self
            .matches
            .get(self.current_match)
            .copied()
            .or(self.refresh_anchor.take());
        self.current_match = anchor
            .and_then(|selected| matches.iter().position(|m| *m == selected))
            .unwrap_or(0);
        self.matches = matches;
        self.matches_truncated = truncated;
    }

    /// Advance to the next match, wrapping around. No-op if there are none.
    pub fn next_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = (self.current_match + 1) % self.matches.len();
    }

    /// Move to the previous match, wrapping around. No-op if there are none.
    pub fn prev_match(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        self.current_match = self
            .current_match
            .checked_sub(1)
            .unwrap_or(self.matches.len() - 1);
    }
}

/// Find every case-insensitive occurrence of `query` across a pane's full
/// text, both scrollback (oldest first) and the live grid. `line` is called
/// once per row — `row_provider` yields `(row_index, row_text)` pairs in the
/// same order `SearchMatch::row` addresses (scrollback rows first, then live
/// grid rows, matching `TextPrompt::matches`' row convention).
pub fn find_matches<'a>(
    query: &str,
    rows: impl Iterator<Item = (usize, &'a str)>,
) -> Vec<SearchMatch> {
    find_matches_bounded(query, rows, usize::MAX).0
}

/// Return at most `limit` matches. The boolean says that more matches exist;
/// callers must not present a capped count as the total count.
pub fn find_matches_bounded<'a>(
    query: &str,
    rows: impl Iterator<Item = (usize, &'a str)>,
    limit: usize,
) -> (Vec<SearchMatch>, bool) {
    if query.is_empty() {
        return (Vec::new(), false);
    }
    let needle: Vec<char> = query.to_lowercase().chars().collect();
    let mut out = Vec::new();
    for (row, text) in rows {
        // Lowercasing can expand a character (İ -> i + combining dot).
        // Keep a source-cell column for every folded character so matches
        // still point at the original terminal grid rather than folded text.
        let mut haystack = Vec::with_capacity(text.chars().count());
        let mut source_cols = Vec::with_capacity(haystack.capacity());
        let mut col = 0;
        for grapheme in text.graphemes(true) {
            let end = col + UnicodeWidthStr::width(grapheme).clamp(1, 2);
            for folded in grapheme.to_lowercase().chars() {
                haystack.push(folded);
                source_cols.push((col, end));
            }
            col = end;
        }
        if needle.is_empty() || haystack.len() < needle.len() {
            continue;
        }
        for start in 0..=(haystack.len() - needle.len()) {
            if haystack[start..start + needle.len()] == needle[..] {
                if out.len() == limit {
                    return (out, true);
                }
                out.push(SearchMatch {
                    row,
                    start_col: source_cols[start].0,
                    end_col: source_cols[start + needle.len() - 1].1,
                });
            }
        }
    }
    (out, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_uses_grapheme_cell_columns() {
        let matches = find_matches("Z", [(0, "x\u{0301} 你👩\u{200d}💻Z")].into_iter());
        assert_eq!(matches[0].start_col, 6);
        assert_eq!(matches[0].end_col, 7);
    }

    #[test]
    fn insert_and_backspace_preserve_multibyte_chars() {
        let mut p = TextPrompt::new(PromptKind::Find, "");
        for c in "héllo 日本".chars() {
            p.insert_char(c);
        }
        assert_eq!(p.text(), "héllo 日本");
        p.backspace();
        assert_eq!(p.text(), "héllo 日");
        p.move_home();
        p.delete_forward();
        assert_eq!(p.text(), "éllo 日");
    }

    #[test]
    fn cursor_moves_clamp_at_bounds() {
        let mut p = TextPrompt::new(PromptKind::Find, "ab");
        p.move_right();
        assert_eq!(p.cursor, 2);
        p.move_left();
        p.move_left();
        p.move_left();
        assert_eq!(p.cursor, 0);
    }

    #[test]
    fn find_matches_is_case_insensitive_and_column_accurate() {
        let rows = vec![(0, "Hello World"), (1, "no match here"), (2, "WORLD wide")];
        let matches = find_matches("world", rows.into_iter());
        assert_eq!(
            matches,
            vec![
                SearchMatch {
                    row: 0,
                    start_col: 6,
                    end_col: 11
                },
                SearchMatch {
                    row: 2,
                    start_col: 0,
                    end_col: 5
                },
            ]
        );
    }

    #[test]
    fn find_matches_handles_overlapping_and_multibyte() {
        let rows = vec![(0, "aaaa"), (1, "日本語 日本語")];
        assert_eq!(find_matches("aa", rows.clone().into_iter()).len(), 3);
        let jp = find_matches("日本", rows.into_iter());
        assert_eq!(jp.len(), 2);
        assert_eq!(jp[0].start_col, 0);
        assert_eq!(jp[1].start_col, 7);
    }

    #[test]
    fn lowercase_expansion_keeps_original_grid_columns() {
        let rows = [(0, "İa")];
        let matches = find_matches("a", rows.into_iter());
        assert_eq!(matches[0].start_col, 1);
        assert_eq!(matches[0].end_col, 2);
    }

    #[test]
    fn bounded_search_reports_truncation() {
        let rows = [(0, "aaaa")];
        let (matches, truncated) = find_matches_bounded("a", rows.into_iter(), 2);
        assert_eq!(matches.len(), 2);
        assert!(truncated);
    }

    /// Run explicitly with `cargo test --release -p volt-ui -- --ignored
    /// --nocapture`; not a CI timing assertion because runner speed varies.
    #[test]
    #[ignore]
    fn benchmark_full_scrollback_search() {
        let line = format!("{}needle", "x".repeat(194));
        let rows: Vec<(usize, String)> = (0..10_000).map(|i| (i, line.clone())).collect();
        let start = std::time::Instant::now();
        let (matches, truncated) = find_matches_bounded(
            "needle",
            rows.iter().map(|(i, s)| (*i, s.as_str())),
            100_000,
        );
        let elapsed = start.elapsed();
        assert_eq!(matches.len(), 10_000);
        assert!(!truncated);
        eprintln!("10k x 200 Find scan: {elapsed:?}");
    }

    #[test]
    fn empty_query_matches_nothing() {
        let rows = vec![(0, "anything")];
        assert!(find_matches("", rows.into_iter()).is_empty());
    }

    #[test]
    fn output_refresh_preserves_selected_match() {
        let rows = [(0, "marker"), (1, "marker"), (2, "marker")];
        let mut p = TextPrompt::new(PromptKind::Find, "marker");
        p.replace_matches(find_matches("marker", rows.into_iter()), false);
        p.next_match();
        p.next_match();
        p.invalidate_matches();
        p.invalidate_matches(); // coalesced output must not lose the anchor
        assert!(p.matches.is_empty()); // never render stale highlights
        p.replace_matches(find_matches("marker", rows.into_iter()), false);
        assert_eq!(p.current_match, 2);
        p.next_match();
        assert_eq!(p.current_match, 0);
    }

    #[test]
    fn refreshed_selection_tracks_location_not_result_index() {
        let mut p = TextPrompt::new(PromptKind::Find, "x");
        p.replace_matches(find_matches("x", [(4, "x"), (8, "x")].into_iter()), false);
        p.next_match();
        p.invalidate_matches();
        p.replace_matches(
            find_matches("x", [(2, "x"), (4, "x"), (8, "x")].into_iter()),
            true,
        );
        assert_eq!(p.current_match, 2);
        assert!(p.matches_truncated);
        p.clear_matches(); // editing the query or switching panes resets selection
        p.replace_matches(find_matches("x", [(2, "x"), (8, "x")].into_iter()), false);
        assert_eq!(p.current_match, 0);
        assert!(!p.matches_truncated);
    }

    #[test]
    fn removed_match_and_empty_refresh_are_safe() {
        let mut p = TextPrompt::new(PromptKind::Find, "x");
        p.replace_matches(find_matches("x", [(0, "x"), (1, "x")].into_iter()), false);
        p.next_match();
        p.invalidate_matches();
        p.replace_matches(find_matches("x", [(0, "x")].into_iter()), false);
        assert_eq!(p.current_match, 0);
        p.invalidate_matches();
        p.replace_matches(Vec::new(), false);
        p.next_match();
        p.prev_match();
        assert_eq!(p.current_match, 0);
    }

    #[test]
    fn match_navigation_wraps() {
        let mut p = TextPrompt::new(PromptKind::Find, "x");
        p.matches = vec![
            SearchMatch {
                row: 0,
                start_col: 0,
                end_col: 1,
            },
            SearchMatch {
                row: 1,
                start_col: 0,
                end_col: 1,
            },
        ];
        p.current_match = 1;
        p.next_match();
        assert_eq!(p.current_match, 0);
        p.prev_match();
        assert_eq!(p.current_match, 1);
    }
}
