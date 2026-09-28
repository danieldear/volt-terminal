use volt_core::{events::CoreEvent, performer::Performer};
use vte::Parser;

fn feed(p: &mut Performer, bytes: &[u8]) {
    Parser::new().advance(p, bytes);
}
fn history(p: &mut Performer) {
    for _ in 0..6 {
        feed(p, b"\x1b]133;A\x07$ command\r\noutput\r\n");
    }
}

#[test]
fn navigation_tracks_history_and_eviction() {
    let mut p = Performer::new(20, 3);
    p.set_scrollback_limit(5);
    history(&mut p);
    assert_eq!(p.grid.scrollback_len(), 5);
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 2);
    assert_eq!(p.grid.prompt_scroll_offset(2, true), 4);
    assert_eq!(p.grid.prompt_scroll_offset(4, true), 4);
    assert_eq!(p.grid.prompt_scroll_offset(4, false), 2);
    assert_eq!(p.grid.prompt_scroll_offset(2, false), 0);
    p.set_scrollback_limit(3);
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 2);
    assert_eq!(p.grid.prompt_scroll_offset(2, true), 2);
}

#[test]
fn alt_screen_cannot_spoof_shell_and_main_marks_survive() {
    let mut p = Performer::new(20, 3);
    history(&mut p);
    let offset = p.grid.prompt_scroll_offset(0, true);
    feed(
        &mut p,
        b"\x1b[?1049h\x1b]133;A\x07\x1b]133;C\x07\x1b]133;D;0\x07",
    );
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 0);
    assert!(p.pending_events.is_empty());
    feed(&mut p, b"\x1b[?1049l");
    assert_eq!(p.grid.prompt_scroll_offset(0, true), offset);
}

#[test]
fn resize_clear_and_reset_never_reuse_stale_anchors() {
    let mut p = Performer::new(20, 3);
    history(&mut p);
    p.resize(20, 3); // A no-op resize must preserve anchors.
    assert!(p.grid.prompt_scroll_offset(0, true) > 0);
    p.resize(21, 3);
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 0);
    history(&mut p);
    p.grid.clear_scrollback();
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 0);
    history(&mut p);
    p.reset();
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 0);
}

#[test]
fn destructive_screen_changes_discard_live_marks_but_keep_history() {
    for edit in [
        b"\x1b[2J".as_slice(),
        b"\x1b[1;1H\x1b[L",
        b"\x1b[1;1H\x1b[M",
        b"\x1b[2;3r\x1b[S",
    ] {
        let mut p = Performer::new(20, 3);
        history(&mut p);
        feed(&mut p, b"\x1b]133;A\x07");
        feed(&mut p, edit);
        // Make any incorrectly retained live anchor scroll into history.
        feed(&mut p, b"\x1b[r\x1b[3;1H\r\n\r\n\r\n");
        assert!(p.grid.prompt_scroll_offset(0, true) >= 4);
    }
}

#[test]
fn fragmented_protocol_status_and_duplicate_start() {
    let mut p = Performer::new(20, 3);
    let mut parser = Parser::new();
    for byte in b"\x1b]133;A\x1b\\$ \x1b]133;B\x07\x1b]133;C\x07\x1b]133;C\x07\x1b]133;D;42\x07" {
        parser.advance(&mut p, &[*byte]);
    }
    assert_eq!(p.pending_events.len(), 2);
    assert!(matches!(p.pending_events[0], CoreEvent::CommandStarted));
    assert!(matches!(
        p.pending_events[1],
        CoreEvent::CommandFinished { exit_code: 42, .. }
    ));
    p.pending_events.clear();
    feed(
        &mut p,
        b"\x1b]133;D;bad\x07\x1b]133;D;256\x07\x1b]133;D;-1\x07",
    );
    assert!(p.pending_events.is_empty());
    feed(&mut p, b"\x1b]133;D\x07");
    assert!(matches!(
        p.pending_events[0],
        CoreEvent::CommandFinished {
            exit_code: -1,
            duration_ms: 0
        }
    ));
}

#[test]
fn fish_native_extended_marks_drive_navigation_and_status() {
    // Fish 4.9.3 emits these OSC 133 forms itself. In particular, the A/C
    // extension fields must not stop Volt from recognizing the boundary.
    let mut p = Performer::new(20, 2);
    feed(
        &mut p,
        b"\x1b]133;A;click_events=1\x1b\\fish> \x1b]133;B\x1b\\\
          \x1b]133;C;cmdline_url=false\x1b\\false\r\n\
          \x1b]133;D;1\x1b\\\
          \x1b]133;A;click_events=1\x1b\\fish> \r\n",
    );
    assert!(matches!(p.pending_events[0], CoreEvent::CommandStarted));
    assert!(matches!(
        p.pending_events[1],
        CoreEvent::CommandFinished { exit_code: 1, .. }
    ));
    assert_eq!(p.grid.prompt_scroll_offset(0, true), 1);
}

#[test]
fn prompt_recovers_missing_completion_and_reset_clears_clock() {
    let mut p = Performer::new(20, 3);
    feed(&mut p, b"\x1b]133;C\x07\x1b]133;A\x07");
    assert!(matches!(
        p.pending_events[1],
        CoreEvent::CommandFinished { exit_code: -1, .. }
    ));
    feed(&mut p, b"\x1b]133;C\x07");
    p.reset();
    p.pending_events.clear();
    feed(&mut p, b"\x1b]133;D;0\x07");
    assert!(matches!(
        p.pending_events[0],
        CoreEvent::CommandFinished { duration_ms: 0, .. }
    ));
}
