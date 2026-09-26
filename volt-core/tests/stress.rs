//! Reproducible structure checks, not an independent terminal conformance oracle.
use volt_core::{cell::Cell, performer::Performer};

fn check_row(cells: &[Cell]) {
    for (i, c) in cells.iter().enumerate() {
        if c.is_wide() {
            assert!(
                i + 1 < cells.len() && cells[i + 1].is_continuation(),
                "orphan wide at {i}"
            );
        }
        if c.is_continuation() {
            assert!(
                i > 0 && cells[i - 1].is_wide(),
                "orphan continuation at {i}"
            );
        }
    }
}

#[test]
fn seeded_resize_edit_unicode_history_stress() {
    // 8 deterministic seeds, 5,000 operations each; easy to reproduce in CI.
    for seed in 1..=8u64 {
        let mut rng = seed;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng
        };
        let mut p = Performer::new(80, 24);
        p.set_scrollback_limit(128);
        let mut parser = vte::Parser::new();
        for step in 0..5000 {
            let n = next();
            match n % 12 {
                0 => p.resize((next() % 80 + 1) as usize, (next() % 25 + 1) as usize),
                1 => p.set_scrollback_limit((next() % 128 + 1) as usize),
                2 => parser.advance(&mut p, b"\x1b[?1049h\x1b[?1049l"),
                3 => parser.advance(&mut p, b"\x1b[2J\x1b[H"),
                4 => parser.advance(&mut p, b"\x1b[P\x1b[@\x1b[X"),
                5 => parser.advance(&mut p, b"\x1b[3J\x1b[?6l\x1b[r"),
                6 => parser.advance(&mut p, b"\x1b[?7lABCDE\x1b[?7h"),
                7 => parser.advance(&mut p, b"\x1b[2;4r\x1b[?6h\x1b[99B\x1b[L\x1b[M"),
                8 => parser.advance(&mut p, b"\r\n\t\x08\x1bM"),
                9 => parser.advance(&mut p, b"\x1b7\x1b[999;999H\x1b8"),
                _ => {
                    for byte in "ab日本語👩‍💻x́̈🇺🇸👍🏽\r\n".as_bytes() {
                        parser.advance(&mut p, &[*byte]);
                    }
                }
            }
            assert!(
                p.grid.cursor_col < p.grid.cols && p.grid.cursor_row < p.grid.rows,
                "seed {seed} step {step}"
            );
            assert!(
                p.grid.scroll_top <= p.grid.scroll_bottom && p.grid.scroll_bottom < p.grid.rows
            );
            assert!(p.grid.scrollback_len() <= p.grid.scrollback_limit);
            for row in 0..p.grid.rows {
                check_row(p.grid.row_cells(row));
            }
            for row in 0..p.grid.scrollback_len() {
                check_row(p.grid.scrollback_row(row));
                assert!(
                    !p.grid
                        .row_text(p.grid.scrollback_row(row))
                        .contains('\u{fffd}'),
                    "dangling grapheme ID"
                );
            }
            p.pending_writes.clear();
            p.pending_events.clear();
        }
    }
}

#[test]
fn malformed_stream_stays_bounded_and_reset_recovers() {
    let mut p = Performer::new(0, 0);
    let mut parser = vte::Parser::new();
    let stream = b"\xff\xfe\x1b[999999999999999999999999999;1H\x1bPignored\x1b\\\x1b]52;c;no-clipboard-access\x07\x18";
    for _ in 0..1000 {
        parser.advance(&mut p, stream);
    }
    parser.advance(&mut p, b"\x1bcOK");
    assert_eq!(p.grid.cols, 1);
    assert_eq!(p.grid.cell(0, 0).c(), 'K');
    assert!(p.pending_events.is_empty());
    assert!(p.pending_writes.is_empty());
}
