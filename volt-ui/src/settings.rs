//! Preferences and onboarding state. No terminal output or credentials are exposed.
use std::path::PathBuf;
use volt_config::{
    config::{PromptMode, WorkspaceLayout},
    preferences::Preferences,
    Config, ThemeRegistry,
};
use volt_renderer::settings::{SettingsHit, SettingsRow, SettingsView};
#[derive(Clone)]
enum Control {
    Text,
    Number(f32),
    Choice(Vec<String>),
    Toggle,
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
        let toggle =
            |id, label, v: bool, hint| row(id, label, if v { "On" } else { "Off" }, hint, Toggle);
        match self.page() {
            0 => vec![
                row(
                    "theme",
                    "Theme",
                    &c.theme,
                    "Use − / + or arrows to select a theme",
                    Choice(self.themes.iter().map(|e| e.0.clone()).collect()),
                ),
                row(
                    "font",
                    "Font family",
                    &c.font.family,
                    "Installed fonts; Enter to type a family name",
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
                    adjustable: matches!(r.control, Control::Number(_) | Control::Choice(_)),
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
    pub fn section(&mut self, n: usize) -> Outcome {
        let edited = self.edit.is_some();
        if edited && self.commit() == Outcome::Redraw {
            return Outcome::Redraw;
        }
        self.edit = None;
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
            Control::Toggle => self.adjust(1),
            _ => {
                let chars: Vec<char> = r.value.chars().collect();
                let n = chars.len();
                self.edit = Some((chars, n));
                self.select_all = true;
                self.status =
                    "Type a value; Enter confirms. Shortcuts: press the new chord.".into();
                Outcome::Redraw
            }
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
            Control::Toggle => self.assign(&r.id, if r.value == "On" { "Off" } else { "On" }),
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
            7 => self.draft.editor = defaults.editor,
            _ => {}
        }
        self.edit = None;
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
        assert_eq!(rows.len(), 30);
        if cfg!(target_os = "macos") {
            assert!(rows
                .iter()
                .all(|r| !r.value.contains("super") && !r.value.contains("alt+")));
            assert_eq!(rows[0].value, "Cmd+T");
            assert_eq!(rows[6].value, "Cmd+,");
        }
    }
}
