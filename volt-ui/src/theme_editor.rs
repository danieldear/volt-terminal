//! Theme editor state: which color slot is focused, the hex text being typed,
//! the working copy applied live to the window, and the "Save as" name. Pure
//! state with no window or file access — `App` turns outcomes into theme
//! changes and saves, and the renderer draws `view()`.
use volt_config::{Color, Theme};
use volt_renderer::theme_editor::{EditorRow, ThemeEditorView, BASE_FIELDS, FIELD_COUNT};

const BASE_LABELS: [&str; BASE_FIELDS] = [
    "Background",
    "Foreground",
    "Cursor",
    "Cursor text",
    "Selection",
    "Selection text",
];
const ANSI_NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// What the caller must do after an edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorOutcome {
    /// Only the editor's own presentation changed.
    Redraw,
    /// The working theme changed; apply `working()` to the window.
    ThemeChanged,
    /// Close the editor and restore `original()`.
    Cancel,
    /// Write the working theme. `overwrite` is the user theme id to replace;
    /// `None` saves a new file named after `name`.
    Save {
        name: String,
        overwrite: Option<String>,
    },
}

pub struct ThemeEditor {
    /// Display name of the theme being edited (or last saved).
    base_name: String,
    /// Id of the user theme that plain Save overwrites. `None` for built-ins,
    /// which can only be saved under a new name.
    target_id: Option<String>,
    /// What Revert and Cancel go back to.
    original: Theme,
    working: Theme,
    field: usize,
    /// ANSI slot shown in the detail row while a base color is focused.
    last_ansi: usize,
    /// Text of the focused hex field, possibly a partial color.
    input: String,
    /// The field shows the committed color; the next hex digit replaces it.
    fresh: bool,
    naming: Option<(Vec<char>, usize)>,
    status: Option<(String, bool)>,
}

fn slot(theme: &Theme, i: usize) -> Color {
    match i {
        0 => theme.background,
        1 => theme.foreground,
        2 => theme.cursor,
        3 => theme.cursor_text,
        4 => theme.selection_bg,
        5 => theme.selection_fg,
        _ => theme.ansi[i - BASE_FIELDS],
    }
}

fn set_slot(theme: &mut Theme, i: usize, color: Color) {
    match i {
        0 => theme.background = color,
        1 => theme.foreground = color,
        2 => theme.cursor = color,
        3 => theme.cursor_text = color,
        4 => theme.selection_bg = color,
        5 => theme.selection_fg = color,
        _ => theme.ansi[i - BASE_FIELDS] = color,
    }
}

fn ansi_label(i: usize) -> String {
    let bright = if i >= 8 { "bright " } else { "" };
    format!("ANSI {i} · {bright}{}", ANSI_NAMES[i % 8])
}

impl ThemeEditor {
    /// `target_id` is the user theme's id, or `None` when editing a built-in.
    pub fn new(base_name: &str, target_id: Option<String>, theme: Theme) -> Self {
        let mut editor = Self {
            base_name: base_name.to_string(),
            target_id,
            original: theme.clone(),
            working: theme,
            field: 0,
            last_ansi: 0,
            input: String::new(),
            fresh: true,
            naming: None,
            status: None,
        };
        editor.reset_input();
        editor
    }

    pub fn working(&self) -> &Theme {
        &self.working
    }

    pub fn original(&self) -> &Theme {
        &self.original
    }

    pub fn is_naming(&self) -> bool {
        self.naming.is_some()
    }

    fn reset_input(&mut self) {
        self.input = slot(&self.working, self.field).to_hex();
        self.fresh = true;
    }

    /// Focus a color field, leaving "Save as" if it was open.
    pub fn focus(&mut self, field: usize) -> EditorOutcome {
        if field < FIELD_COUNT {
            self.naming = None;
            self.field = field;
            if field >= BASE_FIELDS {
                self.last_ansi = field - BASE_FIELDS;
            }
            self.reset_input();
        }
        EditorOutcome::Redraw
    }

    pub fn next_field(&mut self) -> EditorOutcome {
        self.focus((self.field + 1) % FIELD_COUNT)
    }

    pub fn prev_field(&mut self) -> EditorOutcome {
        self.focus((self.field + FIELD_COUNT - 1) % FIELD_COUNT)
    }

    /// Typed or committed text: the name while naming, else hex digits for
    /// the focused field. Anything else is ignored.
    pub fn type_text(&mut self, text: &str) -> EditorOutcome {
        if let Some((name, cursor)) = self.naming.as_mut() {
            for c in text.chars().filter(|c| !c.is_control()) {
                if name.len() >= volt_config::themes::MAX_THEME_NAME_CHARS {
                    break;
                }
                name.insert(*cursor, c);
                *cursor += 1;
            }
            self.status = None;
            return EditorOutcome::Redraw;
        }
        let mut changed = false;
        for c in text.chars() {
            if c == '#' {
                if self.fresh || self.input.is_empty() {
                    self.input = "#".into();
                    self.fresh = false;
                }
                continue;
            }
            if !c.is_ascii_hexdigit() {
                continue;
            }
            if self.fresh {
                self.input = "#".into();
                self.fresh = false;
            }
            if !self.input.starts_with('#') {
                self.input.insert(0, '#');
            }
            if self.input.len() < 7 {
                self.input.push(c.to_ascii_lowercase());
                changed = true;
            }
        }
        if changed {
            self.apply_input()
        } else {
            EditorOutcome::Redraw
        }
    }

    /// Apply the field text if it is a complete color.
    fn apply_input(&mut self) -> EditorOutcome {
        self.status = None;
        match Color::from_hex(&self.input) {
            Some(color) if color != slot(&self.working, self.field) => {
                set_slot(&mut self.working, self.field, color);
                EditorOutcome::ThemeChanged
            }
            _ => EditorOutcome::Redraw,
        }
    }

    pub fn backspace(&mut self) -> EditorOutcome {
        if let Some((name, cursor)) = self.naming.as_mut() {
            if *cursor > 0 {
                name.remove(*cursor - 1);
                *cursor -= 1;
            }
            return EditorOutcome::Redraw;
        }
        self.fresh = false;
        if self.input.len() > 1 {
            self.input.pop();
        }
        EditorOutcome::Redraw
    }

    /// Cmd+Backspace: clear the name or the field.
    pub fn clear(&mut self) -> EditorOutcome {
        if let Some((name, cursor)) = self.naming.as_mut() {
            name.drain(..*cursor);
            *cursor = 0;
        } else {
            self.input = "#".into();
            self.fresh = false;
        }
        EditorOutcome::Redraw
    }

    /// Clipboard text: inserted into the name, or applied as a whole color.
    pub fn paste(&mut self, text: &str) -> EditorOutcome {
        if self.naming.is_some() {
            let line = text.lines().next().unwrap_or("");
            return self.type_text(line);
        }
        match Color::from_hex(text) {
            Some(color) => {
                self.input = color.to_hex();
                self.fresh = true;
                self.apply_input()
            }
            None => {
                self.status = Some(("Clipboard isn't a #rrggbb color".into(), true));
                EditorOutcome::Redraw
            }
        }
    }

    pub fn move_cursor(&mut self, delta: isize) -> EditorOutcome {
        if let Some((name, cursor)) = self.naming.as_mut() {
            *cursor = cursor.saturating_add_signed(delta).min(name.len());
        }
        EditorOutcome::Redraw
    }

    pub fn cursor_to_edge(&mut self, end: bool) -> EditorOutcome {
        if let Some((name, cursor)) = self.naming.as_mut() {
            *cursor = if end { name.len() } else { 0 };
        }
        EditorOutcome::Redraw
    }

    /// Enter confirms the name, otherwise moves to the next field.
    pub fn enter(&mut self) -> EditorOutcome {
        if self.naming.is_some() {
            self.confirm_name()
        } else {
            self.next_field()
        }
    }

    /// Esc leaves "Save as" first; a second Esc cancels the editor.
    pub fn escape(&mut self) -> EditorOutcome {
        if self.naming.take().is_some() {
            self.status = None;
            EditorOutcome::Redraw
        } else {
            EditorOutcome::Cancel
        }
    }

    /// Cmd+S / the Save button. Overwrites the user theme being edited;
    /// built-ins (and `save_as`) ask for a name first.
    pub fn save(&mut self, save_as: bool) -> EditorOutcome {
        if self.naming.is_some() {
            return self.confirm_name();
        }
        match (&self.target_id, save_as) {
            (Some(id), false) => EditorOutcome::Save {
                name: self.base_name.clone(),
                overwrite: Some(id.clone()),
            },
            _ => {
                let name: Vec<char> = if self.target_id.is_some() {
                    self.base_name.chars().collect()
                } else {
                    format!("{} Custom", self.base_name).chars().collect()
                };
                let cursor = name.len();
                self.naming = Some((name, cursor));
                self.status = None;
                EditorOutcome::Redraw
            }
        }
    }

    fn confirm_name(&mut self) -> EditorOutcome {
        let Some((name, _)) = &self.naming else {
            return EditorOutcome::Redraw;
        };
        let raw: String = name.iter().collect();
        match volt_config::themes::validate_name(&raw) {
            Ok(name) => EditorOutcome::Save {
                name,
                overwrite: None,
            },
            Err(err) => {
                self.status = Some((err, true));
                EditorOutcome::Redraw
            }
        }
    }

    /// The caller wrote the theme as `id`; further plain saves overwrite it.
    pub fn saved(&mut self, id: String, name: String) {
        self.status = Some((format!("Saved to themes/{id}.toml"), false));
        self.target_id = Some(id);
        self.base_name = name;
        self.original = self.working.clone();
        self.naming = None;
        self.reset_input();
    }

    pub fn save_failed(&mut self, message: String) {
        self.status = Some((message, true));
    }

    pub fn revert(&mut self) -> EditorOutcome {
        self.naming = None;
        self.status = None;
        if self.working == self.original {
            self.reset_input();
            return EditorOutcome::Redraw;
        }
        self.working = self.original.clone();
        self.reset_input();
        EditorOutcome::ThemeChanged
    }

    fn row(&self, i: usize, label: String) -> EditorRow {
        let color = slot(&self.working, i);
        if i == self.field && self.naming.is_none() {
            EditorRow {
                label,
                color,
                text: self.input.clone(),
                // Partial text isn't applied yet; flag it until it is.
                valid: self.fresh || Color::from_hex(&self.input).is_some(),
            }
        } else {
            EditorRow {
                label,
                color,
                text: color.to_hex(),
                valid: true,
            }
        }
    }

    pub fn view(&self) -> ThemeEditorView {
        let dirty = self.working != self.original;
        let ansi_slot = if self.field >= BASE_FIELDS {
            self.field - BASE_FIELDS
        } else {
            self.last_ansi
        };
        ThemeEditorView {
            subtitle: format!(
                "{}{}",
                self.base_name,
                if dirty { " · modified" } else { "" }
            ),
            rows: (0..BASE_FIELDS)
                .map(|i| self.row(i, BASE_LABELS[i].to_string()))
                .collect(),
            ansi: self.working.ansi,
            detail: self.row(BASE_FIELDS + ansi_slot, ansi_label(ansi_slot)),
            focus: self.field,
            naming: self
                .naming
                .as_ref()
                .map(|(name, cursor)| (name.iter().collect(), *cursor)),
            save_label: if self.target_id.is_some() {
                "Save".into()
            } else {
                "Save as…".into()
            },
            status: self.status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn editor() -> ThemeEditor {
        ThemeEditor::new("Nord", None, Theme::by_name("nord"))
    }

    #[test]
    fn typing_six_digits_applies_live_and_partial_input_does_not() {
        let mut e = editor();
        let before = e.working().background;
        assert_eq!(e.type_text("12345"), EditorOutcome::Redraw);
        assert_eq!(e.working().background, before, "partial hex is not applied");
        assert_eq!(e.view().rows[0].text, "#12345");
        assert_eq!(e.type_text("6"), EditorOutcome::ThemeChanged);
        assert_eq!(e.working().background, Color::from_hex("#123456").unwrap());
        // A seventh digit is ignored rather than corrupting the color.
        assert_eq!(e.type_text("7"), EditorOutcome::Redraw);
        assert_eq!(e.view().rows[0].text, "#123456");
    }

    #[test]
    fn first_digit_replaces_the_shown_color_and_junk_is_ignored() {
        let mut e = editor();
        e.type_text("zz#AB");
        assert_eq!(e.view().rows[0].text, "#ab");
        e.backspace();
        assert_eq!(e.view().rows[0].text, "#a");
        e.backspace();
        e.backspace();
        assert_eq!(e.view().rows[0].text, "#", "the # stays");
    }

    #[test]
    fn leaving_a_field_discards_partial_text_and_tab_wraps() {
        let mut e = editor();
        e.type_text("12");
        e.next_field();
        e.prev_field();
        assert_eq!(e.view().rows[0].text, e.working().background.to_hex());
        e.prev_field();
        assert_eq!(e.view().focus, FIELD_COUNT - 1);
        assert_eq!(e.view().detail.label, "ANSI 15 · bright white");
        e.focus(BASE_FIELDS + 1);
        e.type_text("#ff0000");
        assert_eq!(e.working().ansi[1], Color::from_hex("ff0000").unwrap());
        e.focus(0);
        assert_eq!(
            e.view().detail.label,
            "ANSI 1 · red",
            "detail remembers ANSI"
        );
    }

    #[test]
    fn paste_takes_a_whole_color_or_reports_an_error() {
        let mut e = editor();
        e.focus(1);
        assert_eq!(e.paste("  #A0B1C2\n"), EditorOutcome::ThemeChanged);
        assert_eq!(e.working().foreground, Color::from_hex("a0b1c2").unwrap());
        assert_eq!(e.paste("not a color"), EditorOutcome::Redraw);
        assert!(e.view().status.unwrap().1);
    }

    #[test]
    fn builtins_save_through_a_name_and_esc_backs_out_in_steps() {
        let mut e = editor();
        assert_eq!(e.view().save_label, "Save as…");
        assert_eq!(e.save(false), EditorOutcome::Redraw);
        assert_eq!(e.view().naming, Some(("Nord Custom".into(), 11)));
        // Hex digits now go to the name, not the colors.
        let bg = e.working().background;
        e.type_text("2");
        assert_eq!(e.working().background, bg);
        assert_eq!(
            e.enter(),
            EditorOutcome::Save {
                name: "Nord Custom2".into(),
                overwrite: None
            }
        );
        assert_eq!(e.escape(), EditorOutcome::Redraw);
        assert!(!e.is_naming());
        assert_eq!(e.escape(), EditorOutcome::Cancel);
    }

    #[test]
    fn empty_names_are_refused_in_place() {
        let mut e = editor();
        e.save(true);
        e.clear();
        assert_eq!(e.enter(), EditorOutcome::Redraw);
        assert!(e.is_naming());
        assert!(e.view().status.unwrap().1);
    }

    #[test]
    fn after_saving_plain_save_overwrites_and_revert_uses_the_saved_copy() {
        let mut e = editor();
        e.type_text("101010");
        e.save(false);
        e.saved("nord-custom".into(), "Nord Custom".into());
        assert_eq!(e.view().save_label, "Save");
        assert!(!e.view().subtitle.contains("modified"));
        assert_eq!(
            e.save(false),
            EditorOutcome::Save {
                name: "Nord Custom".into(),
                overwrite: Some("nord-custom".into())
            }
        );
        e.type_text("202020");
        assert!(e.view().subtitle.ends_with("· modified"));
        assert_eq!(e.revert(), EditorOutcome::ThemeChanged);
        assert_eq!(e.working().background, Color::from_hex("101010").unwrap());
        assert_eq!(e.revert(), EditorOutcome::Redraw);
        // Save as on a user theme starts from its own name.
        e.save(true);
        assert_eq!(e.view().naming.unwrap().0, "Nord Custom");
    }

    #[test]
    fn name_editing_moves_the_cursor_and_caps_length() {
        let mut e = editor();
        e.save(true);
        e.clear();
        e.type_text("ac");
        e.move_cursor(-1);
        e.type_text("b");
        assert_eq!(e.view().naming, Some(("abc".into(), 2)));
        e.cursor_to_edge(false);
        e.backspace();
        assert_eq!(e.view().naming, Some(("abc".into(), 0)));
        e.type_text(&"x".repeat(200));
        assert_eq!(
            e.view().naming.unwrap().0.chars().count(),
            volt_config::themes::MAX_THEME_NAME_CHARS
        );
    }
}
