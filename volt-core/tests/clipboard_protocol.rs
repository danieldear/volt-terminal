//! Host clipboard access must never happen in parser tests.
use volt_core::parser::Parser;
use volt_core::{events::CoreEvent, performer::Performer};

#[test]
fn copy_is_chunk_stable_and_read_requests_never_answer() {
    for chunk in 1..=20 {
        let mut p = Performer::new(80, 24);
        let mut parser = Parser::new();
        for bytes in b"before\x1b]52;c;aGVsbG8=\x1b\\after".chunks(chunk) {
            parser.advance(&mut p, bytes);
        }
        assert!(
            matches!(p.pending_events.as_slice(), [CoreEvent::ClipboardWriteRequested(v)] if v == b"hello")
        );
        assert!(p
            .grid
            .row_text(p.grid.row_cells(0))
            .starts_with("beforeafter"));
        p.pending_events.clear();
        parser.advance(&mut p, b"\x1b]52;c;?\x07");
        assert!(p.pending_events.is_empty());
        assert!(p.pending_writes.is_empty());
    }
}

#[test]
fn malformed_or_oversized_requests_do_not_change_terminal_state() {
    for payload in ["a", "!!!!", "AA==", "wA==", "aGVsbG8="].iter() {
        let mut p = Performer::new(80, 24);
        let mut parser = Parser::new();
        // Primary-selection and clipboard-manager channels are not supported.
        parser.advance(&mut p, format!("\x1b]52;p;{payload}\x07OK").as_bytes());
        assert!(p.pending_events.is_empty());
        assert!(p.grid.row_text(p.grid.row_cells(0)).starts_with("OK"));
    }
    let mut p = Performer::new(80, 24);
    let mut parser = Parser::new();
    parser.advance(
        &mut p,
        format!("\x1b]52;c;{}\x07OK", "A".repeat(70_000)).as_bytes(),
    );
    assert!(p.pending_events.is_empty());
    assert!(p.grid.row_text(p.grid.row_cells(0)).starts_with("OK"));
}

#[test]
fn output_flood_keeps_only_one_pending_copy_per_drain() {
    let mut p = Performer::new(80, 24);
    let mut parser = Parser::new();
    for _ in 0..1000 {
        parser.advance(&mut p, b"\x1b]52;c;aGVsbG8=\x07");
    }
    assert_eq!(p.pending_events.len(), 1);
    p.pending_events.clear();
    parser.advance(&mut p, b"\x1b]52;c;\x07");
    assert!(
        matches!(p.pending_events.as_slice(), [CoreEvent::ClipboardWriteRequested(v)] if v.is_empty())
    );
}
