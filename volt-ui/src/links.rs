//! Bounded, on-demand link detection. Never invoked by printing or rendering.
use volt_core::grid::Grid;

const MAX_BYTES: usize = 8192;
const MAX_ROWS: usize = 128;

/// Only web URLs can leave the terminal. In particular, file:, shell commands,
/// control characters and bidirectional formatting characters are not accepted.
pub fn safe_web_url(url: &str) -> bool {
    if url.len() > MAX_BYTES
        || url.chars().any(|c| {
            c.is_whitespace()
                || c.is_control()
                || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\\')
        })
    {
        return false;
    }
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    !authority.is_empty() && authority != "." && !authority.contains('@')
}

#[derive(Debug, PartialEq, Eq)]
pub struct LinkTarget {
    pub uri: String,
    pub explicit: bool,
}

/// An OSC 8 destination takes precedence over URL-looking label text. Unsafe
/// explicit destinations fail closed; never fall back to the deceptive label.
pub fn target_at(grid: &Grid, offset: usize, col: usize, row: usize) -> Option<LinkTarget> {
    if col >= grid.cols || row >= grid.rows {
        return None;
    }
    let history = grid.scrollback_len();
    let hit_row = history - offset.min(history) + row;
    let cells = if hit_row < history {
        grid.scrollback_row(hit_row)
    } else {
        grid.row_cells(hit_row - history)
    };
    let col = if cells[col].is_continuation() {
        col.saturating_sub(1)
    } else {
        col
    };
    if let Some(link) = grid.hyperlink(&cells[col]) {
        return safe_web_url(link.uri()).then(|| LinkTarget {
            uri: link.uri().to_string(),
            explicit: true,
        });
    }
    url_at(grid, offset, col, row).map(|uri| LinkTarget {
        uri,
        explicit: false,
    })
}

/// Hit-test terminal cells, including wide graphemes and soft-wrapped history.
/// Hard line breaks never join two URLs. Extremely long logical rows fail closed.
pub fn url_at(grid: &Grid, offset: usize, col: usize, row: usize) -> Option<String> {
    if col >= grid.cols || row >= grid.rows {
        return None;
    }
    let history = grid.scrollback_len();
    let hit_row = history - offset.min(history) + row;
    let wrapped = |r| {
        if r < history {
            grid.scrollback_row_soft_wrapped(r)
        } else {
            grid.row_soft_wrapped(r - history)
        }
    };
    let mut first = hit_row;
    while first > 0 && wrapped(first - 1) {
        first -= 1;
        if hit_row - first >= MAX_ROWS {
            return None;
        }
    }
    let mut text = String::new();
    let mut hit = None;
    let mut previous = 0;
    for r in first..(first + MAX_ROWS).min(history + grid.rows) {
        let cells = if r < history {
            grid.scrollback_row(r)
        } else {
            grid.row_cells(r - history)
        };
        for (c, cell) in cells.iter().enumerate() {
            if r == hit_row && c == col {
                if cell.is_wrap_spacer() {
                    return None;
                }
                hit = Some(if cell.is_continuation() {
                    previous
                } else {
                    text.len()
                });
            }
            if !cell.is_continuation() && !cell.is_wrap_spacer() {
                previous = text.len();
                grid.push_cell_text(&mut text, cell);
            }
            if text.len() > MAX_BYTES {
                return None;
            }
        }
        if !wrapped(r) {
            return token_at(&text, hit?);
        }
    }
    None
}

fn token_at(text: &str, hit: usize) -> Option<String> {
    let delimiter = |c: char| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'');
    let start = text[..hit]
        .rfind(delimiter)
        .map_or(0, |n| n + text[n..].chars().next().unwrap().len_utf8());
    let end = text[hit..].find(delimiter).map_or(text.len(), |n| hit + n);
    let raw = &text[start..end];
    // Permit prose wrappers, without losing balanced parentheses in URL paths.
    let leading = raw.len() - raw.trim_start_matches(['(', '[', '{']).len();
    let mut token = &raw[leading..];
    loop {
        let last = token.chars().last()?;
        let trim = match last {
            '.' | ',' | ';' | ':' | '!' => true,
            ')' | ']' | '}' => {
                let open = match last {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                token.chars().filter(|c| *c == last).count()
                    > token.chars().filter(|c| *c == open).count()
            }
            _ => false,
        };
        if !trim {
            break;
        }
        token = &token[..token.len() - last.len_utf8()];
    }
    if hit < start + leading || hit >= start + leading + token.len() || !safe_web_url(token) {
        return None;
    }
    Some(token.to_owned())
}

pub fn open(url: &str) -> std::io::Result<()> {
    if !safe_web_url(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unsafe link",
        ));
    }
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(target_os = "macos"))]
    let program = "xdg-open";
    // No shell interpolation. Reap the launcher without blocking the UI.
    let mut child = std::process::Command::new(program).arg(url).spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use volt_core::performer::Performer;
    use vte::Parser;
    fn terminal(text: &str, cols: usize, rows: usize) -> Grid {
        let mut p = Performer::new(cols, rows);
        Parser::new().advance(&mut p, text.as_bytes());
        p.grid
    }
    #[test]
    fn explicit_destinations_win_and_unsafe_links_do_not_fall_back_to_labels() {
        let g = terminal(
            "\x1b]8;;https://actual.test\x07https://label.test 界\x1b]8;;\x07",
            40,
            2,
        );
        assert_eq!(
            target_at(&g, 0, 3, 0),
            Some(LinkTarget {
                uri: "https://actual.test".into(),
                explicit: true
            })
        );
        assert_eq!(target_at(&g, 0, 20, 0).unwrap().uri, "https://actual.test");
        let g = terminal(
            "\x1b]8;;file:///secret\x07https://label.test\x1b]8;;\x07",
            40,
            2,
        );
        assert!(target_at(&g, 0, 5, 0).is_none());
        let g = terminal(
            "\x1b]8;;https://actual.test\x07日本日本日本日本\x1b]8;;\x07",
            4,
            2,
        );
        assert_eq!(target_at(&g, 2, 1, 0).unwrap().uri, "https://actual.test");
    }

    #[test]
    fn unicode_hit_mapping_and_wrapped_history() {
        let g = terminal("日本 https://example.org/日本?q=1", 12, 2);
        assert_eq!(
            url_at(&g, 1, 7, 0).as_deref(),
            Some("https://example.org/日本?q=1")
        );
        let g = terminal("https://example.org/日本", 80, 2);
        assert_eq!(
            url_at(&g, 0, 20, 0).as_deref(),
            Some("https://example.org/日本")
        );
        assert!(url_at(&g, 0, 50, 0).is_none());
    }
    #[test]
    fn prose_punctuation_and_safe_schemes() {
        assert!(safe_web_url("HTTPS://example.org/path"));
        let t = "(https://example.org/foo(bar)).";
        assert_eq!(
            token_at(t, 10).as_deref(),
            Some("https://example.org/foo(bar)")
        );
        assert!(token_at(t, 0).is_none());
        for bad in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://",
            "https://a\n",
            "https://a\u{202e}b",
            "https://user@a",
            "https://a\\b",
        ] {
            assert!(!safe_web_url(bad), "{bad:?}");
        }
    }
    #[test]
    fn hard_lines_do_not_join_and_long_lines_fail_closed() {
        let g = terminal("https://example.org\r\n/another/path", 40, 3);
        assert_eq!(url_at(&g, 0, 0, 0).as_deref(), Some("https://example.org"));
        assert!(url_at(&g, 0, 0, 1).is_none());
        let g = terminal(&format!("https://a/{}", "x".repeat(MAX_BYTES)), 100, 100);
        assert!(url_at(&g, 0, 5, 0).is_none());
    }
}
