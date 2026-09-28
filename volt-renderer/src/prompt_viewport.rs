//! A read-only link preview must reveal its beginning (host), not only its tail.
//! Arrow/Home/End navigation pages long destinations without changing the URL.
use unicode_width::UnicodeWidthChar;

pub(crate) fn window(text: &str, cursor: usize, columns: usize) -> (String, usize) {
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let budget = columns.saturating_sub(2).max(1);
    let width = |c: char| UnicodeWidthChar::width(c).unwrap_or(1);
    let mut start = cursor;
    let mut used = 0;
    while start > 0 && used + width(chars[start - 1]) < budget {
        start -= 1;
        used += width(chars[start]);
    }
    let mut end = start;
    used = 0;
    while end < chars.len() && used + width(chars[end]) <= budget {
        used += width(chars[end]);
        end += 1;
    }
    let mut display = String::new();
    if start > 0 {
        display.push('‹');
    }
    display.extend(&chars[start..end]);
    if end < chars.len() {
        display.push('›');
    }
    (display, cursor - start + usize::from(start > 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;
    #[test]
    fn host_first_and_arrow_navigation_reveals_tail() {
        let uri = "https://example.test/a/very/long/path";
        let (start, cursor) = window(uri, 0, 25);
        assert!(start.starts_with("https://example.test/"));
        assert_eq!(cursor, 0);
        assert!(start.ends_with('›'));
        let (end, cursor) = window(uri, uri.chars().count(), 25);
        assert!(end.ends_with("/long/path"));
        assert!(end.starts_with('‹'));
        assert_eq!(cursor, end.chars().count());
    }
    #[test]
    fn unicode_windows_are_bounded_and_cursor_is_valid() {
        let uri = "https://example.test/日本語/e\u{301}";
        for cols in 4..32 {
            for cursor in 0..=uri.chars().count() + 2 {
                let (text, at) = window(uri, cursor, cols);
                assert!(text.width() <= cols);
                assert!(at <= text.chars().count());
            }
        }
    }
}
