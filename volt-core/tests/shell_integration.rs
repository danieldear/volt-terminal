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
fn resize_remaps_anchors_and_clear_reset_remove_them() {
    let mut p = Performer::new(20, 3);
    history(&mut p);
    p.resize(20, 3); // A no-op resize must preserve anchors.
    assert!(p.grid.prompt_scroll_offset(0, true) > 0);
    let before = p.grid.prompt_scroll_offset(0, true);
    p.resize(21, 3);
    assert_eq!(p.grid.prompt_scroll_offset(0, true), before);
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

#[test]
fn command_boundaries_follow_wrap_reflow_history_and_exclude_next_prompt() {
    let mut p = Performer::new(8, 4);
    feed(
        &mut p,
        b"$ cmd\r\n\x1b]133;C\x07abcdefghijkl\r\n\x1b]133;D;0\x07$ next",
    );
    assert_eq!(p.grid.latest_command_output(), Some(((1, 0), (3, 0))));
    p.resize(20, 4);
    assert_eq!(p.grid.latest_command_output(), Some(((1, 0), (2, 0))));
    p.resize(5, 4);
    assert_eq!(p.grid.latest_command_output(), Some(((1, 0), (4, 0))));
    feed(&mut p, b"\x1b[?1049h\x1b]133;C\x07fake\x1b]133;D;0\x07");
    assert_eq!(p.grid.latest_command_output(), None);
    feed(&mut p, b"\x1b[?1049l");
    assert!(p.grid.latest_command_output().is_some());
    p.set_scrollback_limit(1);
    feed(&mut p, b"\r\n1\r\n2\r\n3\r\n4\r\n");
    assert_eq!(p.grid.latest_command_output(), None);
}
#[test]
fn running_output_boundary_is_remapped_before_completion() {
    let mut p = Performer::new(8, 5);
    feed(&mut p, b"$ cmd\r\n\x1b]133;C\x07abcdefghij");
    p.resize(20, 5);
    feed(&mut p, b"\r\n\x1b]133;D;0\x07");
    assert_eq!(p.grid.latest_command_output(), Some(((1, 0), (2, 0))));
}

#[test]
fn exclusive_output_end_survives_repeated_bottom_edge_reflow() {
    let mut p = Performer::new(3, 1);
    feed(&mut p, b"\x1b]133;C\x07abc\x1b]133;D;0\x07");
    assert_eq!(p.grid.latest_command_output(), Some(((0, 0), (0, 3))));
    p.resize(1, 1);
    assert_eq!(p.grid.latest_command_output(), Some(((0, 0), (3, 0))));
    p.resize(2, 1);
    assert_eq!(p.grid.latest_command_output(), Some(((0, 0), (2, 0))));
}

#[test]
fn output_start_after_pending_wrap_excludes_the_command_cell() {
    let mut p = Performer::new(3, 3);
    feed(&mut p, b"cmd\x1b]133;C\x07OUT\x1b]133;D;0\x07");
    assert_eq!(p.grid.latest_command_output(), Some(((0, 3), (1, 3))));
    p.resize(10, 3);
    assert_eq!(p.grid.latest_command_output(), Some(((0, 3), (0, 6))));
}
