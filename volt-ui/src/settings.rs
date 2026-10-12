//! Preferences and onboarding state. No terminal output or credentials are exposed.
use std::path::PathBuf;
use volt_config::{
    config::{PromptMode, WorkspaceLayout},
    preferences::Preferences,
    Config, ThemeRegistry,
};
use volt_renderer::settings::{
    dropdown_rect, inside, slider_track, SettingsControl, SettingsDropdown, SettingsHit,
    SettingsLayout, SettingsRow, SettingsView, DROPDOWN_ROWS,
};
#[derive(Clone)]
enum Control {
    Text,
    Number(f32),
    Choice(Vec<String>),
    Toggle(bool),
    Action(Outcome),
    Shortcut(volt_config::keybindings::Action),
}
#[derive(Clone)]
struct Row {
    id: String,
    label: String,
    value: String,
    hint: String,
    control: Control,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    Redraw,
    Preview,
    Apply,
    Cancel,
    Skip,
    Config,
    Themes,
    Tasks,
    AddTask,
    Secure,
    GettingStarted,
}
struct Dropdown {
    values: Vec<String>,
    labels: Vec<String>,
    selected: usize,
    current: usize,
    offset: usize,
    custom: bool,
    prefix: String,
}
pub struct Settings {
    pub draft: Config,
    pub initial: Config,
    pub transaction: Preferences,
    pub onboarding: bool,
    pub section: usize,
    pub focus: usize,
    offset: usize,
    capacity: usize,
    edit: Option<(Vec<char>, usize)>,
    select_all: bool,
    dropdown: Option<Dropdown>,
    slider_drag: Option<usize>,
    pub status: String,
    themes: Vec<(String, String)>,
    fonts: Vec<String>,
    shells: Vec<String>,
}
fn row(id: &str, label: &str, value: impl ToString, hint: &str, control: Control) -> Row {
    Row {
        id: id.into(),
        label: label.into(),
        value: value.to_string(),
        hint: hint.into(),
        control,
    }
}
impl Settings {
    pub fn open(
        path: PathBuf,
        current: &Config,
        themes: &ThemeRegistry,
        fonts: Vec<String>,
        onboarding: bool,
    ) -> std::io::Result<Self> {
        let transaction = Preferences::open(path)?;
        let mut draft = if transaction.is_new() {
            current.clone()
        } else {
            transaction.initial.clone()
        };
        if draft.theme.is_empty() {
            draft.theme = "catppuccin".into();
        }
        if onboarding
            && transaction.is_new()
            && matches!(
                std::path::Path::new(&draft.shell.program)
                    .file_name()
                    .and_then(|s| s.to_str()),
                Some("zsh" | "bash" | "fish")
            )
        {
            draft.shell.prompt = PromptMode::Meow;
            draft.shell.integration = true;
        }
        let mut shells = vec![current.shell.program.clone()];
        for p in [
            "/bin/zsh",
            "/bin/bash",
            "/usr/bin/bash",
            "/opt/homebrew/bin/fish",
            "/usr/local/bin/fish",
            "/usr/bin/fish",
        ] {
            if std::path::Path::new(p).is_file() {
                shells.push(p.into());
            }
        }
        shells.sort();
        shells.dedup();
        Ok(Self {
            draft,
            initial: current.clone(),
            transaction,
            onboarding,
            section: 0,
            focus: 0,
            offset: 0,
            capacity: 3,
            edit: None,
            select_all: false,
            dropdown: None,
            slider_drag: None,
            status: String::new(),
            themes: themes
                .entries()
                .iter()
                .map(|e| (e.id.clone(), e.name.clone()))
                .collect(),
            fonts,
            shells,
        })
    }
    pub fn sections(&self) -> Vec<String> {
        if self.onboarding {
            ["Welcome", "Appearance", "Workflow", "Shell & prompt"]
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            [
                "Appearance",
                "Shell & prompt",
                "Terminal",
                "Workspace",
                "Tasks",
                "Shortcuts",
                "Security",
                "Advanced",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect()
        }
    }
    pub fn page(&self) -> usize {
        if self.onboarding {
            match self.section {
                1 => 0,
                3 => 1,
                _ => 8 + self.section,
            }
        } else {
            self.section
        }
    }
    fn rows(&self) -> Vec<Row> {
        use Control::*;
        let c = &self.draft;
        let toggle = |id, label, v: bool, hint| {
            row(id, label, if v { "On" } else { "Off" }, hint, Toggle(v))
        };
        match self.page() {
            0 => vec![
                row(
                    "theme",
                    "Theme",
                    &c.theme,
                    "Open the list; ↑↓ chooses, Enter previews, Esc dismisses",
                    Choice(self.themes.iter().map(|e| e.0.clone()).collect()),
                ),
                row(
                    "font",
                    "Font family",
                    &c.font.family,
                    "Choose an installed font or Custom… to type a family name",
                    Choice(self.fonts.clone()),
                ),
                row(
                    "size",
                    "Font size",
                    c.font.size,
                    "6–72 points • changes preview live",
                    Number(1.),
                ),
                row(
                    "height",
                    "Line height",
                    c.appearance.line_height,
                    "1.0–3.0 • text and glyph alignment",
                    Number(0.1),
                ),
                row(
                    "padding",
                    "Padding",
                    c.appearance.padding,
                    "0–100 logical pixels",
                    Number(1.),
                ),
                row(
                    "opacity",
                    "Opacity",
                    c.appearance.opacity,
                    "0.1–1.0 • window transparency",
                    Number(0.05),
                ),
                toggle(
                    "blur",
                    "Background blur",
                    c.appearance.blur_enabled(),
                    "Platform compositor support varies",
                ),
                row(
                    "customize",
                    "Customize theme",
                    "Open editor",
                    "Apply changes first; opens the existing color editor",
                    Action(Outcome::Themes),
                ),
            ],
            1 => vec![
                row(
                    "shell",
                    "Shell",
                    &c.shell.program,
                    "Finish opens a ready tab; later shell changes affect new tabs/panes",
                    Choice(self.shells.clone()),
                ),
                row(
                    "args",
                    "Shell arguments",
                    toml::Value::Array(
                        c.shell
                            .args
                            .iter()
                            .map(|s| toml::Value::String(s.clone()))
                            .collect(),
                    )
                    .to_string(),
                    "TOML array, for example [\"-l\"]",
                    Text,
                ),
                row(
                    "prompt",
                    "Prompt",
                    if c.shell.prompt == PromptMode::Meow {
                        "Meow"
                    } else {
                        "Existing"
                    },
                    "Meow is bundled; Existing preserves your startup prompt",
                    Choice(vec!["Existing".into(), "Meow".into()]),
                ),
                toggle(
                    "integration",
                    "Shell integration",
                    c.shell.integration,
                    "New shells: prompt navigation and task completion signals",
                ),
            ],
            2 => vec![
                row(
                    "cursor",
                    "Cursor style",
                    format!("{:?}", c.appearance.cursor_style).to_lowercase(),
                    "Block, beam or underline",
                    Choice(vec!["block".into(), "beam".into(), "underline".into()]),
                ),
                toggle(
                    "blink",
                    "Cursor blinking",
                    c.appearance.cursor_blink,
                    "Blink while the terminal is focused",
                ),
                row(
                    "scrollback",
                    "Scrollback lines",
                    c.terminal.scrollback_lines,
                    "1–1000000 per pane; large history costs memory",
                    Number(1000.),
                ),
                row(
                    "divider",
                    "Pane divider opacity",
                    c.appearance.divider_opacity,
                    "0–1 • idle divider visibility",
                    Number(0.01),
                ),
            ],
            3 => vec![
                toggle(
                    "workspace_enabled",
                    "Workspace card at startup",
                    c.workspace.enabled,
                    "Cmd+Shift+A toggles the card; no discovery while closed",
                ),
                row(
                    "layout",
                    "Workspace placement",
                    if c.workspace.layout == WorkspaceLayout::Floating {
                        "floating"
                    } else {
                        "docked"
                    },
                    "Floating overlays content; docked reserves terminal space",
                    Choice(vec!["docked".into(), "floating".into()]),
                ),
                row(
                    "card",
                    "Workspace card",
                    "Open current-project context",
                    "Relevant project sections appear automatically",
                    Action(Outcome::Tasks),
                ),
            ],
            4 => vec![
                row(
                    "tasks",
                    "Project tasks",
                    "Open current-project tasks",
                    "Tasks are project-scoped, not global preferences",
                    Action(Outcome::Tasks),
                ),
                row(
                    "add",
                    "Add task",
                    "Open task form",
                    "Commands run in the current pane after trust/safety checks",
                    Action(Outcome::AddTask),
                ),
                row(
                    "info",
                    "Trust and confirmation",
                    "Review before running",
                    "Changing a task file invalidates previous review",
                    Action(Outcome::Redraw),
                ),
            ],
            5 => shortcut_rows(c),
            6 => vec![
                row(
                    "secure",
                    "Secure keyboard entry",
                    "Toggle for this app",
                    "macOS only; protects input, not output or clipboard",
                    Action(Outcome::Secure),
                ),
                row(
                    "protect",
                    "Task execution guards",
                    "Enabled",
                    "Running jobs, TUIs, read-only and hidden input block task injection",
                    Action(Outcome::Redraw),
                ),
                row(
                    "clipboard",
                    "Terminal clipboard requests",
                    "Disabled",
                    "OSC 52 is not implemented; ordinary copy/paste remains available",
                    Action(Outcome::Redraw),
                ),
            ],
            7 => vec![
                toggle("restore", "Restore workspace layout", c.session.restore,
                    "Next launch: tabs, splits and local folders only; no commands or output. Custom tabs only"),
                row(
                    "editor",
                    "Default file editor",
                    &c.editor.command,
                    "For example nvim or code --wait; empty uses environment/default app",
                    Text,
                ),
                row(
                    "config",
                    "Configuration file",
                    "Open config.toml",
                    "Advanced options, per-file editors and manual editing",
                    Action(Outcome::Config),
                ),
                row(
                    "guide",
                    "Getting started",
                    "Reopen introduction",
                    "Appearance, shortcuts, tasks and shell setup",
                    Action(Outcome::GettingStarted),
                ),
                row(
                    "reset",
                    "Reset page",
                    "Use footer Reset page",
                    "Only this page's draft values reset; Apply saves",
                    Action(Outcome::Redraw),
                ),
            ],
            8 => vec![
                row(
                    "welcome",
                    "Welcome to Volt",
                    "Your shell, your project",
                    "Skip anytime; no startup files are modified",
                    Action(Outcome::Redraw),
                ),
                row(
                    "setup",
                    "Make it yours",
                    "Theme, font and prompt",
                    "Settings can be reopened with Cmd+, / Ctrl+,",
                    Action(Outcome::Redraw),
                ),
                row(
                    "privacy",
                    "Local by default",
                    "No AI account required",
                    "Introduction examples never execute commands",
                    Action(Outcome::Redraw),
                ),
            ],
            _ => vec![
                row(
                    "keys",
                    "Tabs and panes",
                    "Cmd+T • Cmd+D",
                    "Cmd+Shift+Left/Right moves tabs; shortcuts are configurable",
                    Action(Outcome::Redraw),
                ),
                row(
                    "settings",
                    "Settings and search",
                    "Cmd+, • Cmd+F",
                    "Change preferences here; search project files or retained output",
                    Action(Outcome::Redraw),
                ),
                row(
                    "tasks",
                    "Project tasks",
                    ".volt/tasks.toml",
                    "Add or import reviewed commands; one click runs in your current pane",
                    Action(Outcome::Redraw),
                ),
                row(
                    "workspace",
                    "Workspace card",
                    "Cmd+Shift+A",
                    "Pin to dock or float; Git/project sections appear when relevant",
                    Action(Outcome::Redraw),
                ),
            ],
        }
    }
    pub fn set_capacity(&mut self, n: usize) {
        self.capacity = n.max(1);
        self.ensure_visible();
    }
    fn ensure_visible(&mut self) {
        let n = self.rows().len();
        self.focus = self.focus.min(n.saturating_sub(1));
        if self.focus < self.offset {
            self.offset = self.focus;
        }
        if self.focus >= self.offset + self.capacity {
            self.offset = self.focus + 1 - self.capacity;
        }
    }
    pub fn view(&self) -> SettingsView {
        let rows = self.rows();
        let page = self.page();
        SettingsView {
            title: if self.onboarding {
                "Make Volt yours"
            } else {
                "Settings"
            }
            .into(),
            subtitle: if self.onboarding {
                format!("Step {} of 4 • ↑↓ scroll • skip anytime", self.section + 1)
            } else {
                format!(
                    "Apply saves • Esc cancels • Rows {}–{} / {} • ↑↓ scroll",
                    self.offset + 1,
                    (self.offset + self.capacity).min(rows.len()),
                    rows.len()
                )
            },
            sections: self.sections(),
            section: self.section,
            rows: rows
                .into_iter()
                .skip(self.offset)
                .take(self.capacity)
                .enumerate()
                .map(|(i, r)| SettingsRow {
                    adjustable: false,
                    caret: self
                        .edit
                        .as_ref()
                        .filter(|_| i + self.offset == self.focus)
                        .map(|(_, i)| *i),
                    selected: self.select_all && i + self.offset == self.focus,
                    label: r.label,
                    value: if i + self.offset == self.focus {
                        self.edit
                            .as_ref()
                            .map(|(s, _)| s.iter().collect())
                            .unwrap_or(r.value)
                    } else {
                        r.value
                    },
                    hint: r.hint,
                    actionable: !matches!(r.control, Control::Action(Outcome::Redraw)),
                    editing: self.edit.is_some() && i + self.offset == self.focus,
                })
                .collect(),
            focus: self.focus - self.offset,
            status: self.status.clone(),
            onboarding: self.onboarding,
            preview: page == 0,
        }
    }
    /// Visible control metadata, independent of translated labels or values.
    pub fn toggle_rows(&self) -> Vec<Option<bool>> {
        self.rows()
            .into_iter()
            .skip(self.offset)
            .take(self.capacity)
            .map(|r| match r.control {
                Control::Toggle(checked) => Some(checked),
                _ => None,
            })
            .collect()
    }

    /// Semantic component types, never guessed from presentation strings.
    pub fn control_rows(&self) -> Vec<SettingsControl> {
        self.rows()
            .into_iter()
            .skip(self.offset)
            .take(self.capacity)
            .map(|r| match r.control {
                Control::Toggle(v) => SettingsControl::Switch(v),
                Control::Choice(_) => SettingsControl::Choice,
                Control::Number(_) if r.id == "opacity" || r.id == "divider" => {
                    SettingsControl::Slider {
                        value: r.value.parse().unwrap_or(1.),
                        min: if r.id == "opacity" { 0.1 } else { 0. },
                        max: 1.,
                    }
                }
                Control::Number(_) => SettingsControl::Number,
                Control::Text => SettingsControl::Text,
                Control::Shortcut(_) => SettingsControl::Shortcut,
                Control::Action(Outcome::Redraw) => SettingsControl::Info,
                Control::Action(o) => SettingsControl::Action(
                    match o {
                        Outcome::Themes => "Customize…",
                        Outcome::Tasks if r.id == "card" => "Open card",
                        Outcome::Tasks => "Open tasks",
                        Outcome::AddTask => "Add task…",
                        Outcome::Config => "Open config",
                        Outcome::Secure => "Toggle secure entry",
                        Outcome::GettingStarted => "Show introduction",
                        _ => "Open",
                    }
                    .into(),
                ),
            })
            .collect()
    }
    pub fn dropdown_view(&self) -> Option<SettingsDropdown> {
        self.dropdown.as_ref().map(|d| SettingsDropdown {
            row: self.focus.saturating_sub(self.offset),
            items: d.labels.clone(),
            selected: d.selected,
            current: d.current,
            offset: d.offset,
        })
    }
    pub fn dropdown_open(&self) -> bool {
        self.dropdown.is_some()
    }
    fn open_dropdown(&mut self, values: Vec<String>, row: &Row) -> Outcome {
        let custom = matches!(row.id.as_str(), "font" | "shell");
        let mut labels: Vec<String> = values
            .iter()
            .map(|v| {
                if row.id == "theme" {
                    self.themes
                        .iter()
                        .find(|e| e.0 == *v)
                        .map(|e| e.1.clone())
                        .unwrap_or(v.clone())
                } else {
                    v.clone()
                }
            })
            .collect();
        let mut values = values;
        if custom {
            values.insert(0, String::new());
            labels.insert(0, "Custom…".into());
        }
        if values.is_empty() {
            self.status = "No choices available".into();
            return Outcome::Redraw;
        }
        let selected = values.iter().position(|v| *v == row.value).unwrap_or(0);
        self.dropdown = Some(Dropdown {
            values,
            labels,
            selected,
            current: selected,
            offset: selected.saturating_sub(DROPDOWN_ROWS - 1),
            custom,
            prefix: String::new(),
        });
        self.status = "↑↓ chooses • Enter selects • type to jump • Esc dismisses".into();
        Outcome::Redraw
    }
    fn choose_dropdown(&mut self) -> Outcome {
        let Some(d) = self.dropdown.take() else {
            return Outcome::Redraw;
        };
        if d.custom && d.selected == 0 {
            return self.begin_edit();
        }
        let Some(row) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        self.assign(&row.id, &d.values[d.selected])
    }
    fn dropdown_move(&mut self, delta: isize) -> Outcome {
        if let Some(d) = &mut self.dropdown {
            d.selected = d
                .selected
                .saturating_add_signed(delta)
                .min(d.values.len() - 1);
            if d.selected < d.offset {
                d.offset = d.selected;
            }
            if d.selected >= d.offset + DROPDOWN_ROWS {
                d.offset = d.selected + 1 - DROPDOWN_ROWS;
            }
        }
        Outcome::Redraw
    }
    pub fn dropdown_key(&mut self, key: &str, text: &str) -> Outcome {
        match key {
            "Escape" => {
                self.dropdown = None;
                self.status.clear();
                Outcome::Redraw
            }
            "Enter" | "NumpadEnter" => self.choose_dropdown(),
            "ArrowDown" => self.dropdown_move(1),
            "ArrowUp" => self.dropdown_move(-1),
            "PageDown" => self.dropdown_move(DROPDOWN_ROWS as isize),
            "PageUp" => self.dropdown_move(-(DROPDOWN_ROWS as isize)),
            "Home" => self.dropdown_move(-isize::MAX),
            "End" => self.dropdown_move(isize::MAX),
            "Tab" => {
                self.dropdown = None;
                self.move_focus(1)
            }
            _ => {
                if !text.is_empty() && !text.chars().any(char::is_control) {
                    if let Some(d) = &mut self.dropdown {
                        if d.prefix.len() + text.len() <= 256 {
                            d.prefix.push_str(&text.to_lowercase());
                        }
                        let find = |prefix: &str| {
                            d.labels
                                .iter()
                                .position(|v| v.to_lowercase().starts_with(prefix))
                        };
                        if let Some(i) = find(&d.prefix).or_else(|| find(&text.to_lowercase())) {
                            d.selected = i;
                            d.offset = i.saturating_sub(DROPDOWN_ROWS - 1);
                        }
                    }
                }
                Outcome::Redraw
            }
        }
    }
    pub fn wheel(&mut self, delta: isize) -> Outcome {
        if self.dropdown.is_some() {
            self.dropdown_move(delta)
        } else {
            self.move_focus(delta)
        }
    }
    pub fn pointer_click(&mut self, l: &SettingsLayout, x: f32, y: f32) -> Outcome {
        if let Some(view) = self.dropdown_view() {
            let r = dropdown_rect(l, &view);
            if inside(r, x, y) {
                let slot = ((y - r[1] - 4. * l.scale) / (28. * l.scale)).floor();
                if slot >= 0. && slot < DROPDOWN_ROWS as f32 {
                    let index = view.offset + slot as usize;
                    if index < view.items.len() {
                        self.dropdown.as_mut().unwrap().selected = index;
                        return self.choose_dropdown();
                    }
                }
                return Outcome::Redraw;
            }
            // Dismiss without activating a control obscured by the menu.
            self.dropdown = None;
            self.status.clear();
            return Outcome::Redraw;
        }
        let hit = l.hit(x, y, self.sections().len());
        if let SettingsHit::Row(i) | SettingsHit::Plus(i) | SettingsHit::Minus(i) = hit {
            let absolute = self.offset + i;
            let Some(row) = self.rows().get(absolute).cloned() else {
                return Outcome::Redraw;
            };
            if matches!(row.id.as_str(), "opacity" | "divider") {
                let t = slider_track(l, i);
                if inside(
                    [
                        t[0] - 8. * l.scale,
                        t[1] - 8. * l.scale,
                        t[2] + 16. * l.scale,
                        20. * l.scale,
                    ],
                    x,
                    y,
                ) {
                    if self.edit.is_some() && self.commit() == Outcome::Redraw {
                        return Outcome::Redraw;
                    }
                    self.focus = absolute;
                    self.slider_drag = Some(absolute);
                    return self.set_slider(l, x);
                }
            }
            return self.hit(SettingsHit::Row(i));
        }
        self.hit(hit)
    }
    fn set_slider(&mut self, l: &SettingsLayout, x: f32) -> Outcome {
        let Some(index) = self.slider_drag else {
            return Outcome::Redraw;
        };
        let Some(row) = self.rows().get(index).cloned() else {
            return Outcome::Redraw;
        };
        if !x.is_finite() {
            return Outcome::Redraw;
        }
        let t = slider_track(l, index.saturating_sub(self.offset));
        let min = if row.id == "opacity" { 0.1 } else { 0. };
        let value = min + ((x - t[0]) / t[2]).clamp(0., 1.) * (1. - min);
        self.assign(&row.id, &format!("{:.2}", value))
    }
    pub fn pointer_drag(&mut self, l: &SettingsLayout, x: f32) -> Option<Outcome> {
        self.slider_drag.map(|_| self.set_slider(l, x))
    }
    pub fn pointer_move(&mut self, l: &SettingsLayout, x: f32, y: f32) -> Option<Outcome> {
        if self.slider_drag.is_some() {
            return self.pointer_drag(l, x);
        }
        let view = self.dropdown_view()?;
        let r = dropdown_rect(l, &view);
        if inside(r, x, y) {
            let slot = ((y - r[1] - 4. * l.scale) / (28. * l.scale)).floor();
            if slot >= 0. && slot < DROPDOWN_ROWS as f32 {
                let index = view.offset + slot as usize;
                if index < view.items.len() && index != view.selected {
                    self.dropdown.as_mut()?.selected = index;
                    return Some(Outcome::Redraw);
                }
            }
        }
        None
    }
    pub fn pointer_release(&mut self) {
        self.slider_drag = None;
    }

    pub fn section(&mut self, n: usize) -> Outcome {
        let edited = self.edit.is_some();
        if edited && self.commit() == Outcome::Redraw {
            return Outcome::Redraw;
        }
        self.edit = None;
        self.dropdown = None;
        self.slider_drag = None;
        self.section = n.min(self.sections().len() - 1);
        self.focus = 0;
        self.offset = 0;
        self.status.clear();
        if edited {
            Outcome::Preview
        } else {
            Outcome::Redraw
        }
    }
    pub fn move_focus(&mut self, delta: isize) -> Outcome {
        let edited = self.edit.is_some();
        if edited && self.commit() == Outcome::Redraw {
            return Outcome::Redraw;
        }
        self.edit = None;
        self.dropdown = None;
        self.slider_drag = None;
        self.focus = self
            .focus
            .saturating_add_signed(delta)
            .min(self.rows().len().saturating_sub(1));
        self.ensure_visible();
        if edited {
            Outcome::Preview
        } else {
            Outcome::Redraw
        }
    }
    pub fn hit(&mut self, h: SettingsHit) -> Outcome {
        let edited = self.edit.is_some()
            && matches!(
                h,
                SettingsHit::Row(_)
                    | SettingsHit::Plus(_)
                    | SettingsHit::Minus(_)
                    | SettingsHit::Section(_)
                    | SettingsHit::Apply
            );
        if edited && self.commit() == Outcome::Redraw {
            return Outcome::Redraw;
        }
        let out = match h {
            SettingsHit::Section(i) => self.section(i),
            SettingsHit::Row(i) => {
                self.edit = None;
                self.focus = self.offset + i;
                self.activate()
            }
            SettingsHit::Plus(i) | SettingsHit::Minus(i) => {
                self.edit = None;
                self.focus = self.offset + i;
                self.adjust(if matches!(h, SettingsHit::Plus(_)) {
                    1
                } else {
                    -1
                })
            }
            SettingsHit::Apply => self.advance(),
            SettingsHit::Cancel => {
                if self.onboarding {
                    Outcome::Skip
                } else {
                    Outcome::Cancel
                }
            }
            SettingsHit::Back => {
                if self.onboarding {
                    self.section(self.section.saturating_sub(1))
                } else {
                    self.reset_page()
                }
            }
            SettingsHit::Config => Outcome::Config,
            _ => Outcome::Redraw,
        };
        if edited && out == Outcome::Redraw {
            Outcome::Preview
        } else {
            out
        }
    }
    pub fn advance(&mut self) -> Outcome {
        if self.edit.is_some() && self.commit() == Outcome::Redraw {
            return Outcome::Redraw;
        }
        if self.onboarding && self.section < 3 {
            return self.section(self.section + 1);
        }
        Outcome::Apply
    }
    pub fn activate(&mut self) -> Outcome {
        let Some(r) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        match r.control {
            Control::Action(o) => o,
            Control::Toggle(_) => self.adjust(1),
            Control::Choice(ref values) => self.open_dropdown(values.clone(), &r),
            _ => self.begin_edit(),
        }
    }
    fn begin_edit(&mut self) -> Outcome {
        let Some(r) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        {
            let chars: Vec<char> = r.value.chars().collect();
            let n = chars.len();
            self.edit = Some((chars, n));
            self.select_all = true;
            self.status = "Type a value; Enter confirms. Shortcuts: press the new chord.".into();
            Outcome::Redraw
        }
    }
    pub fn selected_text(&self) -> Option<String> {
        self.edit
            .as_ref()
            .filter(|_| self.select_all)
            .map(|(s, _)| s.iter().collect())
    }
    pub fn cursor_edge(&mut self, end: bool) {
        self.select_all = false;
        if let Some((s, i)) = &mut self.edit {
            *i = if end { s.len() } else { 0 };
        }
    }
    pub fn is_editing(&self) -> bool {
        self.edit.is_some()
    }
    pub fn escape(&mut self) -> Outcome {
        if self.dropdown.take().is_some() {
            self.status.clear();
            return Outcome::Redraw;
        }
        self.slider_drag = None;
        if self.edit.take().is_some() {
            self.status.clear();
            Outcome::Redraw
        } else if self.onboarding {
            Outcome::Skip
        } else {
            Outcome::Cancel
        }
    }
    pub fn text(&mut self, s: &str) -> Outcome {
        if self.dropdown.is_some() {
            return self.dropdown_key("Text", s);
        }
        if let Some((chars, cursor)) = &mut self.edit {
            if self.select_all {
                chars.clear();
                *cursor = 0;
                self.select_all = false;
            }
            for c in s.chars().filter(|c| !c.is_control()).take(256) {
                if chars.len() < 256 {
                    chars.insert(*cursor, c);
                    *cursor += 1;
                }
            }
        }
        Outcome::Redraw
    }
    pub fn erase(&mut self, delete: bool) -> Outcome {
        if self.edit.is_none() {
            if let Some(Row {
                control: Control::Shortcut(action),
                ..
            }) = self.rows().get(self.focus)
            {
                self.draft.keybindings.retain(|b| b.action != *action);
                self.status = "Shortcut override removed; Apply to save".into();
                return Outcome::Preview;
            }
        }
        if let Some((s, i)) = &mut self.edit {
            if self.select_all {
                s.clear();
                *i = 0;
                self.select_all = false;
                return Outcome::Redraw;
            }
            if delete && *i < s.len() {
                s.remove(*i);
            } else if !delete && *i > 0 {
                *i -= 1;
                s.remove(*i);
            }
        }
        Outcome::Redraw
    }
    pub fn select_all(&mut self) {
        if let Some((s, i)) = &mut self.edit {
            *i = s.len();
            self.select_all = true;
        }
    }
    pub fn cursor(&mut self, delta: isize) {
        self.select_all = false;
        if let Some((s, i)) = &mut self.edit {
            *i = i.saturating_add_signed(delta).min(s.len());
        }
    }
    pub fn shortcut(&mut self, chord: String) -> Outcome {
        let Some(r) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        if let Control::Shortcut(action) = r.control {
            match chord.clone().try_into() {
                Ok(key) => {
                    if self
                        .draft
                        .keybindings
                        .iter()
                        .any(|b| b.key == key && b.action != action)
                    {
                        self.status =
                            "Shortcut already assigned. Change the other action first.".into();
                        return Outcome::Redraw;
                    }
                    self.draft.keybindings.retain(|b| b.action != action);
                    self.draft
                        .keybindings
                        .push(volt_config::keybindings::KeyBinding { key, action });
                    self.edit = None;
                    self.status = "Shortcut updated; Apply to save".into();
                    Outcome::Preview
                }
                Err(_) => {
                    self.status = "Unsupported shortcut".into();
                    Outcome::Redraw
                }
            }
        } else {
            Outcome::Redraw
        }
    }
    pub fn recording_shortcut(&self) -> bool {
        self.edit.is_some()
            && self
                .rows()
                .get(self.focus)
                .is_some_and(|r| matches!(r.control, Control::Shortcut(_)))
    }
    pub fn commit(&mut self) -> Outcome {
        let Some((s, _)) = self.edit.take() else {
            return self.activate();
        };
        self.select_all = false;
        let value: String = s.into_iter().collect();
        let Some(r) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        let outcome = self.assign(&r.id, &value);
        if outcome == Outcome::Redraw {
            let chars: Vec<char> = value.chars().collect();
            let n = chars.len();
            self.edit = Some((chars, n));
            self.select_all = true;
        }
        outcome
    }
    pub fn adjust(&mut self, delta: isize) -> Outcome {
        let Some(r) = self.rows().get(self.focus).cloned() else {
            return Outcome::Redraw;
        };
        match r.control {
            Control::Number(step) => {
                let value = r.value.parse::<f32>().unwrap_or(0.) + step * delta as f32;
                self.assign(&r.id, &format!("{:.3}", value))
            }
            Control::Toggle(checked) => self.assign(&r.id, if checked { "Off" } else { "On" }),
            Control::Choice(values) => {
                if values.is_empty() {
                    return Outcome::Redraw;
                }
                let i = values.iter().position(|s| s == &r.value).unwrap_or(0);
                let n = (i as isize + delta).rem_euclid(values.len() as isize) as usize;
                self.assign(&r.id, &values[n])
            }
            Control::Action(o) => o,
            Control::Shortcut(_) => self.activate(),
            Control::Text => self.activate(),
        }
    }
    fn assign(&mut self, id: &str, v: &str) -> Outcome {
        let old = self.draft.clone();
        let c = &mut self.draft;
        let n = v.parse::<f32>().ok();
        let error = || "Enter a valid value".to_string();
        let result = (|| -> Result<(), String> {
            match id {
                "theme" => {
                    if !self.themes.iter().any(|e| e.0 == v) {
                        return Err("Choose an available theme".into());
                    }
                    c.theme = v.into();
                }
                "font" => c.font.family = v.into(),
                "size" => c.font.size = n.ok_or_else(error)?,
                "height" => c.appearance.line_height = n.ok_or_else(error)?,
                "padding" => {
                    let x = n.ok_or_else(error)?;
                    if !(0. ..=100.).contains(&x) {
                        return Err(error());
                    }
                    c.appearance.padding = x as u16;
                }
                "opacity" => {
                    c.appearance.opacity = n.ok_or_else(error)?;
                    c.appearance.transparent = false;
                }
                "blur" => {
                    c.appearance.blur = v == "On";
                    c.appearance.blur_amount = if v == "On" { 1. } else { 0. };
                }
                "cursor" => {
                    c.appearance.cursor_style = match v {
                        "block" => volt_config::CursorStyle::Block,
                        "beam" => volt_config::CursorStyle::Beam,
                        "underline" => volt_config::CursorStyle::Underline,
                        _ => return Err(error()),
                    }
                }
                "blink" => c.appearance.cursor_blink = v == "On",
                "scrollback" => {
                    let x = n.ok_or_else(error)?;
                    if !(1. ..=1_000_000.).contains(&x) {
                        return Err(error());
                    }
                    c.terminal.scrollback_lines = x as usize;
                }
                "divider" => c.appearance.divider_opacity = n.ok_or_else(error)?,
                "shell" => {
                    if !std::path::Path::new(v).is_file() {
                        return Err("Shell executable not found".into());
                    }
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if std::fs::metadata(v)
                            .map_err(|_| error())?
                            .permissions()
                            .mode()
                            & 0o111
                            == 0
                        {
                            return Err("Shell is not executable".into());
                        }
                    }
                    c.shell.program = v.into();
                }
                "args" => {
                    let value: toml::Value =
                        toml::from_str(&format!("args={v}")).map_err(|_| error())?;
                    c.shell.args = value["args"]
                        .as_array()
                        .ok_or_else(error)?
                        .iter()
                        .map(|v| v.as_str().map(String::from).ok_or_else(error))
                        .collect::<Result<_, _>>()?;
                }
                "prompt" => {
                    c.shell.prompt = if v == "Meow" {
                        PromptMode::Meow
                    } else {
                        PromptMode::Existing
                    }
                }
                "restore" => c.session.restore = v == "On",
                "integration" => c.shell.integration = v == "On",
                "workspace_enabled" => c.workspace.enabled = v == "On",
                "layout" => {
                    c.workspace.layout = if v == "floating" {
                        WorkspaceLayout::Floating
                    } else {
                        WorkspaceLayout::Docked
                    }
                }
                "editor" => c.editor.command = v.into(),
                _ => return Err(error()),
            }
            volt_config::preferences::validate(c)
        })();
        match result {
            Ok(()) => {
                self.status =
                    "Preview updated • Apply saves • shell changes affect new tabs".into();
                Outcome::Preview
            }
            Err(e) => {
                self.draft = old;
                self.status = e;
                Outcome::Redraw
            }
        }
    }
    fn reset_page(&mut self) -> Outcome {
        let defaults = Config::default();
        let page = self.page();
        match page {
            0 => {
                self.draft.font = defaults.font;
                self.draft.theme = "catppuccin".into();
                self.draft.appearance.padding = defaults.appearance.padding;
                self.draft.appearance.line_height = defaults.appearance.line_height;
                self.draft.appearance.opacity = defaults.appearance.opacity;
                self.draft.appearance.transparent = false;
                self.draft.appearance.blur = false;
                self.draft.appearance.blur_amount = 0.;
            }
            1 => self.draft.shell = defaults.shell,
            2 => {
                self.draft.terminal = defaults.terminal;
                self.draft.appearance.cursor_style = defaults.appearance.cursor_style;
                self.draft.appearance.cursor_blink = defaults.appearance.cursor_blink;
                self.draft.appearance.divider_opacity = defaults.appearance.divider_opacity;
            }
            3 => self.draft.workspace = defaults.workspace,
            5 => self.draft.keybindings.clear(),
            7 => {
                self.draft.editor = defaults.editor;
                self.draft.session = defaults.session;
            }
            _ => {}
        }
        self.edit = None;
        self.dropdown = None;
        self.slider_drag = None;
        self.status = "Page reset in draft; Apply to save".into();
        Outcome::Preview
    }
    pub fn save(&mut self, skip: bool) -> Result<(), String> {
        if !skip && self.edit.is_some() && self.commit() == Outcome::Redraw {
            return Err(self.status.clone());
        }
        if skip {
            self.draft = self.initial.clone();
        }
        if self.onboarding {
            self.draft.setup.completed = true;
        }
        self.transaction
            .save(&self.draft)
            .map_err(|e| e.to_string())
    }
}
fn shortcut_rows(c: &Config) -> Vec<Row> {
    use volt_config::keybindings::Action as A;
    [
        (A::NewTab, "New tab", "super+t"),
        (A::NewWindow, "New window", "super+n"),
        (A::ClosePane, "Close pane", "super+w"),
        (A::NextTab, "Next tab", "ctrl+tab"),
        (A::PreviousTab, "Previous tab", "ctrl+shift+tab"),
        (A::Find, "Search", "super+f"),
        (A::OpenConfig, "Settings", "super+comma"),
        (A::SearchWorkspace, "Workspace search", "super+shift+p"),
        (
            A::ToggleWorkspaceLayout,
            "Dock / float workspace",
            "Unassigned",
        ),
        (A::ToggleWorkspace, "Workspace card", "super+shift+a"),
        (A::SplitRight, "Split right", "super+d"),
        (A::SplitDown, "Split down", "super+shift+d"),
        (A::SplitLeft, "Split left", "Unassigned"),
        (A::SplitUp, "Split up", "Unassigned"),
        (A::FocusLeft, "Focus left", "super+alt+left"),
        (A::FocusRight, "Focus right", "super+alt+right"),
        (A::FocusUp, "Focus up", "super+alt+up"),
        (A::FocusDown, "Focus down", "super+alt+down"),
        (
            A::SelectCommandOutput,
            "Select last command output",
            "super+shift+o",
        ),
        (A::PreviousPrompt, "Previous prompt", "super+shift+up"),
        (A::NextPrompt, "Next prompt", "super+shift+down"),
        (A::IncreaseFontSize, "Increase font size", "super+equal"),
        (A::DecreaseFontSize, "Decrease font size", "super+minus"),
        (A::ToggleFullscreen, "Fullscreen", "Unassigned"),
        (A::ReloadConfig, "Reload configuration", "super+shift+r"),
        (A::CustomizeTheme, "Customize theme", "Unassigned"),
        (A::Quit, "Quit", "super+q"),
        (A::MoveTabLeft, "Move tab left", "shift+super+left"),
        (A::MoveTabRight, "Move tab right", "shift+super+right"),
        (A::Copy, "Copy", "super+c"),
        (A::Paste, "Paste", "super+v"),
    ]
    .iter()
    .map(|(a, label, default)| {
        let value = c
            .keybindings
            .iter()
            .rev()
            .find(|b| b.action == *a)
            .map(|b| String::from(b.key.clone()))
            .unwrap_or_else(|| default.to_string());
        let value = shortcut_label(&value, cfg!(target_os = "macos"));
        row(
            "shortcut",
            label,
            value,
            "Enter records a chord • Delete removes override • Reset page restores defaults",
            Control::Shortcut(*a),
        )
    })
    .collect()
}

/// Presentation only: configuration and keyboard dispatch keep canonical chords.
fn shortcut_label(raw: &str, macos: bool) -> String {
    let Ok(chord) = volt_config::keybindings::Chord::try_from(raw.to_string()) else {
        return raw.to_string();
    };
    let mut parts = Vec::new();
    for (bit, label) in [
        (1, "Ctrl"),
        (2, if macos { "Option" } else { "Alt" }),
        (4, "Shift"),
        (8, if macos { "Cmd" } else { "Super" }),
    ] {
        if chord.modifiers & bit != 0 {
            parts.push(label.to_string());
        }
    }
    let key = match chord.key.as_str() {
        "enter" if macos => "Return",
        "enter" => "Enter",
        "escape" => "Esc",
        "backspace" if macos => "Delete",
        "backspace" => "Backspace",
        "delete" if macos => "Forward Delete",
        "delete" => "Delete",
        "pageup" => "Page Up",
        "pagedown" => "Page Down",
        "equal" => "=",
        "minus" => "-",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "semicolon" => ";",
        "quote" => "'",
        "backquote" => "`",
        "bracketleft" => "[",
        "bracketright" => "]",
        "backslash" => "\\",
        other => {
            let mut chars = other.chars();
            let first = chars.next().unwrap_or_default().to_ascii_uppercase();
            parts.push(format!("{first}{}", chars.as_str()));
            return parts.join("+");
        }
    };
    parts.push(key.into());
    parts.join("+")
}
#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> Settings {
        let p = std::env::temp_dir().join(format!(
            "volt-settings-model-{}-{}.toml",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        Settings::open(
            p,
            &Config::default(),
            &ThemeRegistry::load(),
            vec!["monospace".into()],
            false,
        )
        .unwrap()
    }
    #[test]
    fn toolkit_toggles_follow_visible_rows_and_draft_changes() {
        let mut s = settings();
        s.section(3); // Workspace: toggle, placement choice, action.
        s.set_capacity(3);
        assert_eq!(
            s.toggle_rows(),
            vec![Some(s.draft.workspace.enabled), None, None]
        );
        let original = s.initial.workspace.enabled;
        s.focus = 0;
        s.hit(SettingsHit::Row(0));
        assert_eq!(s.toggle_rows()[0], Some(!original));
        assert_eq!(s.initial.workspace.enabled, original);
        assert!(s.transaction.is_new()); // No Apply/save happened.
                                         // The rightmost switch area shares the existing +/- hit zones.
        s.hit(SettingsHit::Plus(0));
        assert_eq!(s.toggle_rows()[0], Some(original));
        s.hit(SettingsHit::Minus(0));
        assert_eq!(s.toggle_rows()[0], Some(!original));
        s.set_capacity(1);
        s.move_focus(1);
        assert_eq!(s.toggle_rows(), vec![None]);
        s.move_focus(-1);
        assert_eq!(s.toggle_rows(), vec![Some(!original)]);
        s.section(5); // Shortcuts must not be mistaken for switches.
        assert!(s.toggle_rows().iter().all(Option::is_none));
    }
    #[test]
    fn invalid_preview_keeps_previous_value() {
        let mut s = settings();
        s.focus = 2;
        let old = s.draft.font.size;
        s.activate();
        s.select_all();
        s.text("999");
        assert_eq!(s.commit(), Outcome::Redraw);
        assert_eq!(s.draft.font.size, old);
    }
    #[test]
    fn section_and_scroll_are_bounded() {
        let mut s = settings();
        s.set_capacity(2);
        s.move_focus(100);
        assert_eq!(s.view().rows.len(), 2);
        assert_eq!(s.view().focus, 1);
        s.section(500);
        assert_eq!(s.section, 7);
    }
    #[test]
    fn skip_preserves_existing_preferences() {
        let mut s = settings();
        s.onboarding = true;
        s.draft.font.size = 20.;
        s.save(true).unwrap();
        assert_eq!(s.draft.font.size, s.initial.font.size);
        assert!(s.draft.setup.completed);
    }
    #[test]
    fn no_ai_secret_is_exposed() {
        let mut s = settings();
        s.draft.ai.api_key = "DO-NOT-EXPOSE".into();
        for i in 0..8 {
            s.section(i);
            assert!(!format!("{:?}", s.view()).contains("DO-NOT-EXPOSE"));
        }
    }
    #[test]
    fn apply_commits_text_and_invalid_edits_block_navigation() {
        let mut s = settings();
        s.focus = 2;
        s.activate();
        s.text("19");
        s.save(false).unwrap();
        assert_eq!(s.draft.font.size, 19.);
        s.activate();
        s.text("999");
        assert!(s.save(false).is_err());
        assert_eq!(s.section(1), Outcome::Redraw);
        assert_eq!(s.section, 0);
        assert!(s.is_editing());
    }
    #[test]
    fn edits_select_replace_copy_and_unicode_cursor() {
        let mut s = settings();
        s.focus = 1;
        s.activate();
        s.dropdown_key("Home", "");
        s.dropdown_key("Enter", ""); // Custom font opens the editable field.
        assert_eq!(s.selected_text().as_deref(), Some("monospace"));
        s.text("日本");
        s.cursor_edge(false);
        s.text("a");
        s.select_all();
        assert_eq!(s.selected_text().as_deref(), Some("a日本"));
        s.erase(false);
        assert_eq!(s.selected_text(), None);
    }
    #[test]
    fn every_page_exposes_typed_components_not_generic_steppers() {
        let mut s = settings();
        s.set_capacity(7);
        for section in 0..8 {
            s.section(section);
            assert_eq!(s.control_rows().len(), s.view().rows.len());
            assert!(s.view().rows.iter().all(|r| !r.adjustable));
        }
        s.section(0);
        let c = s.control_rows();
        assert!(matches!(c[0], SettingsControl::Choice));
        assert!(matches!(c[2], SettingsControl::Number));
        assert!(matches!(c[5], SettingsControl::Slider { .. }));
        assert!(matches!(c[6], SettingsControl::Switch(_)));
        s.section(4);
        assert!(matches!(s.control_rows()[0], SettingsControl::Action(_)));
        assert!(matches!(s.control_rows()[2], SettingsControl::Info));
        s.section(5);
        assert!(matches!(s.control_rows()[0], SettingsControl::Shortcut));
        s.onboarding = true;
        for section in 0..4 {
            s.section(section);
            assert_eq!(s.control_rows().len(), s.view().rows.len());
            assert!(s.view().rows.iter().all(|r| !r.adjustable));
        }
    }
    #[test]
    fn dropdown_keyboard_preview_escape_and_custom_font_are_transactional() {
        let mut s = settings();
        s.activate();
        let original = s.draft.theme.clone();
        assert!(s.dropdown_open());
        let current = s.dropdown_view().unwrap().current;
        s.dropdown_key("End", "");
        assert_eq!(s.dropdown_view().unwrap().current, current);
        assert_eq!(s.draft.theme, original);
        let selected =
            s.dropdown.as_ref().unwrap().values[s.dropdown.as_ref().unwrap().selected].clone();
        assert_eq!(s.dropdown_key("Enter", ""), Outcome::Preview);
        assert_eq!(s.draft.theme, selected);
        assert!(s.transaction.is_new());
        s.activate();
        s.dropdown_key("Home", "");
        assert_eq!(s.escape(), Outcome::Redraw);
        assert!(!s.dropdown_open());
        assert_eq!(s.draft.theme, selected);
        s.focus = 1;
        s.activate();
        s.dropdown_key("Home", "");
        s.dropdown_key("Enter", "");
        assert!(s.is_editing());
        s.text("Custom Mono");
        assert_eq!(s.commit(), Outcome::Preview);
        assert_eq!(s.draft.font.family, "Custom Mono");
    }
    #[test]
    fn dropdown_pointer_and_slider_drag_share_scaled_paint_geometry() {
        for scale in [1., 1.5, 2.] {
            let mut s = settings();
            let l = SettingsLayout::new(850. * scale, 680. * scale, 0., scale).unwrap();
            s.set_capacity(l.rows);
            let field = volt_renderer::settings::control_rect(&l, 0);
            s.pointer_click(&l, field[0] + field[2] - 4. * scale, field[1] + 10. * scale);
            assert!(s.dropdown_open()); // Right edge opens list, never cycles.
            let view = s.dropdown_view().unwrap();
            let r = dropdown_rect(&l, &view);
            assert_eq!(
                s.pointer_click(&l, r[0] + 30. * scale, r[1] + 17. * scale),
                Outcome::Preview
            );
            assert!(!s.dropdown_open());
            let t = slider_track(&l, 5);
            assert_eq!(s.pointer_click(&l, t[0], t[1]), Outcome::Preview);
            assert_eq!(s.draft.appearance.opacity, 0.1);
            s.pointer_drag(&l, t[0] + t[2] * 2.);
            assert_eq!(s.draft.appearance.opacity, 1.);
            s.pointer_release();
            assert!(s.pointer_drag(&l, t[0]).is_none());
            assert_eq!(
                s.initial.appearance.opacity,
                Config::default().appearance.opacity
            );
        }
    }
    #[test]
    fn shortcut_arrow_chord_validates_and_delete_restores() {
        let mut s = settings();
        s.section(5);
        s.focus = 0;
        s.activate();
        assert_eq!(s.shortcut("ctrl+alt+left".into()), Outcome::Preview);
        assert_eq!(s.draft.keybindings.len(), 1);
        assert_eq!(
            s.rows()[0].value,
            shortcut_label("ctrl+alt+left", cfg!(target_os = "macos"))
        );
        assert_eq!(
            String::from(s.draft.keybindings[0].key.clone()),
            "ctrl+alt+left"
        );
        s.erase(true);
        assert!(s.draft.keybindings.is_empty());
        assert_eq!(
            s.rows()[0].value,
            shortcut_label("super+t", cfg!(target_os = "macos"))
        );
    }
    #[test]
    fn shortcut_labels_are_platform_specific_without_changing_serialization() {
        for (raw, mac, other) in [
            ("super+t", "Cmd+T", "Super+T"),
            ("super+comma", "Cmd+,", "Super+,"),
            ("ctrl+shift+tab", "Ctrl+Shift+Tab", "Ctrl+Shift+Tab"),
            (
                "command+option+shift+left",
                "Option+Shift+Cmd+Left",
                "Alt+Shift+Super+Left",
            ),
            ("cmd+f12", "Cmd+F12", "Super+F12"),
            ("backspace", "Delete", "Backspace"),
            ("delete", "Forward Delete", "Delete"),
            ("enter", "Return", "Enter"),
            ("Unassigned", "Unassigned", "Unassigned"),
        ] {
            assert_eq!(shortcut_label(raw, true), mac);
            assert_eq!(shortcut_label(raw, false), other);
        }
        let chord = volt_config::keybindings::Chord::try_from("Cmd+Shift+F".to_string()).unwrap();
        assert_eq!(String::from(chord), "shift+super+f");
        for key in [
            "equal",
            "minus",
            "period",
            "slash",
            "semicolon",
            "quote",
            "backquote",
            "bracketleft",
            "bracketright",
            "backslash",
            "pageup",
            "pagedown",
            "escape",
            "space",
            "up",
            "home",
            "end",
        ] {
            let label = shortcut_label(&format!("super+{key}"), true);
            assert!(label.starts_with("Cmd+"));
            assert!(!label.contains("super"));
        }
    }
    #[test]
    fn all_default_shortcuts_use_native_names() {
        let rows = shortcut_rows(&Config::default());
        assert_eq!(rows.len(), 31);
        if cfg!(target_os = "macos") {
            assert!(rows
                .iter()
                .all(|r| !r.value.contains("super") && !r.value.contains("alt+")));
            assert_eq!(rows[0].value, "Cmd+T");
            assert_eq!(rows[6].value, "Cmd+,");
        }
    }
}
