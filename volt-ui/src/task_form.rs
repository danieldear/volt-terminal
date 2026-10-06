//! "Add task" form state: three text fields and a checkbox. Pure state with
//! no file access: `App` saves the task and the renderer draws `view()`.
use std::path::PathBuf;
use volt_config::tasks::TaskDef;
use volt_renderer::task_form::{FormField, TaskFormView, FIELDS};

const NAME: usize = 0;
const RUN: usize = 1;
const CWD: usize = 2;
/// Focus index of the "Ask before running" checkbox.
const CONFIRM: usize = FIELDS;
const LIMITS: [usize; FIELDS] = [
    volt_config::tasks::MAX_NAME_CHARS,
    volt_config::tasks::MAX_RUN_CHARS,
    256,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormOutcome {
    Redraw,
    Cancel,
    Save(TaskDef),
}

pub struct TaskForm {
    /// Project folder the task is saved to.
    pub root: PathBuf,
    values: [Vec<char>; FIELDS],
    cursors: [usize; FIELDS],
    confirm: bool,
    focus: usize,
    status: Option<(String, bool)>,
}

impl TaskForm {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            values: Default::default(),
            cursors: [0; FIELDS],
            confirm: false,
            focus: NAME,
            status: None,
        }
    }

    fn field(&mut self) -> Option<(&mut Vec<char>, &mut usize, usize)> {
        let i = self.focus;
        (i < FIELDS).then(|| (&mut self.values[i], &mut self.cursors[i], LIMITS[i]))
    }

    pub fn focus(&mut self, i: usize) -> FormOutcome {
        self.focus = i.min(CONFIRM);
        FormOutcome::Redraw
    }

    pub fn next(&mut self, back: bool) -> FormOutcome {
        self.focus = if back {
            (self.focus + CONFIRM) % (CONFIRM + 1)
        } else {
            (self.focus + 1) % (CONFIRM + 1)
        };
        FormOutcome::Redraw
    }

    /// Typed or pasted text. Line breaks end the input: a task is one line.
    pub fn type_text(&mut self, text: &str) -> FormOutcome {
        if self.focus == CONFIRM {
            if text == " " {
                self.confirm = !self.confirm;
            }
            return FormOutcome::Redraw;
        }
        let line = text.lines().next().unwrap_or("");
        if let Some((value, cursor, limit)) = self.field() {
            for c in line.chars().filter(|c| !c.is_control()) {
                if value.len() >= limit {
                    break;
                }
                value.insert(*cursor, c);
                *cursor += 1;
            }
        }
        self.status = None;
        FormOutcome::Redraw
    }

    pub fn backspace(&mut self) -> FormOutcome {
        if let Some((value, cursor, _)) = self.field() {
            if *cursor > 0 {
                value.remove(*cursor - 1);
                *cursor -= 1;
            }
        }
        FormOutcome::Redraw
    }

    pub fn delete(&mut self) -> FormOutcome {
        if let Some((value, cursor, _)) = self.field() {
            if *cursor < value.len() {
                value.remove(*cursor);
            }
        }
        FormOutcome::Redraw
    }

    /// Cmd+Backspace: clear to the start of the field.
    pub fn clear(&mut self) -> FormOutcome {
        if let Some((value, cursor, _)) = self.field() {
            value.drain(..*cursor);
            *cursor = 0;
        }
        FormOutcome::Redraw
    }

    pub fn move_cursor(&mut self, delta: isize) -> FormOutcome {
        if let Some((value, cursor, _)) = self.field() {
            *cursor = cursor.saturating_add_signed(delta).min(value.len());
        }
        FormOutcome::Redraw
    }

    pub fn cursor_to_edge(&mut self, end: bool) -> FormOutcome {
        if let Some((value, cursor, _)) = self.field() {
            *cursor = if end { value.len() } else { 0 };
        }
        FormOutcome::Redraw
    }

    pub fn toggle_confirm(&mut self) -> FormOutcome {
        self.confirm = !self.confirm;
        self.focus = CONFIRM;
        FormOutcome::Redraw
    }

    /// Enter: toggles the checkbox when it has focus, otherwise saves.
    pub fn enter(&mut self) -> FormOutcome {
        if self.focus == CONFIRM {
            self.toggle_confirm()
        } else {
            self.save()
        }
    }

    pub fn save(&mut self) -> FormOutcome {
        let text = |i: usize| self.values[i].iter().collect::<String>().trim().to_string();
        let (name, run, cwd) = (text(NAME), text(RUN), text(CWD));
        let problem = if name.is_empty() {
            Some((NAME, "Give the task a name."))
        } else if run.is_empty() {
            Some((RUN, "Enter the command to run."))
        } else {
            None
        };
        if let Some((field, message)) = problem {
            self.focus = field;
            self.status = Some((message.into(), true));
            return FormOutcome::Redraw;
        }
        FormOutcome::Save(TaskDef {
            name,
            run,
            cwd: (!cwd.is_empty() && cwd != ".").then_some(cwd),
            confirm: self.confirm,
        })
    }

    /// The caller couldn't save; show why and keep the form open.
    pub fn failed(&mut self, message: String) {
        self.status = Some((message, true));
    }

    pub fn view(&self) -> TaskFormView {
        let field = |i: usize, label: &str, placeholder: &str| FormField {
            label: label.into(),
            value: self.values[i].iter().collect(),
            placeholder: placeholder.into(),
            cursor: (self.focus == i).then_some(self.cursors[i]),
        };
        TaskFormView {
            title: "Add task".into(),
            project: volt_config::tasks::tasks_file(&self.root)
                .display()
                .to_string(),
            fields: vec![
                field(NAME, "Name", "Build"),
                field(RUN, "Command", "cargo build --release"),
                field(CWD, "Folder (inside the project)", "Project folder"),
            ],
            confirm: self.confirm,
            focus: self.focus,
            status: self.status.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form() -> TaskForm {
        TaskForm::new("/w/app".into())
    }

    #[test]
    fn typing_fills_the_focused_field_and_tab_moves_on() {
        let mut f = form();
        f.type_text("Build");
        f.next(false);
        f.type_text("cargo build --release");
        f.next(false);
        f.type_text("volt");
        f.next(false);
        f.type_text(" ");
        assert_eq!(
            f.enter(),
            FormOutcome::Redraw,
            "Enter on the checkbox toggles it"
        );
        f.toggle_confirm();
        assert_eq!(
            f.save(),
            FormOutcome::Save(TaskDef {
                name: "Build".into(),
                run: "cargo build --release".into(),
                cwd: Some("volt".into()),
                confirm: true,
            })
        );
        f.next(false);
        assert_eq!(f.view().focus, NAME, "Tab wraps around");
        f.next(true);
        assert_eq!(f.view().focus, CONFIRM, "Shift+Tab wraps back");
    }

    #[test]
    fn missing_fields_are_reported_in_place() {
        let mut f = form();
        assert_eq!(f.save(), FormOutcome::Redraw);
        assert_eq!(f.view().status.unwrap().0, "Give the task a name.");
        f.type_text("Build");
        f.save();
        assert_eq!(f.view().focus, RUN);
        assert!(f.view().status.unwrap().1);
    }

    #[test]
    fn pasted_text_stays_on_one_line_and_editing_keys_work() {
        let mut f = form();
        f.focus(RUN);
        f.type_text("make all\nrm -rf ~");
        assert_eq!(f.view().fields[RUN].value, "make all");
        f.move_cursor(-3);
        f.type_text("-j8 ");
        assert_eq!(f.view().fields[RUN].value, "make -j8 all");
        f.cursor_to_edge(true);
        f.backspace();
        assert_eq!(f.view().fields[RUN].value, "make -j8 al");
        f.cursor_to_edge(false);
        f.delete();
        f.cursor_to_edge(true);
        f.clear();
        assert_eq!(f.view().fields[RUN].value, "");
        // An empty folder means the project folder.
        f.focus(NAME);
        f.type_text("X");
        f.focus(RUN);
        f.type_text("y");
        f.focus(CWD);
        f.type_text(" . ");
        assert!(matches!(
            f.save(),
            FormOutcome::Save(TaskDef { cwd: None, .. })
        ));
    }
}
