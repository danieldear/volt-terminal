//! Bounded plain-text editing, independent of a window/clipboard implementation.
use super::*;
use unicode_segmentation::UnicodeSegmentation;
pub(super) const MAX_TEXT: usize = 8192;
const MAX_UNDO: usize = 64;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKey {
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    DocumentStart,
    DocumentEnd,
    WordLeft,
    WordRight,
    Backspace,
    Delete,
    Enter,
    Submit,
    Undo,
    Redo,
}
#[derive(Clone)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
    upstream: bool,
}
pub(super) struct Composition {
    id: String,
    text: String,
    cursor: Option<(usize, usize)>,
}
#[derive(Clone, Default)]
pub(super) struct EditorState {
    text: String,
    caret: usize,
    anchor: usize,
    preedit: String,
    preedit_cursor: Option<(usize, usize)>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    positions: Vec<(usize, [f32; 2])>,
    rows: Vec<VisualRow>,
    display_positions: Vec<(usize, [f32; 2])>,
    paint_caret: [f32; 2],
    composition_range: Option<std::ops::Range<usize>>,
    upstream: bool,
    offset: [f32; 2],
    follow_caret: bool,
    layout_scale: f32,
    drag: bool,
}
impl EditorState {
    fn new(text: String) -> Self {
        let caret = text.len();
        Self {
            text,
            caret,
            anchor: caret,
            follow_caret: true,
            ..Default::default()
        }
    }
    fn range(&self) -> std::ops::Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }
    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
            upstream: self.upstream,
        }
    }
    fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.caret = s.caret;
        self.anchor = s.anchor;
        self.preedit.clear();
        self.upstream = s.upstream;
        self.follow_caret = true;
    }
    fn replace(&mut self, text: &str) -> bool {
        let range = self.range();
        if self.text.len() - range.len() + text.len() > MAX_TEXT {
            return false;
        }
        if range.is_empty() && text.is_empty() {
            return false;
        }
        if self.text.get(range.clone()) == Some(text) {
            self.caret = range.start + text.len();
            self.anchor = self.caret;
            self.follow_caret = true;
            return false;
        }
        if self.undo.len() == MAX_UNDO {
            self.undo.remove(0);
        }
        self.undo.push(self.snapshot());
        self.redo.clear();
        self.text.replace_range(range.clone(), text);
        self.caret = boundaries(&self.text)
            .into_iter()
            .find(|i| *i >= range.start + text.len())
            .unwrap_or(self.text.len());
        self.anchor = self.caret;
        self.preedit.clear();
        self.upstream = false;
        self.follow_caret = true;
        true
    }
    fn move_to(&mut self, to: usize, shift: bool) {
        self.caret = boundary(&self.text, to);
        self.upstream = false;
        if !shift {
            self.anchor = self.caret;
        }
        self.preedit.clear();
        self.follow_caret = true;
    }
}
fn boundaries(text: &str) -> Vec<usize> {
    text.grapheme_indices(true)
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect()
}
fn boundary(text: &str, at: usize) -> usize {
    boundaries(text)
        .into_iter()
        .take_while(|i| *i <= at)
        .last()
        .unwrap_or(0)
}
fn word_left(text: &str, at: usize) -> usize {
    text[..at]
        .split_word_bound_indices()
        .rev()
        .find(|(_, word)| word.chars().any(char::is_alphanumeric))
        .map_or(0, |(i, _)| i)
}
fn word_right(text: &str, at: usize) -> usize {
    text[at..]
        .split_word_bound_indices()
        .find(|(_, word)| word.chars().any(char::is_alphanumeric))
        .map_or(text.len(), |(i, word)| at + i + word.len())
}
fn previous(text: &str, at: usize) -> usize {
    boundaries(text)
        .into_iter()
        .take_while(|i| *i < at)
        .last()
        .unwrap_or(0)
}
fn next(text: &str, at: usize) -> usize {
    boundaries(text)
        .into_iter()
        .find(|i| *i > at)
        .unwrap_or(text.len())
}
pub(super) fn sanitize(text: &str, multiline: bool) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .map(|c| if c == '\n' && !multiline { ' ' } else { c })
        .filter(|c| (*c == '\n' && multiline) || *c == '\t' || !c.is_control())
        .collect::<String>()
        .replace('\t', "    ")
}
#[derive(Clone)]
struct VisualRow {
    start: usize,
    end: usize,
    text: String,
    x: f32,
}
fn editor_line_height(style: &TextStyle) -> f32 {
    style.size
        * if style.line_height > 0. {
            style.line_height
        } else {
            1.2
        }
}
fn editor_row_style(style: &TextStyle) -> TextStyle {
    let mut row = style.clone();
    // Rows are wrapped/aligned by the editor so ink, selection and caret
    // share coordinates. Never center an unbounded shaping buffer.
    row.align = super::super::extras::TextAlign::Start;
    row.max_lines = 0;
    row.no_wrap = true;
    row
}
fn caret_position(positions: &[(usize, [f32; 2])], at: usize, upstream: bool) -> [f32; 2] {
    if upstream {
        positions.iter().find(|(i, _)| *i == at)
    } else {
        positions.iter().rev().find(|(i, _)| *i == at)
    }
    .map_or([0.; 2], |(_, p)| *p)
}
/// Wrap at whitespace when possible, with grapheme fallback for a long token.
/// Each segment is shaped again independently, exactly as it is drawn.
fn visual_rows(
    text: &str,
    options: &super::super::document::EditorOptions,
    style: &TextStyle,
    width: f32,
    metrics: &mut impl TextMetrics,
) -> (Vec<VisualRow>, Vec<(usize, [f32; 2])>) {
    let mut rows = Vec::new();
    let mut positions = Vec::new();
    let mut base = 0;
    let line_height = editor_line_height(style);
    let row_style = editor_row_style(style);
    for hard_line in text.split('\n') {
        let mut start = 0;
        loop {
            let remainder = &hard_line[start..];
            let points = boundaries(remainder);
            let display = if options.password {
                "•".repeat(remainder.graphemes(true).count())
            } else {
                remainder.to_string()
            };
            let display_points = boundaries(&display);
            let advances = metrics.line_advances(&display, &row_style, &display_points);
            let mut end = remainder.len();
            if options.soft_wrap && advances.last().copied().unwrap_or(0.) > width {
                let fitting = advances
                    .iter()
                    .enumerate()
                    .take_while(|(_, x)| **x <= width)
                    .last()
                    .map_or(1, |(i, _)| i.max(1));
                end = points[fitting.min(points.len() - 1)];
                if !options.password {
                    let whitespace = remainder[..end]
                        .grapheme_indices(true)
                        .filter(|(_, g)| g.chars().all(char::is_whitespace))
                        .map(|(i, g)| i + g.len())
                        .next_back();
                    if let Some(space) = whitespace {
                        end = space;
                    }
                }
            }
            let mut segment = &remainder[..end];
            let mut display = if options.password {
                "•".repeat(segment.graphemes(true).count())
            } else {
                segment.to_string()
            };
            let mut points = boundaries(segment);
            let mut advances = metrics.line_advances(&display, &row_style, &boundaries(&display));
            // Shaping a segment can differ from the original line's contextual width.
            while options.soft_wrap
                && advances.last().copied().unwrap_or(0.) > width
                && points.len() > 2
            {
                end = points[points.len() - 2];
                segment = &remainder[..end];
                display = if options.password {
                    "•".repeat(segment.graphemes(true).count())
                } else {
                    segment.to_string()
                };
                points = boundaries(segment);
                advances = metrics.line_advances(&display, &row_style, &boundaries(&display));
            }
            let y = rows.len() as f32 * line_height;
            let free = (width - advances.iter().copied().fold(0_f32, f32::max)).max(0.);
            let x = match style.align {
                super::super::extras::TextAlign::Center => free * 0.5,
                super::super::extras::TextAlign::End => free,
                _ => 0., // Editable rows do not expand inter-word spacing.
            };
            for (i, at) in points.iter().enumerate() {
                positions.push((
                    base + start + at,
                    [x + advances.get(i).copied().unwrap_or(0.), y],
                ));
            }
            rows.push(VisualRow {
                start: base + start,
                end: base + start + end,
                text: display,
                x,
            });
            start += end;
            if start >= hard_line.len() {
                break;
            }
        }
        base += hard_line.len() + 1;
    }
    (rows, positions)
}

impl Runtime {
    fn editable_box(&self) -> Option<&LayoutBox> {
        self.focused.as_deref().and_then(|id| {
            self.layout_boxes
                .iter()
                .find(|b| b.id == id && b.enabled && b.editor.is_some() && self.scope_allows_id(id))
        })
    }
    pub fn editing_focus(&self) -> bool {
        self.editable_box().is_some()
    }
    pub fn editing_ime_allowed(&self) -> bool {
        self.editable_box()
            .is_some_and(|b| b.editor.as_ref().is_some_and(|e| !e.read_only))
    }
    pub fn editing_password(&self) -> bool {
        self.editable_box()
            .is_some_and(|b| b.editor.as_ref().is_some_and(|e| e.password))
    }
    pub fn editor_text(&self, id: &str) -> Option<String> {
        fn password(node: &Node, id: &str) -> bool {
            (node.id == id && node.editor.as_ref().is_some_and(|options| options.password))
                || node.children.iter().any(|child| password(child, id))
        }
        if password(self.document.root(), id) {
            return None;
        }
        self.editors.borrow().get(id).map(|e| e.text.clone())
    }
    pub fn editor_selection(&self, id: &str) -> Option<(usize, usize)> {
        self.editors.borrow().get(id).map(|e| (e.anchor, e.caret))
    }
    pub(super) fn editor_composition_snapshot(&self, next: &Document) -> Option<Composition> {
        let id = self.ime_target.as_deref()?;
        if self.focused.as_deref() != Some(id) {
            return None;
        }
        fn find<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
            if n.id == id {
                Some(n)
            } else {
                n.children.iter().find_map(|c| find(c, id))
            }
        }
        let node = find(next.root(), id)?;
        if !node.enabled || node.editor.as_ref().is_none_or(|o| o.read_only) {
            return None;
        }
        let editors = self.editors.borrow();
        let editor = editors.get(id)?;
        if node.text.as_deref() != Some(&editor.text) {
            return None;
        }
        Some(Composition {
            id: id.into(),
            text: editor.preedit.clone(),
            cursor: editor.preedit_cursor,
        })
    }
    pub(super) fn restore_editor_composition(&mut self, composition: Composition) {
        if self.focused.as_deref() != Some(&composition.id)
            || !self.scope_allows_id(&composition.id)
        {
            return;
        }
        if let Some(editor) = self.editors.get_mut().get_mut(&composition.id) {
            editor.preedit = composition.text;
            editor.preedit_cursor = composition.cursor;
            self.ime_target = Some(composition.id);
            self.ime_cancelled = false;
        }
    }
    pub(super) fn reconcile_editors(&mut self) {
        fn visit(n: &Node, map: &mut BTreeMap<String, EditorState>, ids: &mut BTreeSet<String>) {
            if n.editor.is_some() {
                ids.insert(n.id.clone());
                let value = n.text.clone().unwrap_or_default();
                let e = map
                    .entry(n.id.clone())
                    .or_insert_with(|| EditorState::new(value.clone()));
                if e.text != value {
                    e.text = value;
                    e.caret = boundary(&e.text, e.caret);
                    e.anchor = boundary(&e.text, e.anchor);
                    e.upstream = false;
                    e.undo.clear();
                    e.redo.clear();
                    e.preedit.clear();
                    e.follow_caret = true;
                }
            }
            for c in &n.children {
                visit(c, map, ids);
            }
        }
        let mut ids = BTreeSet::new();
        visit(self.document.root(), self.editors.get_mut(), &mut ids);
        self.editors.get_mut().retain(|id, _| ids.contains(id));
    }
    pub(super) fn layout_editors(&mut self, metrics: &mut impl TextMetrics) {
        for b in &self.layout_boxes {
            if let Some(options) = &b.editor {
                if let Some(e) = self.editors.get_mut().get_mut(&b.id) {
                    if e.layout_scale > 0. && e.layout_scale != self.scale {
                        for offset in &mut e.offset {
                            *offset *= self.scale / e.layout_scale;
                        }
                    }
                    e.layout_scale = self.scale;
                    let line_height = editor_line_height(&b.text_style);
                    let [_, _, width, height] = inner_geometry(b, self.scale);
                    let width = width.max(self.scale);
                    let height = height.max(line_height);
                    let (rows, positions) =
                        visual_rows(&e.text, options, &b.text_style, width, metrics);
                    e.positions = positions;
                    if e.preedit.is_empty() {
                        e.rows = rows;
                        e.display_positions = e.positions.clone();
                        e.paint_caret = caret_position(&e.positions, e.caret, e.upstream);
                        e.composition_range = None;
                    } else {
                        let range = e.range();
                        let mut display = e.text.clone();
                        display.replace_range(range.clone(), &e.preedit);
                        let (rows, positions) =
                            visual_rows(&display, options, &b.text_style, width, metrics);
                        let cursor = e.preedit_cursor.map_or(e.preedit.len(), |(_, end)| end);
                        e.paint_caret = caret_position(
                            &positions,
                            range.start + boundary(&e.preedit, cursor),
                            false,
                        );
                        e.rows = rows;
                        e.display_positions = positions;
                        e.composition_range = Some(range.start..range.start + e.preedit.len());
                    }
                    let max_x = e
                        .display_positions
                        .iter()
                        .fold(0_f32, |max, (_, p)| max.max(p[0]));
                    let max_y = e.rows.len() as f32 * line_height;
                    let max_x = if options.soft_wrap {
                        0.
                    } else {
                        (max_x - width + 2. * self.scale).max(0.)
                    };
                    e.offset[0] = e.offset[0].clamp(0., max_x);
                    e.offset[1] = e.offset[1].clamp(0., (max_y - height).max(0.));
                    let pos = e.paint_caret;
                    if e.follow_caret {
                        if !options.soft_wrap {
                            if pos[0] < e.offset[0] {
                                e.offset[0] = pos[0];
                            } else if pos[0] > e.offset[0] + width - 2. * self.scale {
                                e.offset[0] = (pos[0] - width + 2. * self.scale).max(0.);
                            }
                        }
                        if pos[1] < e.offset[1] {
                            e.offset[1] = pos[1];
                        } else if pos[1] + line_height > e.offset[1] + height {
                            e.offset[1] = (pos[1] + line_height - height).max(0.);
                        }
                    }
                }
            }
        }
    }
    fn editor_dispatch<C>(
        &mut self,
        id: &str,
        changed: bool,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if !changed {
            return Ok(InputOutcome::default());
        }
        let text = self.editors.borrow()[id].text.clone();
        let handler = self
            .layout_boxes
            .iter()
            .find(|b| b.id == id)
            .and_then(|b| b.on_text_changed.clone());
        self.invalidate_layout();
        if let Some(name) = handler {
            handlers
                .dispatch(&name, Event::TextChanged(text), ctx)
                .map_err(|e| DocumentError(e.to_string()))?;
            return Ok(InputOutcome {
                changed: true,
                activated: true,
            });
        }
        Ok(InputOutcome {
            changed: true,
            activated: false,
        })
    }
    pub fn text_input<C>(
        &mut self,
        text: &str,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if text.len() > MAX_TEXT {
            return Ok(InputOutcome::default());
        }
        let Some(b) = self.editable_box() else {
            return Ok(InputOutcome::default());
        };
        let options = b.editor.as_ref().unwrap();
        if options.read_only {
            return Ok(InputOutcome::default());
        }
        let id = b.id.clone();
        let multiline = options.multiline;
        let sanitized = sanitize(text, multiline);
        if !text.is_empty() && sanitized.is_empty() {
            return Ok(InputOutcome::default());
        };
        let text = sanitized;
        let interaction_changed = self.reveal_focus(&id) | self.cancel_pointer_captures();
        let Some(editor) = self.editors.get_mut().get_mut(&id) else {
            return Ok(InputOutcome::default());
        };
        let old = (editor.caret, editor.anchor, editor.follow_caret);
        let modified = editor.replace(&text);
        let selection_changed = old != (editor.caret, editor.anchor, editor.follow_caret);
        if modified {
            self.editor_dispatch(&id, true, handlers, ctx)
        } else {
            if selection_changed {
                self.invalidate_layout();
            }
            Ok(InputOutcome {
                changed: selection_changed || interaction_changed,
                activated: false,
            })
        }
    }
    pub fn editor_key<C>(
        &mut self,
        key: EditKey,
        shift: bool,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let Some(b) = self.editable_box() else {
            return Ok(InputOutcome::default());
        };
        let options = b.editor.as_ref().unwrap().clone();
        let id = b.id.clone();
        let submit = b.on_submit.clone();
        let interaction_changed = self.reveal_focus(&id) | self.cancel_pointer_captures();
        let enter_submits = !options.multiline || (options.enter_submits && !shift);
        if key == EditKey::Submit || (key == EditKey::Enter && enter_submits) {
            if let Some(name) = submit {
                handlers
                    .dispatch(&name, Event::Activate, ctx)
                    .map_err(|e| DocumentError(e.to_string()))?;
                return Ok(InputOutcome {
                    changed: interaction_changed,
                    activated: true,
                });
            }
            return Ok(InputOutcome {
                changed: interaction_changed,
                activated: false,
            });
        }
        if key == EditKey::Enter {
            let mut outcome = self.text_input("\n", handlers, ctx)?;
            outcome.changed |= interaction_changed;
            return Ok(outcome);
        }
        let Some(e) = self.editors.get_mut().get_mut(&id) else {
            return Ok(InputOutcome::default());
        };
        let old = (e.caret, e.anchor, e.follow_caret, e.upstream);
        let mut modified = false;
        match key {
            EditKey::Left => {
                let at = if !shift && !e.range().is_empty() {
                    e.range().start
                } else {
                    previous(&e.text, e.caret)
                };
                e.move_to(at, shift);
                e.upstream = true;
            }
            EditKey::Right => {
                let at = if !shift && !e.range().is_empty() {
                    e.range().end
                } else {
                    next(&e.text, e.caret)
                };
                e.move_to(at, shift);
            }
            EditKey::Home | EditKey::End => {
                let position = caret_position(&e.positions, e.caret, e.upstream);
                let row = e
                    .positions
                    .iter()
                    .filter(|(_, p)| (p[1] - position[1]).abs() < 0.5);
                let target = if key == EditKey::Home {
                    row.min_by(|(_, a), (_, b)| a[0].total_cmp(&b[0]))
                } else {
                    row.max_by(|(_, a), (_, b)| a[0].total_cmp(&b[0]))
                };
                if let Some((at, _)) = target {
                    e.move_to(*at, shift);
                    e.upstream = key == EditKey::End;
                }
            }
            EditKey::WordLeft => {
                let to = word_left(&e.text, e.caret);
                e.move_to(to, shift);
            }
            EditKey::WordRight => {
                let to = word_right(&e.text, e.caret);
                e.move_to(to, shift);
            }
            EditKey::DocumentStart => e.move_to(0, shift),
            EditKey::DocumentEnd => e.move_to(e.text.len(), shift),
            EditKey::Up | EditKey::Down => {
                let p = caret_position(&e.positions, e.caret, e.upstream);
                let y = p[1]
                    + if key == EditKey::Up {
                        -b_line_height(&self.layout_boxes, &id)
                    } else {
                        b_line_height(&self.layout_boxes, &id)
                    };
                if let Some((at, q)) = e
                    .positions
                    .iter()
                    .filter(|(_, q)| (q[1] - y).abs() < 0.5)
                    .min_by(|(_, a), (_, b)| (a[0] - p[0]).abs().total_cmp(&(b[0] - p[0]).abs()))
                {
                    let (at, q) = (*at, *q);
                    e.move_to(at, shift);
                    e.upstream = caret_position(&e.positions, at, true) == q
                        && caret_position(&e.positions, at, false) != q;
                }
            }
            EditKey::Backspace | EditKey::Delete if !options.read_only => {
                if e.range().is_empty() {
                    if key == EditKey::Backspace {
                        e.anchor = previous(&e.text, e.caret);
                    } else {
                        e.anchor = next(&e.text, e.caret);
                    }
                }
                modified = e.replace("");
            }
            EditKey::Undo if !options.read_only => {
                if let Some(previous) = e.undo.pop() {
                    if e.redo.len() == MAX_UNDO {
                        e.redo.remove(0);
                    }
                    e.redo.push(e.snapshot());
                    e.restore(previous);
                    modified = true;
                }
            }
            EditKey::Redo if !options.read_only => {
                if let Some(next) = e.redo.pop() {
                    if e.undo.len() == MAX_UNDO {
                        e.undo.remove(0);
                    }
                    e.undo.push(e.snapshot());
                    e.restore(next);
                    modified = true;
                }
            }
            _ => {}
        }
        let changed = interaction_changed || old != (e.caret, e.anchor, e.follow_caret, e.upstream);
        if modified {
            return self.editor_dispatch(&id, true, handlers, ctx);
        }
        if changed {
            self.invalidate_layout();
        }
        Ok(InputOutcome {
            changed,
            activated: false,
        })
    }
    pub fn editor_select_all(&mut self) -> bool {
        let Some(id) = self.editable_box().map(|b| b.id.clone()) else {
            return false;
        };
        let interaction_changed = self.reveal_focus(&id) | self.cancel_pointer_captures();
        let Some(e) = self.editors.get_mut().get_mut(&id) else {
            return interaction_changed;
        };
        let changed =
            interaction_changed || e.anchor != 0 || e.caret != e.text.len() || !e.follow_caret;
        e.anchor = 0;
        e.caret = e.text.len();
        e.follow_caret = true;
        self.invalidate_layout();
        changed
    }
    pub fn editor_copy(&self) -> Option<String> {
        let b = self.editable_box()?;
        if b.editor.as_ref()?.password {
            return None;
        };
        let editors = self.editors.borrow();
        let e = editors.get(&b.id)?;
        (!e.range().is_empty()).then(|| e.text[e.range()].to_string())
    }
    pub fn editor_cut<C>(
        &mut self,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<(Option<String>, InputOutcome), DocumentError> {
        let text = self.editor_copy();
        if text.is_none() {
            return Ok((None, InputOutcome::default()));
        };
        let Some(b) = self.editable_box() else {
            return Ok((None, InputOutcome::default()));
        };
        if b.editor.as_ref().unwrap().read_only {
            return Ok((text, InputOutcome::default()));
        };
        let outcome = self.text_input("", handlers, ctx)?;
        Ok((text, outcome))
    }
    pub fn ime_preedit(&mut self, text: &str, cursor: Option<(usize, usize)>) -> bool {
        let Some(b) = self.editable_box() else {
            return false;
        };
        if b.editor.as_ref().unwrap().read_only || text.len() > MAX_TEXT {
            return false;
        };
        let id = b.id.clone();
        let e = self.editors.get_mut().get_mut(&id).unwrap();
        let changed = e.preedit != text || e.preedit_cursor != cursor;
        e.preedit = text.into();
        e.follow_caret = true;
        if !text.is_empty() {
            self.ime_target = Some(id);
            self.ime_cancelled = false;
        }
        e.preedit_cursor = cursor.filter(|(a, b)| {
            *a <= text.len()
                && *b <= text.len()
                && text.is_char_boundary(*a)
                && text.is_char_boundary(*b)
        });
        if changed {
            self.invalidate_layout();
        }
        changed
    }
    pub fn ime_commit<C>(
        &mut self,
        text: &str,
        handlers: &mut Handlers<'_, C>,
        ctx: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        let target = self.ime_target.take();
        let blocked = std::mem::take(&mut self.ime_cancelled)
            || target
                .as_deref()
                .is_some_and(|id| self.focused.as_deref() != Some(id));
        let cancelled = self.ime_cancel();
        if blocked {
            return Ok(InputOutcome {
                changed: cancelled,
                activated: false,
            });
        }
        let mut outcome = self.text_input(text, handlers, ctx)?;
        outcome.changed |= cancelled;
        Ok(outcome)
    }
    pub fn ime_cancel(&mut self) -> bool {
        if self.ime_target.take().is_some() {
            self.ime_cancelled = true;
        }
        let mut changed = false;
        for e in self.editors.get_mut().values_mut() {
            changed |= !e.preedit.is_empty();
            e.preedit.clear();
            e.preedit_cursor = None;
        }
        if changed {
            self.invalidate_layout();
        }
        changed
    }
    pub fn ime_active(&self) -> bool {
        self.editable_box()
            .and_then(|b| {
                self.editors
                    .borrow()
                    .get(&b.id)
                    .map(|e| !e.preedit.is_empty())
            })
            .unwrap_or(false)
    }
    pub fn editor_candidate_bounds(&self) -> Option<[f32; 4]> {
        let b = self.editable_box()?;
        if !b.layout_visible {
            return None;
        }
        let editors = self.editors.borrow();
        let e = editors.get(&b.id)?;
        let p = e.paint_caret;
        let [x, y, _, _] = self.editor_inner(b);
        Some([
            x + p[0] - e.offset[0],
            y + p[1] - e.offset[1],
            self.scale,
            editor_line_height(&b.text_style),
        ])
    }
    pub fn editor_scroll_offset(&self, id: &str) -> Option<[f32; 2]> {
        self.editors
            .borrow()
            .get(id)
            .map(|e| [e.offset[0] / self.scale, e.offset[1] / self.scale])
    }
    pub(super) fn editor_scroll_at(&mut self, x: f32, y: f32, delta: &mut [f32; 2]) -> bool {
        let Some(id) = self.hit(x, y) else {
            return false;
        };
        let Some(b) = self.boxes.iter().find(|b| b.id == id && b.editor.is_some()) else {
            return false;
        };
        let multiline = b.editor.as_ref().is_some_and(|e| e.multiline);
        let [_, _, width, height] = self.editor_inner(b);
        let line_height = editor_line_height(&b.text_style);
        let Some(e) = self.editors.get_mut().get_mut(&id) else {
            return false;
        };
        let max_x = e
            .display_positions
            .iter()
            .fold(0_f32, |max, (_, p)| max.max(p[0]));
        let max_y = e.rows.len() as f32 * line_height;
        let maximum = [
            if b.editor.as_ref().is_some_and(|o| o.soft_wrap) {
                0.
            } else {
                (max_x - width + 2. * self.scale).max(0.)
            },
            if multiline {
                (max_y - height).max(0.)
            } else {
                0.
            },
        ];
        let mut changed = false;
        for axis in 0..2 {
            let old = e.offset[axis];
            e.offset[axis] = (old + delta[axis] * self.scale).clamp(0., maximum[axis]);
            delta[axis] -= (e.offset[axis] - old) / self.scale;
            changed |= e.offset[axis] != old;
        }
        if changed {
            e.follow_caret = false;
            self.invalidate_layout();
        }
        changed
    }
    pub fn editor_pointer_down(&mut self, x: f32, y: f32, shift: bool) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(id) = self.hit(x, y) else {
            return false;
        };
        let Some(b) = self.boxes.iter().find(|b| b.id == id && b.editor.is_some()) else {
            return false;
        };
        let [bx, by, _, _] = self.editor_inner(b);
        let Some(e) = self.editors.get_mut().get_mut(&id) else {
            return false;
        };
        let pos = [x - bx + e.offset[0], y - by + e.offset[1]];
        let (at, upstream) = nearest(e, pos, editor_line_height(&b.text_style));
        e.move_to(at, shift);
        e.upstream = upstream;
        e.drag = true;
        self.focused = Some(id);
        self.capture = None;
        self.keyboard = None;
        self.invalidate_layout();
        self.sync_tooltip_visibility();
        true
    }
    pub fn editor_pointer_drag(&mut self, x: f32, y: f32) -> bool {
        if !x.is_finite() || !y.is_finite() {
            return false;
        }
        let Some(id) = self.focused.clone() else {
            return false;
        };
        let Some(b) = self.boxes.iter().find(|b| b.id == id && b.editor.is_some()) else {
            return false;
        };
        let [bx, by, _, _] = self.editor_inner(b);
        let Some(e) = self.editors.get_mut().get_mut(&id).filter(|e| e.drag) else {
            return false;
        };
        let (at, upstream) = nearest(
            e,
            [x - bx + e.offset[0], y - by + e.offset[1]],
            editor_line_height(&b.text_style),
        );
        if at == e.caret && upstream == e.upstream {
            return false;
        };
        e.move_to(at, true);
        e.upstream = upstream;
        self.invalidate_layout();
        true
    }
    pub(super) fn editor_pointer_up(&mut self) -> bool {
        let mut changed = false;
        for e in self.editors.get_mut().values_mut() {
            changed |= e.drag;
            e.drag = false;
        }
        changed
    }
}
fn b_line_height(boxes: &[LayoutBox], id: &str) -> f32 {
    boxes
        .iter()
        .find(|b| b.id == id)
        .map_or(16., |b| editor_line_height(&b.text_style))
}
fn nearest(e: &EditorState, p: [f32; 2], height: f32) -> (usize, bool) {
    e.positions
        .iter()
        .min_by(|(_, a), (_, b)| {
            let score = |q: &[f32; 2]| {
                ((q[1] + height * 0.5 - p[1]) / height).abs() * 100000. + (q[0] - p[0]).abs()
            };
            score(a).total_cmp(&score(b))
        })
        .map_or((0, false), |(i, q)| {
            (
                *i,
                caret_position(&e.positions, *i, true) == *q
                    && caret_position(&e.positions, *i, false) != *q,
            )
        })
}
fn inner_geometry(b: &LayoutBox, scale: f32) -> [f32; 4] {
    let [x, y, w, h] = b.bounds.bounds();
    let line = editor_line_height(&b.text_style);
    let top = if b.editor.as_ref().is_some_and(|e| e.multiline) {
        7. * scale
    } else {
        ((h - line) * 0.5).max(7. * scale).min((h * 0.5).max(0.))
    };
    let [lead, trail] = b.editor.as_ref().map_or([0.; 2], |e| e.inset);
    [
        x + (10. + lead) * scale,
        y + top,
        (w - (20. + lead + trail) * scale).max(0.),
        (h - 2. * top).max(0.),
    ]
}
impl Runtime {
    /// Text area inside an editor. Single-line fields center one line vertically;
    /// multiline areas keep a fixed top inset for their explicit lines.
    pub(super) fn editor_inner(&self, b: &LayoutBox) -> [f32; 4] {
        inner_geometry(b, self.scale)
    }
    pub(super) fn paint_editor<P: DocumentPainter>(
        &self,
        p: &mut P,
        ui: &UiTheme,
        b: &LayoutBox,
    ) -> PaintBatch<P::Shape, P::Glyph> {
        let c = ui.colors();
        let r = b.bounds.bounds();
        let mut shapes = Vec::new();
        let mut glyphs = Vec::new();
        let focused = self.focused.as_deref() == Some(&b.id);
        let options = b.editor.as_ref().unwrap();
        let disabled = !b.enabled;
        let alpha = if disabled { 0.45 } else { 1. };
        let tint = |mut color: [f32; 4]| {
            color[3] *= alpha;
            color
        };
        paint_surface(
            p,
            &mut shapes,
            r,
            b.radius_spec(self.scale),
            tint(b.style.background.unwrap_or(c.panel)),
        );
        paint_border(
            p,
            &mut shapes,
            r,
            b.radius_spec(self.scale),
            b.style.border_width.unwrap_or(self.scale),
            tint(if focused {
                b.style.accent_color.unwrap_or(c.accent)
            } else {
                b.style.border_color.unwrap_or(c.border)
            }),
        );
        let inner = self.editor_inner(b);
        if inner[2] <= 0. || inner[3] <= 0. {
            return PaintBatch {
                clip: b.clip,
                shapes,
                glyphs,
            };
        }
        let editors = self.editors.borrow();
        let Some(e) = editors.get(&b.id) else {
            return PaintBatch {
                clip: b.clip,
                shapes,
                glyphs,
            };
        };
        let line_height = editor_line_height(&b.text_style);
        let selected = e.range();
        let clip = Rect::new(inner[0], inner[1], inner[2], inner[3]).unwrap();
        if focused {
            let range = e.composition_range.as_ref().unwrap_or(&selected);
            if !range.is_empty() {
                let mut color = b.style.accent_color.unwrap_or(c.accent);
                let composing = e.composition_range.is_some();
                if !composing {
                    color[3] *= 0.3;
                }
                for (row, line) in e.rows.iter().enumerate() {
                    if range.start > line.end || range.end <= line.start {
                        continue;
                    }
                    let y = row as f32 * line_height;
                    let pos = |i| {
                        e.display_positions
                            .iter()
                            .find(|(at, p)| *at == i && (p[1] - y).abs() < 0.5)
                            .map_or(0., |(_, p)| p[0])
                    };
                    let low = range.start.max(line.start);
                    let high = range.end.min(line.end);
                    let left = inner[0] + pos(low) - e.offset[0];
                    let right = inner[0] + pos(high) - e.offset[0]
                        + if !composing && range.end > line.end {
                            4. * self.scale
                        } else {
                            0.
                        };
                    let top = inner[1] + y - e.offset[1]
                        + if composing {
                            line_height - self.scale
                        } else {
                            0.
                        };
                    let height = if composing { self.scale } else { line_height };
                    if let Some(rect) = Rect::new(left, top, (right - left).max(0.1), height)
                        .and_then(|r| intersection(r, clip))
                    {
                        let [x, y, w, h] = rect.bounds();
                        p.draw_rect(&mut shapes, x, y, w, h, color);
                    }
                }
            }
        }
        if e.text.is_empty() && e.preedit.is_empty() {
            p.ui_text_styled(
                &mut glyphs,
                &options.placeholder,
                inner,
                &b.text_style,
                tint(c.muted),
            );
        } else {
            let row_style = editor_row_style(&b.text_style);
            for (row, line) in e.rows.iter().enumerate() {
                let top = inner[1] + row as f32 * line_height - e.offset[1];
                if top + line_height <= inner[1] || top >= inner[1] + inner[3] {
                    continue;
                }
                // Same independently-shaped segments and widths used by visual_rows.
                p.ui_text_styled(
                    &mut glyphs,
                    &line.text,
                    [
                        inner[0] + line.x - e.offset[0],
                        top,
                        1_000_000.,
                        line_height,
                    ],
                    &row_style,
                    tint(b.style.color.unwrap_or(c.text)),
                );
            }
        }
        if focused && (e.preedit.is_empty() || e.preedit_cursor.is_some()) {
            let pos = e.paint_caret;
            let cx = inner[0] + pos[0] - e.offset[0];
            let cy = inner[1] + pos[1] - e.offset[1];
            if let Some(caret) = Rect::new(cx, cy, self.scale.max(1.), line_height)
                .and_then(|r| intersection(r, clip))
            {
                let [x, y, w, h] = caret.bounds();
                p.draw_rect(&mut shapes, x, y, w, h, c.text);
            }
        }
        // Split content into separate batches at Runtime::paint_nodes for innerclip.
        PaintBatch {
            clip: b.clip,
            shapes,
            glyphs,
        }
    }
}
