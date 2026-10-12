#![cfg(feature = "rust-ui")]
use volt_ui_kit::{
    declarative::{Bindings, EditKey, Runtime, TextMetrics},
    ui::prelude::*,
    Viewport,
};
#[derive(Default)]
struct App {
    value: String,
    submitted: usize,
}
struct Metrics;
impl TextMetrics for Metrics {
    fn measure(&mut self, t: &str, size: f32, w: f32) -> [f32; 2] {
        [(t.chars().count() as f32 * size * 0.5).min(w), size * 1.2]
    }
}
fn mount(
    app: &App,
    password: bool,
    read_only: bool,
) -> (Runtime, volt_ui_kit::declarative::Handlers<'static, App>) {
    let e: Element<App> = text_field(&app.value)
        .key("entry")
        .password(password)
        .read_only(read_only)
        .on_change(|a: &mut App, v| a.value = v)
        .on_submit(|a: &mut App| a.submitted += 1)
        .into();
    let (doc, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r.focus_next(false);
    (r, h)
}
#[test]
fn grapheme_deletion_and_undo_are_atomic() {
    let mut a = App {
        value: "A👨‍👩‍👧‍👦e\u{301}".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_key(EditKey::Backspace, false, &mut h, &mut a)
        .unwrap();
    assert_eq!(a.value, "A👨‍👩‍👧‍👦");
    r.editor_key(EditKey::Backspace, false, &mut h, &mut a)
        .unwrap();
    assert_eq!(a.value, "A");
    r.editor_key(EditKey::Undo, false, &mut h, &mut a).unwrap();
    assert_eq!(a.value, "A👨‍👩‍👧‍👦");
}
#[test]
fn preedit_is_visual_only_commit_and_cancel_are_controlled() {
    let mut a = App::default();
    let (mut r, mut h) = mount(&a, false, false);
    r.ime_preedit("に", Some((0, 3)));
    assert_eq!(a.value, "");
    r.ime_cancel();
    assert_eq!(a.value, "");
    r.ime_preedit("日本", None);
    r.ime_commit("日本", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "日本");
    r.editor_key(EditKey::Enter, false, &mut h, &mut a).unwrap();
    assert_eq!(a.submitted, 1);
}
#[test]
fn password_and_read_only_protect_mutation_and_clipboard() {
    let mut a = App {
        value: "secret".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, true, false);
    r.editor_select_all();
    assert!(r.editor_copy().is_none());
    assert!(r.editor_cut(&mut h, &mut a).unwrap().0.is_none());
    assert_eq!(a.value, "secret");
    let (mut r, mut h) = mount(&a, false, true);
    r.text_input("oops", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "secret");
}
fn replace_field(
    runtime: &mut Runtime,
    handlers: &mut volt_ui_kit::declarative::Handlers<'static, App>,
    app: &App,
) {
    let e: Element<App> = text_field(&app.value)
        .key("entry")
        .on_change(|a: &mut App, v| a.value = v)
        .into();
    let (doc, next) = e.build().unwrap().into_parts();
    runtime
        .replace_document(doc, &next, &Bindings::default())
        .unwrap();
    *handlers = next;
    runtime
        .layout(
            Viewport::new(400., 100., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
}
#[test]
fn keyed_recomposition_preserves_cursor_history_and_external_updates_reset_it() {
    let mut a = App {
        value: "hello".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_key(EditKey::Left, false, &mut h, &mut a).unwrap();
    r.text_input("X", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "hellXo");
    replace_field(&mut r, &mut h, &a);
    assert_eq!(r.editor_selection("entry"), Some((5, 5)));
    r.editor_key(EditKey::Undo, false, &mut h, &mut a).unwrap();
    assert_eq!(a.value, "hello");
    replace_field(&mut r, &mut h, &a);
    r.editor_key(EditKey::Redo, false, &mut h, &mut a).unwrap();
    assert_eq!(a.value, "hellXo");
    a.value = "🌍".into();
    replace_field(&mut r, &mut h, &a);
    r.editor_key(EditKey::Undo, false, &mut h, &mut a).unwrap();
    assert_eq!(a.value, "🌍");
    assert_eq!(r.editor_selection("entry"), Some((4, 4)));
}
#[test]
fn selection_replacement_clipboard_and_word_navigation_use_grapheme_boundaries() {
    let mut a = App {
        value: "hello e\u{301} world".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_key(EditKey::WordLeft, false, &mut h, &mut a)
        .unwrap();
    assert_eq!(r.editor_selection("entry"), Some((10, 10)));
    r.editor_key(EditKey::End, true, &mut h, &mut a).unwrap();
    assert_eq!(r.editor_copy().as_deref(), Some("world"));
    r.text_input("🌍", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "hello e\u{301} 🌍");
    r.editor_key(EditKey::Left, true, &mut h, &mut a).unwrap();
    assert_eq!(r.editor_copy().as_deref(), Some("🌍"));
    let (text, outcome) = r.editor_cut(&mut h, &mut a).unwrap();
    assert_eq!(text.as_deref(), Some("🌍"));
    assert!(outcome.activated);
    assert_eq!(a.value, "hello e\u{301} ");
}
#[test]
fn limits_and_undo_budget_reject_oversized_paste_without_losing_selection() {
    let mut a = App {
        value: "safe".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_select_all();
    assert!(
        !r.text_input(&"a".repeat(8193), &mut h, &mut a)
            .unwrap()
            .changed
    );
    assert_eq!(a.value, "safe");
    assert_eq!(r.editor_copy().as_deref(), Some("safe"));
    r.text_input("", &mut h, &mut a).unwrap();
    for _ in 0..100 {
        r.text_input("a", &mut h, &mut a).unwrap();
    }
    for _ in 0..100 {
        r.editor_key(EditKey::Undo, false, &mut h, &mut a).unwrap();
    }
    assert_eq!(a.value.len(), 36);
}
#[test]
fn read_only_validation_and_disabled_focus_are_distinct() {
    let e: Element<App> = text_field("readonly").read_only(true).into();
    assert!(e.build().is_ok());
    let e: Element<App> = text_field("missing").into();
    assert!(e.build().is_err());
    let e: Element<App> = text_field("disabled")
        .enabled(false)
        .on_change(|a: &mut App, v| a.value = v)
        .into();
    let (doc, mut h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert!(!r.focus_next(false));
    let mut a = App::default();
    assert!(!r.text_input("oops", &mut h, &mut a).unwrap().activated);
    assert_eq!(a.value, "");
}
#[test]
fn multiline_explicit_lines_preserve_horizontal_column_and_submit_is_single_line_only() {
    let mut a = App {
        value: "abc\n12345\nxy".into(),
        ..Default::default()
    };
    let e: Element<App> = text_area(&a.value)
        .key("entry")
        .height(80.)
        .on_change(|a: &mut App, v| a.value = v)
        .on_submit(|a: &mut App| a.submitted += 1)
        .into();
    let (doc, mut h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r.focus_next(false);
    r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
        .unwrap();
    r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
    r.editor_key(EditKey::Down, false, &mut h, &mut a).unwrap();
    assert_eq!(r.editor_selection("entry"), Some((5, 5)));
    r.editor_key(EditKey::Enter, false, &mut h, &mut a).unwrap();
    assert_eq!(a.value, "abc\n1\n2345\nxy");
    assert_eq!(a.submitted, 0);
    r.editor_key(EditKey::Submit, false, &mut h, &mut a)
        .unwrap();
    assert_eq!(a.submitted, 1);
    assert_eq!(a.value, "abc\n1\n2345\nxy");
}
#[test]
fn pointer_shift_selection_and_drag_use_measured_advances_at_both_scales() {
    for scale in [1., 2.] {
        let mut a = App {
            value: "abcd".into(),
            ..Default::default()
        };
        let (mut r, mut h) = mount(&a, false, false);
        r.layout(
            Viewport::new(400. * scale, 100. * scale, scale).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        assert!(r.editor_pointer_down(17. * scale, 18. * scale, false));
        assert_eq!(r.editor_selection("entry"), Some((1, 1)));
        r.pointer_up_outcome(17. * scale, 18. * scale, &mut h, &mut a)
            .unwrap();
        assert!(r.editor_pointer_down(31. * scale, 18. * scale, true));
        assert_eq!(r.editor_copy().as_deref(), Some("bc"));
        r.pointer_move_outcome(38. * scale, 18. * scale, &mut h, &mut a)
            .unwrap();
        assert_eq!(r.editor_copy().as_deref(), Some("bcd"));
        r.pointer_up_outcome(38. * scale, 18. * scale, &mut h, &mut a)
            .unwrap();
        assert!(!r.editor_pointer_drag(17. * scale, 18. * scale));
    }
}
#[test]
fn composition_candidate_follows_preedit_cursor_and_cancellation_restores_selection() {
    let mut a = App {
        value: "ab".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_select_all();
    assert!(r.ime_preedit("日本", Some((3, 3))));
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    assert_eq!(r.editor_candidate_bounds().unwrap()[0], 17.);
    assert_eq!(a.value, "ab");
    r.ime_cancel();
    assert_eq!(r.editor_selection("entry"), Some((0, 2)));
    r.ime_preedit("日本", Some((1, 9)));
    r.ime_commit("日", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "日");
}
#[derive(Default)]
struct Recorder {
    scale: f32,
}
impl TextMetrics for Recorder {
    fn measure(&mut self, t: &str, size: f32, width: f32) -> [f32; 2] {
        Metrics.measure(t, size, width)
    }
}
impl volt_ui_kit::Painter for Recorder {
    type Shape = ([f32; 4], [f32; 4]);
    type Glyph = (String, [f32; 4]);
    fn viewport(&self) -> Viewport {
        Viewport::new(400. * self.scale, 100. * self.scale, self.scale).unwrap()
    }
    fn draw_rect(&self, o: &mut Vec<Self::Shape>, x: f32, y: f32, w: f32, h: f32, c: [f32; 4]) {
        o.push(([x, y, w, h], c));
    }
    fn card_triangle(&self, _: &mut Vec<Self::Shape>, _: [[f32; 2]; 3], _: [f32; 4]) {}
    fn card_text(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: [f32; 4],
    ) {
    }
    fn draw_text_colored(
        &mut self,
        _: &mut Vec<Self::Glyph>,
        _: &str,
        _: f32,
        _: f32,
        _: f32,
        _: f32,
        _: &dyn Fn(usize) -> [f32; 4],
    ) {
    }
}
impl volt_ui_kit::declarative::DocumentPainter for Recorder {
    fn ui_text(&mut self, o: &mut Vec<Self::Glyph>, t: &str, b: [f32; 4], _: f32, _: [f32; 4]) {
        o.push((t.into(), b));
    }
}
#[test]
fn password_glyphs_and_diagnostics_never_return_raw_text_and_glyph_clip_is_inside_field() {
    use volt_ui_kit::Painter;
    for scale in [1., 2.] {
        let a = App {
            value: "s🔐e\u{301}cret".into(),
            ..Default::default()
        };
        let (mut r, _) = mount(&a, true, false);
        let mut p = Recorder { scale };
        r.layout(p.viewport(), &Bindings::default(), &mut p)
            .unwrap();
        assert!(r.editor_text("entry").is_none());
        let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
        let glyphs: Vec<_> = batches.iter().flat_map(|b| &b.glyphs).collect();
        assert!(glyphs.iter().all(|(s, _)| s.chars().all(|c| c == '•')));
        assert!(glyphs.iter().any(|(s, _)| s == "•••••••"));
        let batch = batches.iter().find(|b| !b.glyphs.is_empty()).unwrap();
        let [x, y, w, h] = batch.clip.bounds();
        assert_eq!(x, 10. * scale);
        assert!(y >= 7. * scale);
        assert_eq!(w, 380. * scale);
        assert!(h > 0. && h <= 22. * scale);
    }
}
#[test]
fn same_value_replacement_collapses_selection_without_an_application_change() {
    let mut a = App {
        value: "same".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_select_all();
    let outcome = r.text_input("same", &mut h, &mut a).unwrap();
    assert!(outcome.changed);
    assert!(!outcome.activated);
    assert_eq!(r.editor_selection("entry"), Some((4, 4)));
    r.ime_preedit("tentative", Some((9, 9)));
    let outcome = r.ime_commit("", &mut h, &mut a).unwrap();
    assert!(outcome.changed);
    assert!(!outcome.activated);
}
#[test]
fn inserting_a_joiner_keeps_the_cursor_on_the_new_grapheme_boundary() {
    let mut a = App {
        value: "👩👩".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
        .unwrap();
    r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
    r.text_input("\u{200d}", &mut h, &mut a).unwrap();
    assert_eq!(r.editor_selection("entry"), Some((11, 11)));
    r.editor_key(EditKey::Backspace, false, &mut h, &mut a)
        .unwrap();
    assert_eq!(a.value, "");
}
#[test]
fn multiline_wheel_scroll_persists_without_caret_snap_until_navigation() {
    for scale in [1., 2.] {
        let mut a = App {
            value: (0..10)
                .map(|i| format!("line {i}"))
                .collect::<Vec<_>>()
                .join("\n"),
            ..Default::default()
        };
        let e: Element<App> = text_area(&a.value)
            .key("entry")
            .height(60.)
            .on_change(|a: &mut App, v| a.value = v)
            .into();
        let (doc, mut h) = e.build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        let v = Viewport::new(400. * scale, 100. * scale, scale).unwrap();
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        r.focus_next(false);
        let initial = r.editor_scroll_offset("entry").unwrap()[1];
        assert!(initial > 0.);
        assert!(r.scroll_at(20. * scale, 20. * scale, [0., -20.]));
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        let after = r.editor_scroll_offset("entry").unwrap()[1];
        assert!((initial - after - 20.).abs() < 0.1);
        r.invalidate_layout();
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        assert_eq!(r.editor_scroll_offset("entry").unwrap()[1], after);
        r.editor_key(EditKey::DocumentEnd, false, &mut h, &mut a)
            .unwrap();
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        assert_eq!(r.editor_scroll_offset("entry").unwrap()[1], initial);
    }
}
#[test]
fn keyed_recomposition_preserves_active_ime_and_a_removed_editor_rejects_late_commit() {
    let mut a = App {
        value: "ab".into(),
        ..Default::default()
    };
    let (mut r, mut h) = mount(&a, false, false);
    r.ime_preedit("日本", Some((6, 6)));
    replace_field(&mut r, &mut h, &a);
    assert!(r.ime_active());
    r.ime_commit("日本", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "ab日本");
    r.ime_preedit("old", Some((3, 3)));
    let e: Element<App> = text_field("")
        .key("new-editor")
        .on_change(|a: &mut App, v| a.value = v)
        .into();
    let (doc, next) = e.build().unwrap().into_parts();
    r.replace_document(doc, &next, &Bindings::default())
        .unwrap();
    h = next;
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut Metrics,
    )
    .unwrap();
    r.focus_next(false);
    let outcome = r.ime_commit("stale", &mut h, &mut a).unwrap();
    assert!(!outcome.activated);
    assert_eq!(a.value, "ab日本");
    r.ime_preedit("new", Some((3, 3)));
    r.ime_commit("new", &mut h, &mut a).unwrap();
    assert_eq!(a.value, "new");
}
#[test]
fn password_diagnostics_remain_redacted_between_recomposition_and_layout() {
    let e: Element<App> = text_field("secret")
        .key("entry")
        .password(true)
        .on_change(|a: &mut App, v| a.value = v)
        .into();
    let (doc, h) = e.build().unwrap().into_parts();
    let r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
    assert!(r.editor_text("entry").is_none());
}
#[test]
fn document_and_layout_debug_redact_passwords() {
    let a = App {
        value: "secret-only-value".into(),
        ..Default::default()
    };
    let (r, _) = mount(&a, true, false);
    assert!(!format!("{:?}", r.document()).contains(&a.value));
    assert!(!format!("{:?}", r.boxes()).contains(&a.value));
}
#[test]
fn opt_in_soft_wrap_keeps_plain_text_and_navigation_geometry() {
    for scale in [1., 2.] {
        let mut a = App {
            value: "abcdefghij".into(),
            ..Default::default()
        };
        let e: Element<App> = text_area(&a.value)
            .key("wrap")
            .width(55.)
            .height(80.)
            .soft_wrap(true)
            .on_change(|a: &mut App, v| a.value = v)
            .into();
        let (d, mut h) = e.build().unwrap().into_parts();
        let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
        let v = Viewport::new(400. * scale, 100. * scale, scale).unwrap();
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        r.focus_next(false);
        r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
            .unwrap();
        r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
        r.editor_key(EditKey::Down, false, &mut h, &mut a).unwrap();
        assert_eq!(r.editor_selection("wrap"), Some((6, 6)));
        assert_eq!(a.value, "abcdefghij");
        r.editor_key(EditKey::Enter, false, &mut h, &mut a).unwrap();
        assert_eq!(a.value, "abcdef\nghij");
    }
}

fn wrapped(
    app: &App,
    scale: f32,
    password: bool,
) -> (
    Runtime,
    volt_ui_kit::declarative::Handlers<'static, App>,
    Viewport,
) {
    let e: Element<App> = text_area(&app.value)
        .key("wrap")
        .width(55.)
        .height(95.)
        .soft_wrap(true)
        .password(password)
        .on_change(|a: &mut App, v| a.value = v)
        .into();
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let v = Viewport::new(400. * scale, 100. * scale, scale).unwrap();
    r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
    r.focus_next(false);
    (r, h, v)
}
#[test]
fn wrapped_line_end_and_pointer_start_have_distinct_boundary_affinity() {
    for scale in [1., 2.] {
        let mut a = App {
            value: "abcdefghij".into(),
            ..Default::default()
        };
        let (mut r, mut h, v) = wrapped(&a, scale, false);
        r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
            .unwrap();
        r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
        r.editor_key(EditKey::End, false, &mut h, &mut a).unwrap();
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        assert_eq!(r.editor_selection("wrap"), Some((5, 5)));
        let at = r.editor_candidate_bounds().unwrap();
        assert!((at[0] / scale - 45.).abs() < 0.1);
        assert!((at[1] / scale - 7.).abs() < 0.1);
        assert!(r.editor_pointer_down(10. * scale, 25. * scale, false));
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        assert_eq!(r.editor_selection("wrap"), Some((5, 5)));
        let at = r.editor_candidate_bounds().unwrap();
        assert!((at[0] / scale - 10.).abs() < 0.1);
        assert!((at[1] / scale - 23.8).abs() < 0.1);
        r.editor_key(EditKey::End, false, &mut h, &mut a).unwrap();
        assert_eq!(r.editor_selection("wrap"), Some((10, 10)));
        r.editor_key(EditKey::Home, false, &mut h, &mut a).unwrap();
        assert_eq!(r.editor_selection("wrap"), Some((5, 5)));
    }
}
#[test]
fn wrapped_ime_candidate_and_underlines_use_the_same_rows_as_paint_then_undo() {
    for scale in [1., 2.] {
        let mut a = App {
            value: "abcdefghij".into(),
            ..Default::default()
        };
        let (mut r, mut h, v) = wrapped(&a, scale, false);
        r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
            .unwrap();
        r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
        assert!(r.ime_preedit("ABCDEF", Some((6, 6))));
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        let at = r.editor_candidate_bounds().unwrap();
        assert!((at[0] / scale - 24.).abs() < 0.1);
        assert!((at[1] / scale - 23.8).abs() < 0.1);
        assert_eq!(a.value, "abcdefghij");
        let mut p = Recorder { scale };
        let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
        let glyphs: Vec<_> = batches
            .iter()
            .flat_map(|b| &b.glyphs)
            .map(|g| g.0.as_str())
            .collect();
        assert_eq!(glyphs, vec!["aABCD", "EFbcd", "efghi", "j"]);
        let underlines = batches
            .iter()
            .flat_map(|b| &b.shapes)
            .filter(|(rect, _)| (rect[3] - scale).abs() < 0.1 && rect[2] > scale)
            .count();
        assert!(underlines >= 2);
        r.ime_commit("ABCDEF", &mut h, &mut a).unwrap();
        assert_eq!(a.value, "aABCDEFbcdefghij");
        r.editor_key(EditKey::Undo, false, &mut h, &mut a).unwrap();
        assert_eq!(a.value, "abcdefghij");
    }
}
#[test]
fn wrapped_selection_paints_per_visual_row_and_password_stays_masked() {
    for scale in [1., 2.] {
        let mut a = App {
            value: "abcdefghij".into(),
            ..Default::default()
        };
        let (mut r, mut h, v) = wrapped(&a, scale, false);
        r.editor_key(EditKey::DocumentStart, false, &mut h, &mut a)
            .unwrap();
        r.editor_key(EditKey::Right, false, &mut h, &mut a).unwrap();
        for _ in 0..7 {
            r.editor_key(EditKey::Right, true, &mut h, &mut a).unwrap();
        }
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        assert_eq!(r.editor_copy().as_deref(), Some("bcdefgh"));
        let mut p = Recorder { scale };
        let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
        let selected: Vec<_> = batches
            .iter()
            .flat_map(|b| &b.shapes)
            .filter(|(rect, color)| {
                (rect[3] / scale - 16.8).abs() < 0.1 && (color[3] - 0.3).abs() < 0.01
            })
            .collect();
        assert_eq!(selected.len(), 2);
        let (mut r, _, v) = wrapped(&a, scale, true);
        r.ime_preedit("secret", Some((6, 6)));
        r.layout(v, &Bindings::default(), &mut Metrics).unwrap();
        let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
        assert!(batches
            .iter()
            .flat_map(|b| &b.glyphs)
            .all(|(t, _)| t.chars().all(|c| c == '•')));
        assert!(r.editor_copy().is_none());
        assert!(r.editor_text("wrap").is_none());
    }
}
#[test]
fn single_line_soft_wrap_is_rejected_and_explicit_lines_stay_default() {
    let e: Element<App> = text_field("value").soft_wrap(true).into();
    assert!(e.build().is_err());
    let e: Element<App> = text_area("abcdefghij").read_only(true).width(55.).into();
    let (d, h) = e.build().unwrap().into_parts();
    let mut r = Runtime::new(d, &h, &Bindings::default()).unwrap();
    let mut p = Recorder { scale: 1. };
    r.layout(
        Viewport::new(400., 100., 1.).unwrap(),
        &Bindings::default(),
        &mut p,
    )
    .unwrap();
    let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
    assert_eq!(
        batches.iter().flat_map(|b| &b.glyphs).next().unwrap().0,
        "abcdefghij"
    );
}

#[cfg(feature = "native")]
#[test]
fn native_shaped_soft_wrap_round_trips_grapheme_caret_and_raster_clipping() {
    use unicode_segmentation::UnicodeSegmentation;
    use volt_ui_kit::native::RasterPainter;
    for scale in [1., 2.] {
        let mut app = App {
            value: "office e\u{301} 👨‍👩‍👧‍👦 native shaping wraps without changing stored text.".into(),
            ..Default::default()
        };
        let original = app.value.clone();
        let ui: Element<App> = text_area(&app.value)
            .key("native-wrap")
            .width(120.)
            .height(90.)
            .font_family("SF")
            .font_size(16.)
            .soft_wrap(true)
            .background("#101418")
            .color("#FFFFFF")
            .on_change(|a: &mut App, v| a.value = v)
            .into();
        let (doc, mut h) = ui.build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        let viewport = Viewport::new(240. * scale, 120. * scale, scale).unwrap();
        let mut p = RasterPainter::new(viewport);
        r.layout(viewport, &Bindings::default(), &mut p).unwrap();
        r.focus_next(false);
        r.editor_key(EditKey::DocumentStart, false, &mut h, &mut app)
            .unwrap();
        r.layout(viewport, &Bindings::default(), &mut p).unwrap();
        for _ in 0..original.graphemes(true).count() {
            r.editor_key(EditKey::Right, false, &mut h, &mut app)
                .unwrap();
            r.layout(viewport, &Bindings::default(), &mut p).unwrap();
            let expected = r.editor_selection("native-wrap").unwrap().1;
            let [x, y, _, line] = r.editor_candidate_bounds().unwrap();
            assert!(x >= 10. * scale - 0.1 && x <= 110. * scale + 0.1);
            assert!(y >= 7. * scale - 0.1 && y + line <= 83. * scale + 0.1);
            assert!(r.editor_pointer_down(x, y + line * 0.5, false));
            r.pointer_up_outcome(x, y, &mut h, &mut app).unwrap();
            assert_eq!(
                r.editor_selection("native-wrap"),
                Some((expected, expected))
            );
            r.layout(viewport, &Bindings::default(), &mut p).unwrap();
        }
        assert_eq!(app.value, original);
        assert_eq!(r.editor_scroll_offset("native-wrap").unwrap()[0], 0.);
        r.editor_select_all();
        r.layout(viewport, &Bindings::default(), &mut p).unwrap();
        let batches = r.paint_ui(&mut p, &volt_ui_kit::UiTheme::dark());
        let pixels = p.rasterize(&batches, [0., 0., 0., 1.]).unwrap();
        for [x, y] in [[130. * scale, 30. * scale], [30. * scale, 100. * scale]] {
            let pixel = pixels.pixel(x as u32, y as u32).unwrap();
            assert_eq!([pixel.red(), pixel.green(), pixel.blue()], [0, 0, 0]);
        }
    }
}
#[test]
fn chat_composer_submits_on_enter_and_breaks_lines_on_shift_enter() {
    for enter_submits in [false, true] {
        let mut a = App::default();
        let e: Element<App> = text_area(&a.value)
            .key("composer")
            .submit_on_enter(enter_submits)
            .on_change(|a: &mut App, v| a.value = v)
            .on_submit(|a: &mut App| a.submitted += 1)
            .into();
        let (doc, mut h) = e.build().unwrap().into_parts();
        let mut r = Runtime::new(doc, &h, &Bindings::default()).unwrap();
        r.layout(
            Viewport::new(400., 200., 1.).unwrap(),
            &Bindings::default(),
            &mut Metrics,
        )
        .unwrap();
        r.focus_next(false);
        r.editor_key(EditKey::Enter, true, &mut h, &mut a).unwrap();
        assert_eq!(a.value, "\n", "Shift+Enter always inserts a newline");
        r.editor_key(EditKey::Enter, false, &mut h, &mut a).unwrap();
        assert_eq!(a.submitted, usize::from(enter_submits));
        assert_eq!(a.value.len(), if enter_submits { 1 } else { 2 });
        r.editor_key(EditKey::Submit, false, &mut h, &mut a)
            .unwrap();
        assert_eq!(a.submitted, usize::from(enter_submits) + 1);
    }
}
