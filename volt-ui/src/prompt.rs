//! Shared modal text-prompt overlay: a single-line text input rendered
//! centered over the terminal. Used for Find (Cmd+F) and the
//! Change Tab Title.../Change Terminal Title... context-menu actions.
//!
//! The buffer is a `Vec<char>` rather than raw byte offsets into a `String`
//! specifically to avoid UTF-8 char-boundary bugs when inserting/deleting at
//! an arbitrary cursor position with multi-byte input.

pub use volt_core::search::{find_matches, find_matches_bounded, SearchMatch};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptKind {
    Find,
    RenameTab,
    RenameTerminal,
    OpenLink,
    /// "Run task?" for tasks marked `confirm = true`; shows the command.
    ConfirmTask,
    /// Explicit permission for just this command, not trust for its whole file.
    ReviewTask,
    TrustTasks,
    ReviewPaste,
    ReviewClipboardWrite,
}

impl PromptKind {
    /// Confirmations show text the user can read and inspect, not edit.
    pub fn read_only(self) -> bool {
        matches!(
            self,
            Self::OpenLink
                | Self::ConfirmTask
                | Self::ReviewTask
                | Self::TrustTasks
                | Self::ReviewPaste
                | Self::ReviewClipboardWrite
        )
    }
}

pub struct TextPrompt {
    pub kind: PromptKind,
    pub task_review: Option<volt_renderer::task_review::TaskReview>,
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
        let cursor = if kind.read_only() { 0 } else { chars.len() };
        Self {
            kind,
            task_review: None,
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
            PromptKind::OpenLink => "Open link? Enter: open / Esc: cancel",
            PromptKind::ConfirmTask => "Run task? Enter: run / Esc: cancel",
            PromptKind::ReviewTask => "Review task · Enter: run once / Esc: cancel",
            PromptKind::TrustTasks => "Review task permissions",
            PromptKind::ReviewPaste => "Review paste",
            PromptKind::ReviewClipboardWrite => "Terminal clipboard request",
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
        if self.kind.read_only() || c.is_control() {
            return;
        }
        self.chars.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.kind.read_only() || self.cursor == 0 {
            return;
        }
        self.chars.remove(self.cursor - 1);
        self.cursor -= 1;
    }

    pub fn delete_forward(&mut self) {
        if !self.kind.read_only() && self.cursor < self.chars.len() {
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_review_is_read_only_and_explicitly_authorizes_only_one_run() {
        for kind in [
            PromptKind::ReviewTask,
            PromptKind::ConfirmTask,
            PromptKind::TrustTasks,
        ] {
            let mut prompt = TextPrompt::new(kind, "(cd '/repo' && cargo build --release)");
            prompt.move_end();
            prompt.insert_char(';');
            prompt.backspace();
            prompt.delete_forward();
            assert_eq!(prompt.text(), "(cd '/repo' && cargo build --release)");
            assert!(kind.read_only());
        }
        assert!(TextPrompt::new(PromptKind::ReviewTask, "echo OK")
            .title()
            .contains("run once"));
    }

    #[test]
    fn link_confirmation_cannot_be_edited_by_typing_paste_or_ime() {
        let mut p = TextPrompt::new(PromptKind::OpenLink, "https://actual.test/path");
        assert_eq!(p.cursor, 0);
        p.move_end();
        p.insert_char('x');
        p.backspace();
        p.move_home();
        p.delete_forward();
        assert_eq!(p.text(), "https://actual.test/path");
        assert_eq!(p.title(), "Open link? Enter: open / Esc: cancel");
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
