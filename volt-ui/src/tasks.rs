//! Project tasks in the UI: loading `.volt/tasks.toml` off the render
//! thread, the command line typed for a task, and a task's run status.
use crate::app::VoltEvent;
use crate::editor::shell_quote;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use volt_config::tasks::{ProjectTasks, TaskDef};
use winit::event_loop::EventLoopProxy;

/// A task's progress in the pane it was typed into. With shell integration
/// (OSC 133) the shell reports start and exit; without it a run stays `Sent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Sent,
    Running,
    Finished(i32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRun {
    pub name: String,
    pub root: PathBuf,
    pub state: RunState,
}

impl TaskRun {
    pub fn started(&mut self) {
        if self.state == RunState::Sent {
            self.state = RunState::Running;
        }
    }
    pub fn finished(&mut self, exit_code: i32) {
        if self.state == RunState::Running {
            self.state = RunState::Finished(exit_code);
        }
    }
}

/// The line to type for `task`. In the task's own folder it is the command
/// as written; elsewhere it runs in a subshell so the shell's folder is kept.
pub fn command_line(
    tasks: &ProjectTasks,
    task: &TaskDef,
    shell: &str,
    here: Option<&Path>,
) -> String {
    let dir = match task.cwd.as_deref() {
        Some(sub) => tasks.root.join(sub),
        None => tasks.root.clone(),
    };
    if here.is_some_and(|here| here == dir) {
        return task.run.clone();
    }
    let dir = shell_quote(&dir.to_string_lossy());
    let program = shell.rsplit('/').next().unwrap_or(shell);
    if program == "fish" {
        // fish has no ( … ) subshell; run it in a child fish instead.
        format!(
            "fish -c {}",
            shell_quote(&format!("cd {dir}; and {}", task.run))
        )
    } else {
        format!("(cd {dir} && {})", task.run)
    }
}

/// Keeps the active pane's project tasks fresh: re-read when the folder
/// changes, and every couple of seconds so edits and pulls show up.
#[derive(Default)]
pub struct TaskLoader {
    pub tasks: Option<ProjectTasks>,
    cwd: Option<PathBuf>,
    pending: Option<mpsc::Receiver<(Option<PathBuf>, Option<ProjectTasks>)>>,
    next_check: Option<Instant>,
    next_cwd_check: Option<Instant>,
}

impl TaskLoader {
    /// Re-read right away, e.g. after Volt saved or trusted the file.
    pub fn refresh(&mut self) {
        self.next_check = None;
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.next_check
    }

    /// Returns true when the loaded tasks changed. `cwd` is only called a
    /// few times a second: it asks the OS for the shell's folder.
    pub fn tick(
        &mut self,
        cwd: impl FnOnce() -> Option<PathBuf>,
        proxy: &EventLoopProxy<VoltEvent>,
    ) -> bool {
        let mut changed = false;
        let cwd = if self.next_cwd_check.is_none_or(|t| Instant::now() >= t) {
            self.next_cwd_check = Some(Instant::now() + Duration::from_millis(250));
            cwd()
        } else {
            self.cwd.clone()
        };
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok((for_cwd, tasks)) => {
                    self.pending = None;
                    if for_cwd == self.cwd && tasks != self.tasks {
                        self.tasks = tasks;
                        changed = true;
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        let moved = cwd != self.cwd;
        if moved {
            self.cwd = cwd.clone();
            self.next_check = None;
            if self.tasks.take().is_some() {
                changed = true;
            }
        }
        let due = self.next_check.is_none_or(|t| Instant::now() >= t);
        if self.pending.is_none() && due {
            self.next_check = Some(Instant::now() + Duration::from_secs(2));
            if let Some(cwd) = cwd {
                let (tx, rx) = mpsc::channel();
                self.pending = Some(rx);
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let home = dirs::home_dir();
                    let tasks =
                        volt_config::tasks::project_root(&cwd, home.as_deref()).map(|root| {
                            volt_config::tasks::load(
                                &root,
                                volt_config::tasks::trust_dir().as_deref(),
                            )
                        });
                    let _ = tx.send((Some(cwd), tasks));
                    let _ = proxy.send_event(VoltEvent::WorkspaceUpdated);
                });
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tasks(root: &str) -> ProjectTasks {
        ProjectTasks {
            root: root.into(),
            ..Default::default()
        }
    }

    fn task(run: &str, cwd: Option<&str>) -> TaskDef {
        TaskDef {
            name: "T".into(),
            run: run.into(),
            cwd: cwd.map(Into::into),
            confirm: false,
        }
    }

    #[test]
    fn tasks_run_as_written_in_their_folder_and_in_a_subshell_elsewhere() {
        let t = tasks("/w/app");
        let build = task("cargo build", None);
        assert_eq!(
            command_line(&t, &build, "/bin/zsh", Some(Path::new("/w/app"))),
            "cargo build"
        );
        assert_eq!(
            command_line(&t, &build, "/bin/zsh", Some(Path::new("/w/app/src"))),
            "(cd '/w/app' && cargo build)"
        );
        let sub = task("npm test", Some("web"));
        assert_eq!(
            command_line(&t, &sub, "/bin/bash", None),
            "(cd '/w/app/web' && npm test)"
        );
        assert_eq!(
            command_line(&t, &sub, "/opt/homebrew/bin/fish", None),
            r"fish -c 'cd '\''/w/app/web'\''; and npm test'"
        );
    }

    #[test]
    fn status_only_moves_forward_with_shell_reports() {
        let mut run = TaskRun {
            name: "Build".into(),
            root: "/w".into(),
            state: RunState::Sent,
        };
        run.finished(1);
        assert_eq!(
            run.state,
            RunState::Sent,
            "no start seen: no status claimed"
        );
        run.started();
        run.finished(2);
        assert_eq!(run.state, RunState::Finished(2));
        run.started();
        assert_eq!(run.state, RunState::Finished(2));
    }
}
