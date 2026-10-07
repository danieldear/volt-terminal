//! Project tasks in the UI: loading `.volt/tasks.toml` off the render
//! thread, the command line typed for a task, and a task's run status.
use crate::app::VoltEvent;
use crate::editor::shell_quote;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use volt_config::tasks::{ProjectTasks, TaskDef};
use winit::event_loop::EventLoopProxy;

/// Confirmation is bound to the reviewed file and the selected terminal.
/// It does not authorize any other command or persist file-wide trust.
pub struct PendingTask {
    pub index: usize,
    pub tasks: ProjectTasks,
    pub pane_pid: Option<u32>,
}

impl PendingTask {
    pub fn applies_to(&self, tasks: Option<&ProjectTasks>, pane_pid: Option<u32>) -> bool {
        self.pane_pid.is_some()
            && self.pane_pid == pane_pid
            && tasks == Some(&self.tasks)
            && self.tasks.tasks.get(self.index).is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchGate {
    Direct,
    Confirm,
    ReviewOnce,
}

pub fn launch_gate(tasks: &ProjectTasks, task: &TaskDef) -> LaunchGate {
    if !tasks.trusted {
        LaunchGate::ReviewOnce
    } else if task.confirm {
        LaunchGate::Confirm
    } else {
        LaunchGate::Direct
    }
}

/// Turn a manifest-discovered command into a reviewed project task. Discovery
/// is never execution: the user must explicitly add it, then run it later.
/// Existing commands are not suggested again, even if their names differ.
pub fn detected_task(
    source: &crate::workspace_project::ProjectTask,
    saved: &ProjectTasks,
) -> Result<Option<TaskDef>, String> {
    let relative = source
        .cwd
        .strip_prefix(&saved.root)
        .map_err(|_| "The detected task is outside this project's tasks folder.".to_string())?;
    if source.argv.is_empty()
        || source
            .argv
            .iter()
            .any(|arg| arg.chars().any(char::is_control))
    {
        return Err("The detected command cannot be saved safely.".into());
    }
    let run = source
        .argv
        .iter()
        .map(|arg| {
            if !arg.is_empty()
                && arg
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_./+@%=-".contains(c))
            {
                arg.clone()
            } else {
                shell_quote(arg)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    let cwd = if relative.as_os_str().is_empty() {
        None
    } else {
        Some(
            relative
                .to_str()
                .ok_or("The detected task folder is not valid UTF-8.")?
                .to_string(),
        )
    };
    if saved
        .tasks
        .iter()
        .any(|task| task.run == run && task.cwd == cwd)
    {
        return Ok(None);
    }
    if run.chars().count() > volt_config::tasks::MAX_RUN_CHARS {
        return Err("The detected command is too long for a project task.".into());
    }
    let base = source.label.trim();
    if base.is_empty() {
        return Err("The detected task has no name.".into());
    }
    let limit = volt_config::tasks::MAX_NAME_CHARS;
    let mut name: String = base.chars().take(limit).collect();
    for number in 2..=volt_config::tasks::MAX_TASKS + 2 {
        if !saved
            .tasks
            .iter()
            .any(|task| task.name.eq_ignore_ascii_case(&name))
        {
            break;
        }
        let suffix = format!(" {number}");
        name = base
            .chars()
            .take(limit.saturating_sub(suffix.len()))
            .collect();
        name.push_str(&suffix);
    }
    Ok(Some(TaskDef {
        name,
        run,
        cwd,
        // Manifest scripts may change independently of .volt/tasks.toml.
        confirm: true,
    }))
}

pub fn suggested_tasks(
    discovered: &[crate::workspace_project::ProjectTask],
    saved: &ProjectTasks,
) -> Vec<(usize, TaskDef)> {
    discovered
        .iter()
        .enumerate()
        .filter_map(|(i, task)| detected_task(task, saved).ok().flatten().map(|t| (i, t)))
        .collect()
}

/// A task's progress in the pane it was typed into. With shell integration
/// (OSC 133) the shell reports start and exit; without it a run stays `Sent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Sent,
    Running,
    Finished(i32),
    /// The foreground job ended, but no shell-reported exit code was available.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TaskRun {
    pub name: String,
    pub root: PathBuf,
    pub command: String,
    pub cwd: Option<String>,
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
    pub fn observe_foreground(&mut self, running: bool) {
        if running {
            self.started();
        } else if self.state == RunState::Running {
            self.state = RunState::Unknown;
        }
    }
}

/// Retain a bounded, most-recent result per task across subsequent launches.
pub fn remember_run(history: &mut Vec<TaskRun>, run: TaskRun) {
    history.retain(|old| old.name != run.name || old.root != run.root);
    if history.len() >= volt_config::tasks::MAX_TASKS {
        history.remove(0);
    }
    history.push(run);
}

pub fn find_run<'a>(
    current: Option<&'a TaskRun>,
    history: &'a [TaskRun],
    root: &Path,
    task: &TaskDef,
) -> Option<&'a TaskRun> {
    let matches = |r: &&TaskRun| {
        r.root == root && r.name == task.name && r.command == task.run && r.cwd == task.cwd
    };
    current
        .filter(matches)
        .or_else(|| history.iter().rev().find(matches))
}

pub fn state_label(state: RunState) -> &'static str {
    match state {
        RunState::Sent => "Sent",
        RunState::Unknown | RunState::Finished(-1) => "Exit unknown",
        RunState::Running => "Running…",
        RunState::Finished(0) => "✓ Done",
        RunState::Finished(_) => "✗ Failed",
    }
}

/// Execution history and file trust are independent: a reviewed run-once
/// task still shows its real result, while the next click remains trust-gated.
pub fn strip_state(run: Option<RunState>, trusted: bool) -> volt_renderer::task_strip::StripState {
    use volt_renderer::task_strip::StripState;
    match run {
        Some(RunState::Sent | RunState::Unknown | RunState::Finished(-1)) => StripState::Sent,
        Some(RunState::Running) => StripState::Running,
        Some(RunState::Finished(0)) => StripState::Succeeded,
        Some(RunState::Finished(_)) => StripState::Failed,
        None if !trusted => StripState::ReviewRequired,
        None => StripState::Idle,
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
    /// Cached off-thread: show first-task creation in real projects, not /tmp.
    pub project_context: bool,
    cwd: Option<PathBuf>,
    pending: Option<mpsc::Receiver<LoadedTasks>>,
    next_check: Option<Instant>,
    next_cwd_check: Option<Instant>,
}

struct LoadedTasks {
    cwd: Option<PathBuf>,
    tasks: Option<ProjectTasks>,
    project_context: bool,
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
                Ok(loaded) => {
                    self.pending = None;
                    if loaded.cwd == self.cwd
                        && (loaded.tasks != self.tasks
                            || loaded.project_context != self.project_context)
                    {
                        self.tasks = loaded.tasks;
                        self.project_context = loaded.project_context;
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
            self.project_context = false;
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
                    let project_context = tasks.as_ref().is_some_and(|tasks| {
                        tasks.source_bytes.is_some()
                            || tasks.root.join(".git").exists()
                            || crate::workspace_project::marked(&tasks.root)
                    });
                    let _ = tx.send(LoadedTasks {
                        cwd: Some(cwd),
                        tasks,
                        project_context,
                    });
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
    use crate::workspace_project::ProjectTask;

    #[test]
    fn own_trusted_tasks_run_directly_but_untrusted_tasks_only_offer_run_once() {
        let task = TaskDef {
            name: "build".into(),
            run: "cargo build --release".into(),
            ..Default::default()
        };
        let mut saved = tasks("/repo");
        assert_eq!(launch_gate(&saved, &task), LaunchGate::ReviewOnce);
        saved.trusted = true;
        assert_eq!(launch_gate(&saved, &task), LaunchGate::Direct);
        let confirm = TaskDef {
            confirm: true,
            ..task.clone()
        };
        assert_eq!(launch_gate(&saved, &confirm), LaunchGate::Confirm);
        saved.trusted = false;
        assert_eq!(launch_gate(&saved, &confirm), LaunchGate::ReviewOnce);
    }

    #[test]
    fn run_once_confirmation_is_invalidated_by_any_file_or_pane_change() {
        let mut saved = tasks("/repo");
        saved.tasks.push(task("printf SAFE", None));
        saved.source_bytes = Some(std::sync::Arc::from(&b"reviewed file"[..]));
        let pending = PendingTask {
            index: 0,
            tasks: saved.clone(),
            pane_pid: Some(123),
        };
        assert!(pending.applies_to(Some(&saved), Some(123)));
        assert!(!saved.trusted, "run-once permission must not trust a file");
        assert!(!pending.applies_to(Some(&saved), Some(456)));
        assert!(!pending.applies_to(Some(&saved), None));
        assert!(!pending.applies_to(None, Some(123)));
        let mut changed = saved.clone();
        changed.source_bytes = Some(std::sync::Arc::from(&b"edited file"[..]));
        assert!(!pending.applies_to(Some(&changed), Some(123)));
        changed = saved.clone();
        changed.tasks[0].run = "printf OTHER".into();
        assert!(!pending.applies_to(Some(&changed), Some(123)));
        changed = saved;
        changed.root = "/other".into();
        assert!(!pending.applies_to(Some(&changed), Some(123)));
    }

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

    fn detected(label: &str, argv: &[&str], cwd: &str) -> ProjectTask {
        ProjectTask {
            label: label.into(),
            argv: argv.iter().map(|s| (*s).into()).collect(),
            cwd: cwd.into(),
            definition: "manifest".into(),
        }
    }

    #[test]
    fn detected_commands_import_with_safe_arguments_and_do_not_repeat() {
        let mut saved = tasks("/w/app");
        let build = detected("cargo build", &["cargo", "build"], "/w/app");
        let candidate = detected_task(&build, &saved).unwrap().unwrap();
        assert_eq!(candidate.name, "cargo build");
        assert_eq!(candidate.run, "cargo build");
        assert_eq!(candidate.cwd, None);
        assert!(candidate.confirm);
        saved.tasks.push(candidate);
        assert!(detected_task(&build, &saved).unwrap().is_none());

        let script = detected(
            "npm run dev",
            &["npm", "run", "$(touch INJECTED); echo bad"],
            "/w/app/web",
        );
        let imported = detected_task(&script, &saved).unwrap().unwrap();
        assert_eq!(imported.cwd.as_deref(), Some("web"));
        assert_eq!(imported.run, "npm run '$(touch INJECTED); echo bad'");
        assert!(detected_task(&script, &tasks("/elsewhere")).is_err());
    }

    #[test]
    fn detected_names_do_not_collide_with_user_tasks() {
        let mut saved = tasks("/w/app");
        saved.tasks.push(TaskDef {
            name: "cargo build".into(),
            run: "cargo build --release".into(),
            ..Default::default()
        });
        let build = detected("cargo build", &["cargo", "build"], "/w/app");
        assert_eq!(
            detected_task(&build, &saved).unwrap().unwrap().name,
            "cargo build 2"
        );
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
    fn last_result_survives_starting_a_different_task_and_history_is_bounded() {
        let mut history = vec![];
        let build_def = TaskDef {
            name: "Build".into(),
            run: "true".into(),
            ..Default::default()
        };
        let test_def = TaskDef {
            name: "Test".into(),
            ..build_def.clone()
        };
        let build = TaskRun {
            name: "Build".into(),
            root: "/w".into(),
            command: "true".into(),
            cwd: None,
            state: RunState::Finished(0),
        };
        remember_run(&mut history, build.clone());
        let test = TaskRun {
            name: "Test".into(),
            state: RunState::Running,
            ..build.clone()
        };
        assert_eq!(
            find_run(Some(&test), &history, Path::new("/w"), &build_def)
                .unwrap()
                .state,
            RunState::Finished(0)
        );
        assert_eq!(
            find_run(Some(&test), &history, Path::new("/w"), &test_def)
                .unwrap()
                .state,
            RunState::Running
        );
        assert!(find_run(Some(&test), &history, Path::new("/other"), &build_def).is_none());
        remember_run(
            &mut history,
            TaskRun {
                state: RunState::Finished(1),
                ..build.clone()
            },
        );
        assert_eq!(history.len(), 1);
        let changed_command = TaskDef {
            run: "false".into(),
            ..build_def.clone()
        };
        let changed_cwd = TaskDef {
            cwd: Some("subdir".into()),
            ..build_def.clone()
        };
        assert!(find_run(Some(&test), &history, Path::new("/w"), &changed_command).is_none());
        assert!(find_run(Some(&test), &history, Path::new("/w"), &changed_cwd).is_none());
        for i in 0..100 {
            remember_run(
                &mut history,
                TaskRun {
                    name: format!("Task {i}"),
                    ..build.clone()
                },
            );
        }
        assert_eq!(history.len(), volt_config::tasks::MAX_TASKS);
    }

    #[test]
    fn run_once_results_are_visible_without_granting_file_trust() {
        use volt_renderer::task_strip::StripState;
        assert_eq!(
            strip_state(Some(RunState::Finished(0)), false),
            StripState::Succeeded
        );
        assert_eq!(
            strip_state(Some(RunState::Finished(2)), false),
            StripState::Failed
        );
        assert_eq!(
            strip_state(Some(RunState::Finished(-1)), false),
            StripState::Sent
        );
        assert_eq!(strip_state(Some(RunState::Sent), true), StripState::Sent);
        assert_eq!(strip_state(None, false), StripState::ReviewRequired);
        assert_eq!(
            launch_gate(&tasks("/w"), &task("true", None)),
            LaunchGate::ReviewOnce
        );
    }

    #[test]
    fn unintegrated_completion_never_claims_success() {
        let mut run = TaskRun {
            name: "test".into(),
            root: "/w".into(),
            command: "true".into(),
            cwd: None,
            state: RunState::Sent,
        };
        run.observe_foreground(true);
        assert_eq!(run.state, RunState::Running);
        run.observe_foreground(false);
        assert_eq!(run.state, RunState::Unknown);
        run.started();
        run.finished(0);
        assert_eq!(run.state, RunState::Unknown);
    }

    #[test]
    fn status_only_moves_forward_with_shell_reports() {
        let mut run = TaskRun {
            name: "Build".into(),
            root: "/w".into(),
            command: "true".into(),
            cwd: None,
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
