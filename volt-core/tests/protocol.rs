use volt_config::Color;
use volt_core::{cell::CellColor, performer::Performer};

fn feed(p: &mut Performer, bytes: &[u8]) {
    vte::Parser::new().advance(p, bytes);
}

#[test]
fn combined_dec_modes_apply_every_parameter() {
    let mut p = Performer::new(10, 5);
    feed(&mut p, b"\x1b[?1;25;1003;1006;2004h");
    assert!(p.application_cursor_keys_mode());
    assert!(p.cursor_visible);
    assert!(p.mouse_reporting_enabled());
    assert!(p.mouse_sgr_mode());
    assert!(p.bracketed_paste_mode());
    feed(&mut p, b"\x1b[?1;25;1003;1006;2004l");
    assert!(!p.application_cursor_keys_mode());
    assert!(!p.cursor_visible);
    assert!(!p.mouse_reporting_enabled());
    assert!(!p.mouse_sgr_mode());
    assert!(!p.bracketed_paste_mode());
}

#[test]
fn margins_default_reset_and_origin_relative_cpr() {
    let mut p = Performer::new(10, 6);
    feed(&mut p, b"\x1b[2;5r\x1b[?6h\x1b[2;3H\x1b[6n");
    assert_eq!((p.grid.cursor_row, p.grid.cursor_col), (2, 2));
    assert_eq!(p.pending_writes.pop().unwrap(), b"\x1b[2;3R");
    feed(&mut p, b"\x1b[99B");
    assert_eq!(p.grid.cursor_row, 4);
    feed(&mut p, b"\x1b[99A");
    assert_eq!(p.grid.cursor_row, 1);
    feed(&mut p, b"\x1b[r");
    assert_eq!((p.grid.scroll_top, p.grid.scroll_bottom), (0, 5));
    feed(&mut p, b"\x1b[3r");
    assert_eq!((p.grid.scroll_top, p.grid.scroll_bottom), (2, 5));
}

#[test]
fn autowrap_can_be_disabled_and_bell_does_not_cancel_pending_wrap() {
    let mut p = Performer::new(3, 3);
    feed(&mut p, b"\x1b[?7lABCD");
    assert_eq!(p.grid.row_text(p.grid.row_cells(0)), "ABD");
    assert_eq!(p.grid.cursor_row, 0);
    feed(&mut p, b"\x1b[?7h\rABC\x07Z");
    assert_eq!(p.grid.cell(0, 1).c(), 'Z');
}

#[test]
fn erase_saved_lines_preserves_visible_screen_and_cursor() {
    let mut p = Performer::new(8, 2);
    feed(&mut p, b"one\r\ntwo\r\nthree");
    assert!(p.grid.scrollback_len() > 0);
    let row = p.grid.row_text(p.grid.row_cells(1));
    let cursor = (p.grid.cursor_col, p.grid.cursor_row);
    feed(&mut p, b"\x1b[3J");
    assert_eq!(p.grid.scrollback_len(), 0);
    assert_eq!(p.grid.row_text(p.grid.row_cells(1)), row);
    assert_eq!((p.grid.cursor_col, p.grid.cursor_row), cursor);
}

#[test]
fn unsupported_private_and_intermediate_commands_do_not_alias_supported_ones() {
    let mut p = Performer::new(8, 4);
    feed(&mut p, b"hello\x1b[2;3H\x1b[?1A\x1b[1 q\x1b[>2J\x1b#c");
    assert_eq!((p.grid.cursor_row, p.grid.cursor_col), (1, 2));
    assert_eq!(p.grid.cell(0, 0).c(), 'h');
}

#[test]
fn line_insert_delete_outside_margins_do_nothing() {
    let mut p = Performer::new(5, 5);
    feed(&mut p, b"A\r\nB\r\nC\r\nD\r\nE\x1b[2;4r\x1b[1;1H\x1b[L");
    assert_eq!(p.grid.cell(0, 0).c(), 'A');
    assert_eq!(p.grid.cell(0, 1).c(), 'B');
    assert_eq!(p.grid.cell(0, 4).c(), 'E');
}

#[test]
fn colon_rgb_with_optional_colorspace_and_ind_nel() {
    let mut p = Performer::new(10, 4);
    feed(&mut p, b"\x1b[38:2::12:34:56mA\x1bD B\x1bEC");
    assert_eq!(p.grid.cell(0, 0).fg, CellColor::Rgb(Color::rgb(12, 34, 56)));
    assert_eq!(p.grid.cell(2, 1).c(), 'B');
    assert_eq!(p.grid.cell(0, 2).c(), 'C');
}

#[test]
fn alt_screen_cursor_save_does_not_destroy_main_return_position() {
    let mut p = Performer::new(10, 6);
    feed(&mut p, b"\x1b[3;4H\x1b[?1049h\x1b[5;8H\x1b7\x1b[?1049l");
    assert_eq!((p.grid.cursor_row, p.grid.cursor_col), (2, 3));
}

#[test]
fn split_bytes_produce_same_screen_modes_and_replies() {
    let bytes =
        "\x1b[2;5r\x1b[?6h\x1b[2;3H日本語 👩‍💻 x́̈\x1b[6n\x1b[?6l\x1b[?1;2004h\x1b[38:2::12:34:56mZ"
            .as_bytes();
    let mut reference = Performer::new(20, 6);
    feed(&mut reference, bytes);
    for size in 1..=bytes.len() {
        let mut p = Performer::new(20, 6);
        let mut parser = vte::Parser::new();
        for chunk in bytes.chunks(size) {
            parser.advance(&mut p, chunk);
        }
        for r in 0..6 {
            assert_eq!(
                p.grid.row_text(p.grid.row_cells(r)),
                reference.grid.row_text(reference.grid.row_cells(r)),
                "chunk {size} row {r}"
            );
        }
        assert_eq!(p.pending_writes, reference.pending_writes);
        assert_eq!(p.bracketed_paste_mode(), reference.bracketed_paste_mode());
        assert_eq!(
            (p.grid.cursor_row, p.grid.cursor_col),
            (reference.grid.cursor_row, reference.grid.cursor_col)
        );
    }
}

#[test]
fn no_wrap_combining_extends_last_column_not_previous_one() {
    let mut p = Performer::new(3, 2);
    feed(&mut p, "\x1b[?7lABx\u{0301}\u{0308}".as_bytes());
    assert_eq!(p.grid.row_text(p.grid.row_cells(0)), "ABx́̈");
    assert_eq!(p.grid.cursor_row, 0);
}

#[test]
fn reenter_alt_with_origin_and_existing_margins_keeps_cursor_valid() {
    let mut p = Performer::new(10, 6);
    feed(
        &mut p,
        b"\x1b[?6h\x1b[?1049h\x1b[2;5r\x1b[?1049l\x1b[?1049h\x1b[6n",
    );
    assert!(p.grid.cursor_row >= p.grid.scroll_top);
    assert_eq!(p.pending_writes.pop().unwrap(), b"\x1b[1;1R");
}

#[test]
fn dec_save_restores_attributes_and_main_saved_cursor_survives_alt() {
    let mut p = Performer::new(10, 6);
    feed(
        &mut p,
        b"\x1b[31;44;1m\x1b[2;3H\x1b7\x1b[?1049h\x1b7\x1b[?1049l\x1b[0m\x1b[H\x1b8X",
    );
    let c = p.grid.cell(2, 1);
    assert_eq!(c.c(), 'X');
    assert_eq!(c.fg, CellColor::Indexed(1));
    assert_eq!(c.bg, CellColor::Indexed(4));
    assert!(c.bold);
}

#[test]
fn delete_lines_does_not_create_scrollback() {
    let mut p = Performer::new(10, 3);
    feed(&mut p, b"A\r\nB\r\nC\x1b[H\x1b[M");
    assert_eq!(p.grid.scrollback_len(), 0);
    assert_eq!(p.grid.cell(0, 0).c(), 'B');
}

#[test]
fn hpa_cancels_deferred_wrap() {
    let mut p = Performer::new(3, 2);
    feed(&mut p, b"ABC\x1b[1`Z");
    assert_eq!(p.grid.row_text(p.grid.row_cells(0)), "ZBC");
    assert_eq!(p.grid.cursor_row, 0);
}

#[test]
fn cursor_restore_does_not_restore_autowrap_mode() {
    let mut p = Performer::new(3, 2);
    feed(&mut p, b"\x1b7\x1b[?7l\x1b8ABCD");
    assert_eq!(p.grid.row_text(p.grid.row_cells(0)), "ABD");
    assert_eq!(p.grid.cursor_row, 0);
}
