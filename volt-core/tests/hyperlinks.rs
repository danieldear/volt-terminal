use volt_core::{grid::Grid, hyperlink::Hyperlink, performer::Performer};
use vte::Parser;

fn feed(p: &mut Performer, text: &str) {
    Parser::new().advance(p, text.as_bytes());
}
fn linked(p: &mut Performer, uri: &str, text: &str) {
    feed(
        p,
        &format!("\x1b]8;id=build;{uri}\x1b\\{text}\x1b]8;;\x1b\\"),
    );
}
fn uri(g: &Grid, col: usize, row: usize) -> Option<&str> {
    g.hyperlink(g.cell(col, row)).map(|l| l.uri())
}
#[test]
fn open_close_sgr_and_repeated_labels_keep_distinct_destinations() {
    let mut p = Performer::new(30, 3);
    linked(&mut p, "https://one.test/a;b", "same");
    linked(&mut p, "https://two.test", "same");
    feed(&mut p, " plain");
    assert_eq!(uri(&p.grid, 0, 0), Some("https://one.test/a;b"));
    assert_eq!(uri(&p.grid, 4, 0), Some("https://two.test"));
    assert_eq!(uri(&p.grid, 8, 0), None);
    assert!(p
        .grid
        .row_text(p.grid.row_cells(0))
        .starts_with("samesame plain"));
    feed(
        &mut p,
        "\r\n\x1b]8;;https://one.test\x07\x1b[31mA\x1b[0mB\x1b]8;;\x07C",
    );
    assert_eq!(uri(&p.grid, 1, 1), Some("https://one.test"));
    assert_eq!(uri(&p.grid, 2, 1), None);
}
#[test]
fn chunk_boundaries_and_bell_terminators() {
    let mut p = Performer::new(10, 2);
    let mut parser = Parser::new();
    for b in b"\x1b]8;id=abc;https://a.test\x07go\x1b]8;;\x07!" {
        parser.advance(&mut p, &[*b]);
    }
    assert_eq!(uri(&p.grid, 0, 0), Some("https://a.test"));
    assert_eq!(p.grid.hyperlink(p.grid.cell(0, 0)).unwrap().id(), "abc");
    assert_eq!(uri(&p.grid, 2, 0), None);
}
#[test]
fn unicode_extensions_wide_partners_and_wrap_preserve_links() {
    let mut p = Performer::new(5, 4);
    linked(&mut p, "https://unicode.test", "abc界e\u{301}👩‍💻");
    assert_eq!(uri(&p.grid, 3, 0), Some("https://unicode.test"));
    assert!(p.grid.cell(4, 0).is_continuation());
    assert_eq!(p.grid.extended_text(p.grid.cell(0, 1)), Some("e\u{301}"));
    assert_eq!(uri(&p.grid, 0, 1), Some("https://unicode.test"));
    assert_eq!(uri(&p.grid, 1, 1), Some("https://unicode.test"));
    // A combining mark belongs to its base even after OSC 8 closes.
    linked(&mut p, "https://base.test", "x");
    feed(&mut p, "\u{301}");
    assert_eq!(uri(&p.grid, 3, 1), Some("https://base.test"));
}
#[test]
fn erase_overwrite_insert_and_delete_do_not_leave_ghost_links() {
    let mut p = Performer::new(12, 2);
    linked(&mut p, "https://edit.test", "abcde");
    feed(&mut p, "\rX");
    assert_eq!(uri(&p.grid, 0, 0), None);
    assert_eq!(uri(&p.grid, 1, 0), Some("https://edit.test"));
    feed(&mut p, "\x1b[2@ ");
    assert_eq!(uri(&p.grid, 1, 0), None);
    assert_eq!(uri(&p.grid, 3, 0), Some("https://edit.test"));
    feed(&mut p, "\r\x1b[3P");
    assert_eq!(p.grid.cell_char(p.grid.cell(0, 0)), 'b');
    assert_eq!(uri(&p.grid, 0, 0), Some("https://edit.test"));
    feed(&mut p, "\x1b[2K");
    assert!((0..12).all(|col| uri(&p.grid, col, 0).is_none()));
}
#[test]
fn scrollback_eviction_reflow_and_grid_import_remap_ids() {
    let mut p = Performer::new(5, 2);
    p.set_scrollback_limit(2);
    linked(&mut p, "https://history.test", "abcdefghi");
    feed(&mut p, "\r\n\r\n");
    assert_eq!(
        p.grid
            .hyperlink(p.grid.scrollback_cell(0, 0))
            .unwrap()
            .uri(),
        "https://history.test"
    );
    let mut other = Grid::new(5, 2);
    other.copy_from_grid(0, 0, &p.grid, p.grid.scrollback_row(0));
    assert_eq!(uri(&other, 0, 0), Some("https://history.test"));
    p.resize(3, 3);
    for r in 0..p.grid.scrollback_len() {
        for cell in p.grid.scrollback_row(r) {
            if p.grid.cell_char(cell).is_ascii_alphabetic() {
                assert_eq!(
                    p.grid.hyperlink(cell).unwrap().uri(),
                    "https://history.test"
                );
            }
        }
    }
    feed(&mut p, "\r\n\r\n\r\n\r\n\r\n");
    assert!((0..p.grid.scrollback_len()).all(|r| p
        .grid
        .scrollback_row(r)
        .iter()
        .all(|c| p.grid.hyperlink(c).is_none())));
}
#[test]
fn visible_reflow_and_wide_overwrite() {
    let mut p = Performer::new(10, 4);
    linked(&mut p, "https://reflow.test", "abcdef界");
    p.resize(4, 5);
    assert_eq!(uri(&p.grid, 0, 0), Some("https://reflow.test"));
    assert_eq!(uri(&p.grid, 0, 1), Some("https://reflow.test"));
    assert_eq!(uri(&p.grid, 2, 1), Some("https://reflow.test"));
    feed(&mut p, "\x1b[2;4HX");
    assert_eq!(uri(&p.grid, 2, 1), None);
    assert_eq!(uri(&p.grid, 3, 1), None);
}
#[test]
fn alt_screen_and_reset_do_not_leak_active_link() {
    let mut p = Performer::new(20, 2);
    feed(&mut p, "\x1b]8;;https://main.test\x07A\x1b[?1049hB");
    assert_eq!(uri(&p.grid, 0, 0), None);
    linked(&mut p, "https://alt.test", "C");
    feed(&mut p, "\x1b[?1049lD");
    assert_eq!(uri(&p.grid, 0, 0), Some("https://main.test"));
    assert_eq!(uri(&p.grid, 1, 0), None);
    feed(&mut p, "\x1b]8;;https://reset.test\x07\x1bcZ");
    assert_eq!(uri(&p.grid, 0, 0), None);
    assert!(!p.grid.has_extended_text());
}
#[test]
fn malformed_sequences_close_link_and_bounds_reject_oversize() {
    let mut p = Performer::new(20, 2);
    feed(&mut p, "\x1b]8;;https://a.test\x07A\x1b]8;invalid\x07B");
    assert_eq!(uri(&p.grid, 1, 0), None);
    feed(
        &mut p,
        &format!(
            "\x1b]8;;https://truncated.test/{}\x07C",
            ";segment".repeat(20)
        ),
    );
    assert_eq!(uri(&p.grid, 2, 0), None);
    let long = "a".repeat(2049);
    assert!(Hyperlink::from_osc(&[b"8", b"", long.as_bytes()]).is_none());
    assert!(Hyperlink::from_osc(&[b"8", b"", b"\xff"]).is_none());
    assert!(Hyperlink::from_osc(&[b"8", b"", b"https://a.test\n"]).is_none());
    assert!(Hyperlink::from_osc(&[
        b"8",
        format!("id={}", "i".repeat(129)).as_bytes(),
        b"https://a.test"
    ])
    .is_none());
}
#[test]
fn resource_cap_degrades_to_text_and_explicit_cleanup_reclaims_links() {
    let mut p = Performer::new(8, 2);
    for i in 0..4100 {
        feed(&mut p, "\r");
        linked(&mut p, &format!("https://cap.test/{i}"), "x");
    }
    assert_eq!(p.grid.cell_char(p.grid.cell(0, 0)), 'x');
    assert_eq!(uri(&p.grid, 0, 0), None);
    p.grid.clear_scrollback();
    feed(&mut p, "\r");
    linked(&mut p, "https://reclaimed.test", "y");
    assert_eq!(uri(&p.grid, 0, 0), Some("https://reclaimed.test"));
}
