//! Find in a terminal's text, in screen columns. Moved from Volt
//! Terminal's UI to share it.
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

    #[test]
    fn empty_query_matches_nothing() {
        let rows = vec![(0, "anything")];
        assert!(find_matches("", rows.into_iter()).is_empty());
    }
}
