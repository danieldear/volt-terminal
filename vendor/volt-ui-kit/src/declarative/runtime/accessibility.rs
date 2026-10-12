//! Toolkit-neutral accessibility snapshot and actions. Hosts map these nodes to
//! a platform API (the native host uses AccessKit); the runtime itself has no
//! assistive-technology dependency.
use super::*;

/// What a node is, for assistive technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessRole {
    /// A scroll view or other grouping that carries meaning.
    Group,
    ScrollView,
    Label,
    Button,
    CheckBox,
    Switch,
    TextInput,
    MultilineTextInput,
    Slider,
    TabList,
    Tab,
    RadioGroup,
    RadioButton,
    ListBox,
    ListBoxOption,
    Table,
    Row,
    ColumnHeader,
    Cell,
    Tree,
    TreeItem,
}
/// Table dimensions and zero-based row/column coordinates, independent of paint layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessTableInfo {
    pub rows: Option<usize>,
    pub columns: Option<usize>,
    pub row: Option<usize>,
    pub column: Option<usize>,
    /// None for unsorted columns, Some(true/false) for ascending/descending.
    pub sort_ascending: Option<bool>,
}
/// One node of the accessibility tree, in document order. `parent` indexes an
/// earlier node; roots have `None`. Bounds are physical pixels in the window.
#[derive(Clone, Debug, PartialEq)]
pub struct AccessNode {
    pub id: String,
    pub parent: Option<usize>,
    pub role: AccessRole,
    pub table: Option<AccessTableInfo>,
    /// True only for mounted activation targets, not semantic row/header groups.
    pub clickable: bool,
    pub label: String,
    /// Input hint, separate from its accessible name.
    pub placeholder: Option<String>,
    /// Current text of an input (never for passwords), or a slider's value.
    pub value: Option<String>,
    /// Slider value, minimum, maximum and step.
    pub numeric: Option<[f32; 4]>,
    pub checked: Option<bool>,
    pub selected: Option<bool>,
    pub expanded: Option<bool>,
    /// Zero-based hierarchy level and logical item index; never inferred from paint depth.
    pub level: Option<usize>,
    pub position_in_set: Option<usize>,
    pub size_of_set: Option<usize>,
    /// Logical offsets and maximums: [x, y, max_x, max_y].
    pub scroll: Option<[f32; 4]>,
    pub bounds: [f32; 4],
    pub enabled: bool,
    pub focused: bool,
    pub password: bool,
    pub read_only: bool,
}
/// A value a screen reader asks to set.
#[derive(Clone, Debug, PartialEq)]
pub enum AccessValue {
    Text(String),
    Number(f32),
}

impl Runtime {
    // Treat adapter requests as untrusted commands. Check the current mounted
    // target before touching focus, selection, composition or application state.
    fn access_target(&self, id: &str) -> Option<&LayoutBox> {
        self.scope_allows_id(id).then_some(())?;
        self.layout_boxes.iter().find(|b| {
            let [_, _, width, height] = b.bounds.bounds();
            b.id == id && b.enabled && !b.departing && width > 0. && height > 0.
        })
    }
    /// The current accessibility tree. While a modal overlay is open only its
    /// contents are exposed, matching what pointer and keyboard can reach.
    pub fn accessibility_nodes(&self) -> Vec<AccessNode> {
        let mut out: Vec<AccessNode> = Vec::new();
        // (layout depth, index in `out`) of the nearest kept ancestors.
        let mut ancestors: Vec<(usize, usize)> = Vec::new();
        let focused = self.focused.as_deref();
        let mut semantic_ids = BTreeMap::new();
        for b in &self.layout_boxes {
            while ancestors.last().is_some_and(|(depth, _)| *depth >= b.depth) {
                ancestors.pop();
            }
            if b.departing || !self.scope_allows_id(&b.id) {
                continue;
            }
            let [_, _, w, h] = b.bounds.bounds();
            let role = if let Some(role) = b.access_role {
                Some(role)
            } else if b.editor.is_some() {
                Some(if b.editor.as_ref().is_some_and(|e| e.multiline) {
                    AccessRole::MultilineTextInput
                } else {
                    AccessRole::TextInput
                })
            } else if b.slider.is_some() {
                Some(AccessRole::Slider)
            } else {
                match b.kind {
                    Kind::Button => Some(AccessRole::Button),
                    Kind::Checkbox => Some(AccessRole::CheckBox),
                    Kind::Toggle => Some(AccessRole::Switch),
                    Kind::Text | Kind::Badge if !b.text.trim().is_empty() => {
                        Some(AccessRole::Label)
                    }
                    _ if b.scroll.is_some() => Some(AccessRole::ScrollView),
                    _ if b.accessible_label.is_some() => Some(AccessRole::Group),
                    _ => None,
                }
            };
            let Some(role) = role.filter(|_| w > 0. && h > 0.) else {
                continue;
            };
            let editor = b.editor.as_ref();
            let (value, label) = match role {
                AccessRole::TextInput | AccessRole::MultilineTextInput => {
                    let placeholder = editor.map(|e| e.placeholder.clone()).unwrap_or_default();
                    // `editor_text` withholds passwords: no characters or length leak.
                    (self.editor_text(&b.id), placeholder)
                }
                _ => (None, b.text.clone()),
            };
            let numeric = b.slider.as_ref().map(|s| {
                let value = self.slider_values.get(&b.id).copied().unwrap_or(s.value);
                [value, s.min, s.max, s.step]
            });
            let parent = b
                .access_parent
                .as_deref()
                .and_then(|id| semantic_ids.get(id).copied())
                .or_else(|| ancestors.last().map(|(_, i)| *i));
            let index = out.len();
            out.push(AccessNode {
                id: b.id.clone(),
                parent,
                role,
                table: b.access_table,
                clickable: interactive(b.kind) && b.editor.is_none() && b.slider.is_none(),
                label: b.accessible_label.clone().unwrap_or(label),
                placeholder: editor.map(|e| e.placeholder.clone()),
                value: value.or_else(|| numeric.map(|n| format!("{}", n[0]))),
                numeric,
                checked: matches!(
                    role,
                    AccessRole::CheckBox | AccessRole::Switch | AccessRole::RadioButton
                )
                .then_some(b.checked),
                selected: b.access_selected,
                expanded: b.access_expanded.or(b.disclosure),
                level: b.access_level,
                position_in_set: b.access_position,
                size_of_set: b
                    .virtual_list
                    .as_ref()
                    .map(|p| p.count)
                    .or(b.access_set_size),
                scroll: b.scroll.map(|_| {
                    let offset = self.scroll_offset(&b.id);
                    [offset[0], offset[1], b.scroll_max[0], b.scroll_max[1]]
                }),
                bounds: b.bounds.bounds(),
                enabled: b.enabled,
                focused: focused == Some(b.id.as_str()),
                password: editor.is_some_and(|e| e.password),
                read_only: editor.is_some_and(|e| e.read_only),
            });
            semantic_ids.insert(b.id.as_str(), index);
            ancestors.push((b.depth, index));
        }
        out
    }
    /// Move keyboard focus to a node, as a screen reader's focus action does.
    pub fn accessibility_focus(&mut self, id: &str) -> bool {
        let reachable = self.access_target(id).is_some_and(interactive_box);
        if !reachable {
            return false;
        }
        let focus_changed = self.focused.as_deref() != Some(id);
        if focus_changed {
            self.cancel_input();
            self.focused = Some(id.to_owned());
        }
        self.focus_visible = true;
        self.reveal_focus(id) || focus_changed
    }
    /// Reveal a mounted semantic node without changing keyboard focus.
    pub fn accessibility_reveal(&mut self, id: &str) -> bool {
        self.access_target(id).is_some() && self.reveal_focus(id)
    }
    /// Page a named scroll viewport on each requested axis (-1, 0, or 1).
    /// Requests are scoped to this viewport; they never bubble into background UI.
    pub fn accessibility_scroll<C>(
        &mut self,
        id: &str,
        direction: [i8; 2],
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if direction.iter().any(|d| !(-1..=1).contains(d)) {
            return Ok(InputOutcome::default());
        }
        let Some(b) = self
            .access_target(id)
            .filter(|b| b.scroll.is_some())
            .cloned()
        else {
            return Ok(InputOutcome::default());
        };
        let mode = b.scroll.expect("checked scroll target");
        let [_, _, width, height] = b.bounds.bounds();
        let [top, right, bottom, left] = b.style.layout.padding;
        let extent = [width - left - right, height - top - bottom];
        let offset = self
            .scroll_offsets
            .entry(id.to_owned())
            .or_insert(b.scroll_offset);
        let before = *offset;
        for axis in 0..2 {
            if mode.axes()[axis] {
                let page = extent[axis].max(0.) * 0.8 / self.scale;
                offset[axis] = (offset[axis] + f32::from(direction[axis]) * page)
                    .clamp(0., b.scroll_max[axis]);
            }
        }
        let changed = before != *offset;
        if changed {
            self.invalidate_layout();
            self.cancel_input();
        }
        // The AX tree also contains clipped mounted viewports. Dispatch this
        // target directly, not just the paint-visible virtual-list candidates.
        let activated = changed && self.dispatch_virtual_window(&b, handlers, context)?;
        Ok(InputOutcome { changed, activated })
    }
    /// Press a button, checkbox or switch (screen reader "click").
    pub fn accessibility_activate<C>(
        &mut self,
        id: &str,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if !self
            .access_target(id)
            .is_some_and(|b| interactive(b.kind) && b.editor.is_none() && b.slider.is_none())
        {
            return Ok(InputOutcome::default());
        }
        let focused = self.accessibility_focus(id);
        let activated = self.activate(id, handlers, context)?;
        Ok(InputOutcome {
            changed: focused || activated,
            activated,
        })
    }
    /// Replace an input's text or set a slider's value.
    pub fn accessibility_set_value<C>(
        &mut self,
        id: &str,
        value: AccessValue,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        match value {
            AccessValue::Text(text) => {
                if text.len() > editing::MAX_TEXT {
                    return Ok(InputOutcome::default());
                }
                let Some(multiline) = self
                    .access_target(id)
                    .and_then(|b| b.editor.as_ref())
                    .filter(|e| !e.read_only)
                    .map(|e| e.multiline)
                else {
                    return Ok(InputOutcome::default());
                };
                let sanitized = editing::sanitize(&text, multiline);
                if sanitized.len() > editing::MAX_TEXT || (!text.is_empty() && sanitized.is_empty())
                {
                    return Ok(InputOutcome::default());
                }
                let focused = self.accessibility_focus(id);
                self.editor_select_all();
                let mut result = self.text_input(&text, handlers, context)?;
                result.changed |= focused;
                Ok(result)
            }
            AccessValue::Number(n) => {
                let Some(props) = self
                    .access_target(id)
                    .and_then(|b| b.slider.clone())
                    .filter(|_| n.is_finite())
                else {
                    return Ok(InputOutcome::default());
                };
                let focused = self.accessibility_focus(id);
                let activated =
                    self.emit_slider(id, value_controls::snap(&props, n), handlers, context)?;
                Ok(InputOutcome {
                    changed: focused || activated,
                    activated,
                })
            }
        }
    }
    /// Increase or decrease a slider by one step.
    pub fn accessibility_step<C>(
        &mut self,
        id: &str,
        increase: bool,
        handlers: &mut Handlers<'_, C>,
        context: &mut C,
    ) -> Result<InputOutcome, DocumentError> {
        if self.access_target(id).is_none_or(|b| b.slider.is_none()) {
            return Ok(InputOutcome::default());
        }
        let focused = self.accessibility_focus(id);
        let key = if increase {
            value_controls::ValueKey::Increase
        } else {
            value_controls::ValueKey::Decrease
        };
        let mut result = self.slider_key(key, handlers, context)?;
        result.changed |= focused;
        Ok(result)
    }
}
