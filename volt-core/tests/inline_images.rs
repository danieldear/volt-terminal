use volt_core::{events::CoreEvent, performer::Performer};
use vte::Perform;

#[test]
fn captures_absolute_anchor_and_coalesces_without_moving_cursor() {
    let mut p = Performer::new(8, 2);
    let mut parser = volt_core::parser::Parser::new();
    parser.advance(&mut p, b"a\r\nb\r\nc");
    let row = p.grid.viewport_absolute_row(0, p.grid.cursor_row).unwrap() as u64;
    let cursor = (p.grid.cursor_col, p.grid.cursor_row);
    p.osc_dispatch(&[b"1337", b"File=inline=1", b"width=2:AAAA"], false);
    p.osc_dispatch(&[b"1337", b"File=inline=1:AAAA"], false);
    assert!(
        matches!(p.pending_events.as_slice(), [CoreEvent::InlineImageRequested {
        parameters, row: r, column: 1, alternate_screen: false, epoch: 1
    }] if *r == row && parameters == b"File=inline=1;width=2:AAAA")
    );
    assert_eq!((p.grid.cursor_col, p.grid.cursor_row), cursor);
}

#[test]
fn bounds_requests_and_preserves_clipboard_event() {
    let mut p = Performer::new(8, 2);
    let huge = vec![b'A'; 64 * 1024 + 1];
    p.osc_dispatch(&[b"1337", b"File=inline=1", &huge], true);
    p.osc_dispatch(&[b"1337", b"not-a-file"], true);
    assert!(p.pending_events.is_empty());
    p.osc_dispatch(&[b"52", b"c", b"aGk="], true);
    p.osc_dispatch(&[b"1337", b"File=inline=1:AAAA"], true);
    assert!(
        matches!(&p.pending_events[0], CoreEvent::ClipboardWriteRequested(data) if data == b"hi")
    );
    assert!(matches!(
        &p.pending_events[1],
        CoreEvent::InlineImageRequested { .. }
    ));
}

#[test]
fn resize_reset_and_alt_reuse_invalidate_but_scroll_does_not() {
    let mut p = Performer::new(8, 2);
    let mut parser = volt_core::parser::Parser::new();
    parser.advance(&mut p, b"a\r\nb\r\nc");
    assert_eq!(p.inline_image_epoch(false), 1);
    p.resize(8, 2);
    assert_eq!(p.inline_image_epoch(false), 1);
    p.resize(9, 2);
    assert_eq!(p.inline_image_epoch(false), 2);
    parser.advance(&mut p, b"\x1b[?1049h");
    let alt_epoch = p.inline_image_epoch(true);
    parser.advance(&mut p, b"\x1b[?1049l\x1b[?1049h");
    assert!(p.inline_image_epoch(true) > alt_epoch);
    assert_eq!(p.inline_image_epoch(false), 2);
    p.reset();
    assert_eq!(p.inline_image_epoch(false), 3);
}

#[test]
fn recycled_nonhistory_and_partial_rows_invalidate_overlays() {
    let mut p = Performer::new(8, 2);
    let mut parser = volt_core::parser::Parser::new();
    p.grid.scrollback_limit = 0; // direct public-grid no-history mode
    parser.advance(&mut p, b"a\r\nb\r\nc");
    assert!(p.inline_image_epoch(false) > 1);
    parser.advance(&mut p, b"\x1b[?1049h");
    let epoch = p.inline_image_epoch(true);
    parser.advance(&mut p, b"a\r\nb\r\nc");
    assert!(p.inline_image_epoch(true) > epoch);
    let epoch = p.inline_image_epoch(true);
    parser.advance(&mut p, b"\x1b[1;1H\x1b[L");
    assert!(p.inline_image_epoch(true) > epoch);
}
