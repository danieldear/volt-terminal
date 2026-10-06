//! Workspace inspector state and bounded, off-thread discovery.

use crate::app::VoltEvent;
use crate::{
    workspace_git::{self, GitDetails, PrState},
    workspace_project::{self, ConfigFile, Project, ProjectTask},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};
use volt_renderer::workspace_card::{CardIcon, CardRow, CardTone, WorkspaceCard};
use winit::event_loop::EventLoopProxy;

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub root: PathBuf,
    pub branch: String,
    pub repository: bool,
    pub diff: Option<(u64, u64)>,
    pub files: Vec<(String, String)>,
    pub configs: Vec<String>,
    pub error: Option<String>,
    pub rust: bool,
    pub agents: Vec<(String, String)>,
    pub mcp_names: Vec<String>,
    pub project: Project,
    pub config_files: Vec<ConfigFile>,
    pub git_details: GitDetails,
}
pub struct WorkspacePanel {
    /// User enablement; irrelevant directories temporarily have no rendered card.
    pub visible: bool,
    pub minimized: bool,
    pub position: Option<[f32; 2]>,
    pub drag_offset: Option<[f32; 2]>,
    pub focused: bool,
    pub hover: Option<usize>,
    pub expanded: Option<usize>,
    pub selected_file: Option<usize>,
    pub scroll: usize,
    pub snapshot: Option<Snapshot>,
    pub selected_task: Option<ProjectTask>,
    /// The project's own tasks (`.volt/tasks.toml`), loaded by the app.
    pub custom_tasks: Option<volt_config::tasks::ProjectTasks>,
    /// The last task typed into the active pane, for its ✓ / ✗ status.
    pub task_run: Option<crate::tasks::TaskRun>,
    /// Why the last task action didn't happen.
    pub task_message: Option<String>,
    pub selected_item: Option<usize>,
    pub pull_request: Option<PrState>,
    pr_key: Option<String>,
    pr_pending: Option<mpsc::Receiver<(String, PrState)>>,
    next_pr_refresh: Instant,
    cwd: Option<PathBuf>,
    pid: Option<u32>,
    pending: Option<mpsc::Receiver<(u64, Snapshot)>>,
    scope_revision: u64,
    reported_cwd: Option<PathBuf>,
    next_cwd_check: Instant,
    pub next_refresh: Instant,
}
impl Default for WorkspacePanel {
    fn default() -> Self {
        Self {
            visible: false,
            minimized: false,
            position: None,
            drag_offset: None,
            focused: false,
            hover: None,
            expanded: None,
            selected_file: None,
            scroll: 0,
            snapshot: None,
            selected_task: None,
            custom_tasks: None,
            task_run: None,
            task_message: None,
            selected_item: None,
            pull_request: None,
            pr_key: None,
            pr_pending: None,
            next_pr_refresh: Instant::now(),
            cwd: None,
            pid: None,
            pending: None,
            scope_revision: 0,
            reported_cwd: None,
            next_cwd_check: Instant::now(),
            next_refresh: Instant::now(),
        }
    }
}
impl WorkspacePanel {
    pub fn activate(&mut self, id: usize) {
        self.focused = true;
        match id {
            100 => {
                self.next_refresh = Instant::now();
                self.next_pr_refresh = Instant::now();
            }
            102 => {
                self.minimized = !self.minimized;
                self.hover = Some(102);
            }
            101 => {
                self.visible = false;
                self.focused = false;
            }
            1000..=1031 => {
                let task = self
                    .snapshot
                    .as_ref()
                    .and_then(|s| s.project.tasks.get(id - 1000))
                    .cloned();
                self.selected_task = if task == self.selected_task {
                    None
                } else {
                    task
                };
            }
            2000..=2063 | 4000..=4063 => {
                self.selected_item = if self.selected_item == Some(id) {
                    None
                } else {
                    Some(id)
                };
            }
            200..=299 => {
                let i = id - 200;
                self.selected_file = if self.selected_file == Some(i) {
                    None
                } else {
                    Some(i)
                };
            }
            _ => {
                self.expanded = if self.expanded == Some(id) {
                    None
                } else {
                    Some(id)
                };
                self.scroll = 0;
                self.selected_file = None;
            }
        }
        // Bring newly disclosed content into view instead of expanding below
        // the fold at the bottom of a short window.
        if self.expanded == Some(id)
            || self.selected_item == Some(id)
            || ((1000..1032).contains(&id) && self.selected_task.is_some())
            || ((200..300).contains(&id) && self.selected_file == Some(id - 200))
        {
            if let Some(row) = self.card().rows.iter().position(|r| r.action == Some(id)) {
                self.scroll = row.saturating_sub(1);
            }
        }
    }
    pub(crate) fn search_snapshot(
        &self,
        cwd: &Option<PathBuf>,
        pid: Option<u32>,
    ) -> Option<&Snapshot> {
        (self.cwd == *cwd && self.pid == pid)
            .then_some(self.snapshot.as_ref())
            .flatten()
    }
    // Borrow on the idle path: cloning a winit macOS proxy registers a new
    // CFRunLoopSource and wakes the loop. Clone only when launching a worker.
    pub fn tick(
        &mut self,
        cwd: Option<PathBuf>,
        pid: Option<u32>,
        proxy: &EventLoopProxy<VoltEvent>,
    ) -> bool {
        if !self.visible {
            return false;
        }
        let mut changed = false;
        // Native cwd checks are cheap, bounded syscalls; do not run Git/network
        // discovery on the UI thread or wait for its slower refresh interval.
        if self.pid != pid || self.reported_cwd != cwd || Instant::now() >= self.next_cwd_check {
            let resolved = pid.and_then(local_process_cwd).or_else(|| cwd.clone());
            self.reported_cwd = cwd;
            self.next_cwd_check = Instant::now() + Duration::from_millis(250);
            changed |= self.update_scope(resolved, pid);
        }
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok((revision, snapshot)) => {
                    changed |= self.accept_snapshot(revision, snapshot);
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                }
                _ => {}
            }
        }
        if self.pending.is_none() && Instant::now() >= self.next_refresh {
            if let Some(path) = self.cwd.clone() {
                let (tx, rx) = mpsc::channel();
                self.pending = Some(rx);
                let revision = self.scope_revision;
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let snapshot = collect(&path);
                    let _ = tx.send((revision, snapshot));
                    let _ = proxy.send_event(VoltEvent::WorkspaceUpdated);
                });
                changed = true;
            }
            self.next_refresh =
                Instant::now() + Duration::from_secs(if self.minimized { 15 } else { 5 });
        }
        // GitHub is a separate, slow lane. Network latency never delays local Git/project facts.
        if let Some(rx) = &self.pr_pending {
            match rx.try_recv() {
                Ok((key, result)) => {
                    if self.pr_key.as_ref() == Some(&key) {
                        self.pull_request = Some(result);
                        changed = true;
                    }
                    self.pr_pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => self.pr_pending = None,
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.pr_pending.is_none() && Instant::now() >= self.next_pr_refresh {
            if let (Some(key), Some(snapshot)) = (self.pr_key.clone(), self.snapshot.as_ref()) {
                let root = snapshot.root.clone();
                let (tx, rx) = mpsc::channel();
                self.pr_pending = Some(rx);
                let proxy = proxy.clone();
                std::thread::spawn(move || {
                    let result = workspace_git::collect_pr(&root);
                    let _ = tx.send((key, result));
                    let _ = proxy.send_event(VoltEvent::WorkspaceUpdated);
                });
            }
            self.next_pr_refresh = Instant::now() + Duration::from_secs(60);
        }
        changed
    }
    pub fn refresh_deadline(&self) -> Option<Instant> {
        self.visible.then_some(if self.pending.is_some() {
            self.next_cwd_check
        } else {
            self.next_cwd_check.min(self.next_refresh)
        })
    }
    fn update_scope(&mut self, cwd: Option<PathBuf>, pid: Option<u32>) -> bool {
        if self.cwd == cwd && self.pid == pid {
            return false;
        }
        self.cwd = cwd;
        self.pid = pid;
        self.scope_revision = self.scope_revision.wrapping_add(1);
        self.snapshot = None;
        self.selected_task = None;
        self.selected_item = None;
        self.pull_request = None;
        self.pr_key = None;
        self.next_pr_refresh = Instant::now();
        self.selected_file = None;
        self.scroll = 0;
        self.next_refresh = Instant::now();
        true
    }

    fn accept_snapshot(&mut self, revision: u64, snapshot: Snapshot) -> bool {
        // Also reject A -> B -> A races, even though the final path matches.
        if revision != self.scope_revision {
            return false;
        }
        if let Some(old) = &self.snapshot {
            if old.files != snapshot.files {
                self.selected_file = None;
            }
        }
        let key = (snapshot.git_details.github && workspace_git::gh_installed()).then(|| {
            format!(
                "{}:{}:{}:{}",
                self.scope_revision,
                snapshot.root.display(),
                snapshot.branch,
                snapshot.git_details.head
            )
        });
        if self.pr_key != key {
            self.pr_key = key;
            self.pull_request = None;
            self.next_pr_refresh = Instant::now();
        }
        if self
            .selected_task
            .as_ref()
            .is_some_and(|task| !snapshot.project.tasks.contains(task))
        {
            self.selected_task = None;
        }
        if self.snapshot.as_ref().is_some_and(|old| {
            old.config_files != snapshot.config_files
                || old.git_details.branches.iter().map(|b| &b.name).ne(snapshot
                    .git_details
                    .branches
                    .iter()
                    .map(|b| &b.name))
        }) {
            self.selected_item = None;
        }
        self.snapshot = Some(snapshot);
        true
    }

    /// Keep enablement separate from relevance so returning to a project restores
    /// the card, but closing it explicitly does not.
    pub fn presentation_card(&self) -> Option<WorkspaceCard> {
        (self.visible
            && self
                .snapshot
                .as_ref()
                .is_some_and(|s| (0..8).any(|id| section_available(id, s))))
        .then(|| self.card())
    }

    pub fn card(&self) -> WorkspaceCard {
        let s = self.snapshot.as_ref();
        let title = self
            .cwd
            .as_ref()
            .or_else(|| s.map(|s| &s.root))
            .map(|p| p.file_name().unwrap_or(p.as_os_str()))
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Workspace".into());
        let (subtitle, status, tone) =
            if let Some(s) = s {
                if s.error.is_some() {
                    (
                        if s.repository {
                            s.branch.clone()
                        } else {
                            String::new()
                        },
                        "Unavailable",
                        CardTone::Red,
                    )
                } else if !s.repository {
                    (
                        if s.rust {
                            "Rust project"
                        } else {
                            "AI configuration"
                        }
                        .into(),
                        "",
                        CardTone::Blue,
                    )
                } else if s.files.iter().any(|(status, _)| {
                    status.contains('U') || matches!(status.as_str(), "AA" | "DD")
                }) {
                    (s.branch.clone(), "Conflict", CardTone::Red)
                } else if s.files.is_empty() {
                    (s.branch.clone(), "Clean", CardTone::Green)
                } else {
                    (s.branch.clone(), "Changed", CardTone::Amber)
                }
            } else {
                ("Reading workspace…".into(), "", CardTone::Muted)
            };
        let mut card = WorkspaceCard {
            title: safe_label(&title, 14),
            subtitle: safe_label(
                &s.filter(|s| !s.project.tags.is_empty())
                    .map(|s| s.project.tags.join(" · "))
                    .unwrap_or(subtitle),
                27,
            ),
            status: status.into(),
            tone,
            minimized: self.minimized,
            hover: self.hover,
            focused: self.focused,
            scroll: self.scroll,
            ..Default::default()
        };
        let mut row = |label: &str, detail: &str, icon, action, section, expanded| {
            card.rows.push(CardRow {
                label: safe_label(
                    label,
                    if detail.is_empty() {
                        27
                    } else {
                        27usize.saturating_sub(
                            unicode_width::UnicodeWidthStr::width(detail).min(14) + 2,
                        )
                    },
                ),
                detail: safe_label(detail, 14),
                icon,
                action,
                section,
                expanded,
                tone: CardTone::Muted,
                diff: None,
            })
        };
        for (id, label, icon) in [
            (0, "Changes", CardIcon::Branch),
            (5, "Project", CardIcon::Folder),
            (6, "Branches", CardIcon::Branch),
            (7, "Worktrees", CardIcon::Folder),
            (8, "Pull request", CardIcon::Link),
            (1, "Tasks", CardIcon::Play),
            (2, "Agent", CardIcon::Agent),
            (3, "Rules & context", CardIcon::File),
            (4, "Tools / MCP", CardIcon::Link),
        ] {
            let custom_tasks = self
                .custom_tasks
                .as_ref()
                .is_some_and(|t| !t.tasks.is_empty() || t.problem.is_some());
            if if id == 8 {
                !matches!(self.pull_request, Some(PrState::Found(_)))
            } else if id == 1 && custom_tasks {
                false
            } else {
                !s.is_some_and(|s| section_available(id, s))
            } {
                continue;
            }
            let expanded = self.expanded == Some(id);
            let detail = match id {
                0 => s
                    .map(|s| {
                        if s.error.is_some() {
                            "Unavailable".into()
                        } else {
                            format!("{} files", s.files.len())
                        }
                    })
                    .unwrap_or_default(),
                1 => self
                    .custom_tasks
                    .as_ref()
                    .filter(|t| !t.tasks.is_empty())
                    .map(|t| t.tasks.len().to_string())
                    .unwrap_or_default(),
                2 => "Not connected".into(),
                6 => s.map(|s| s.branch.clone()).unwrap_or_default(),
                7 => s
                    .map(|s| s.git_details.worktrees.len().to_string())
                    .unwrap_or_default(),
                8 => match &self.pull_request {
                    Some(PrState::Found(pr)) => format!("#{} {}", pr.number, pr.state),
                    _ => String::new(),
                },
                _ => String::new(),
            };
            row(label, &detail, icon, Some(id), true, expanded);
            if !expanded {
                continue;
            }
            match id {
                0 => {
                    if let Some(s) = s {
                        if s.files.is_empty() {
                            row(
                                if s.error.is_some() {
                                    "Repository unavailable"
                                } else {
                                    "Working tree clean"
                                },
                                "",
                                CardIcon::Folder,
                                None,
                                false,
                                false,
                            );
                        }
                        for (i, (status, path)) in s.files.iter().take(100).enumerate() {
                            row(
                                &safe_label(path, 22),
                                status,
                                CardIcon::File,
                                Some(200 + i),
                                false,
                                self.selected_file == Some(i),
                            );
                            if self.selected_file == Some(i) {
                                row(
                                    &format!("Index: {}", status_name(status.as_bytes()[0])),
                                    "",
                                    CardIcon::File,
                                    None,
                                    false,
                                    false,
                                );
                                row(
                                    &format!("Worktree: {}", status_name(status.as_bytes()[1])),
                                    "",
                                    CardIcon::File,
                                    None,
                                    false,
                                    false,
                                );
                            }
                        }
                        if s.files.len() > 100 {
                            row(
                                "First 100 changes shown",
                                "",
                                CardIcon::File,
                                None,
                                false,
                                false,
                            );
                        }
                    } else {
                        row(
                            "Reading repository…",
                            "",
                            CardIcon::Folder,
                            None,
                            false,
                            false,
                        );
                    }
                }
                1 => {
                    if let Some(message) = &self.task_message {
                        for line in display_lines(message) {
                            row(&line, "", CardIcon::File, None, false, false);
                        }
                    }
                    if let Some(custom) = &self.custom_tasks {
                        if let Some(problem) = &custom.problem {
                            for line in display_lines(problem) {
                                row(&line, "", CardIcon::File, None, false, false);
                            }
                        } else if !custom.tasks.is_empty() && !custom.trusted {
                            // Someone else's commands: show them all before anything runs.
                            row(
                                "Review before running:",
                                "",
                                CardIcon::File,
                                None,
                                false,
                                false,
                            );
                            for task in &custom.tasks {
                                for line in display_lines(&format!("{}: {}", task.name, task.run)) {
                                    row(&line, "", CardIcon::File, None, false, false);
                                }
                            }
                            row(
                                "Trust these tasks",
                                "",
                                CardIcon::Play,
                                Some(TRUST_ROW),
                                false,
                                false,
                            );
                        } else {
                            for (i, task) in custom.tasks.iter().enumerate() {
                                let status = self
                                    .task_run
                                    .as_ref()
                                    .filter(|r| r.name == task.name && r.root == custom.root)
                                    .map(|r| match r.state {
                                        crate::tasks::RunState::Sent => "",
                                        crate::tasks::RunState::Running => "Running…",
                                        crate::tasks::RunState::Finished(0) => "✓ Done",
                                        crate::tasks::RunState::Finished(_) => "✗ Failed",
                                    })
                                    .unwrap_or("");
                                row(
                                    &task.name,
                                    status,
                                    CardIcon::Play,
                                    Some(TASK_ROWS + i),
                                    false,
                                    false,
                                );
                            }
                        }
                        row(
                            "Add task",
                            "",
                            CardIcon::Plus,
                            Some(ADD_TASK_ROW),
                            false,
                            false,
                        );
                        if custom.file().exists() || custom.problem.is_some() {
                            row(
                                "Edit tasks file",
                                "",
                                CardIcon::Edit,
                                Some(EDIT_TASKS_ROW),
                                false,
                                false,
                            );
                        }
                    }
                    if let Some(s) = s {
                        if !s.project.tasks.is_empty() {
                            row("Detected", "", CardIcon::File, None, false, false);
                        }
                        for (i, task) in s.project.tasks.iter().enumerate() {
                            row(
                                &task.label,
                                "",
                                CardIcon::Play,
                                Some(1000 + i),
                                false,
                                self.selected_task.as_ref() == Some(task),
                            );
                            if self.selected_task.as_ref() == Some(task) {
                                for line in
                                    display_lines(&format!("Command: {}", task.argv.join(" ")))
                                        .into_iter()
                                        .chain(display_lines(&format!(
                                            "In: {}",
                                            task.cwd.display()
                                        )))
                                        .chain(display_lines(&format!(
                                            "Definition: {}",
                                            task.definition
                                        )))
                                {
                                    row(&line, "", CardIcon::File, None, false, false);
                                }
                                row(
                                    "Run in new terminal",
                                    "",
                                    CardIcon::Play,
                                    Some(900),
                                    false,
                                    false,
                                );
                            }
                        }
                    }
                }
                5 => {
                    if let Some(s) = s {
                        for line in display_lines(&s.project.root.display().to_string()) {
                            row(&line, "", CardIcon::Folder, None, false, false);
                        }
                        for tag in &s.project.tags {
                            row(tag, "Detected", CardIcon::File, None, false, false);
                        }
                        for file in &s.project.evidence {
                            row(file, "Found", CardIcon::File, None, false, false);
                        }
                    }
                }
                6 => {
                    if let Some(s) = s {
                        row(&s.branch, "Current", CardIcon::Branch, None, false, false);
                        if s.git_details.upstream.is_empty() {
                            row(
                                "No upstream configured",
                                "",
                                CardIcon::Link,
                                None,
                                false,
                                false,
                            );
                        } else {
                            for line in
                                display_lines(&format!("Upstream: {}", s.git_details.upstream))
                            {
                                row(&line, "", CardIcon::Link, None, false, false);
                            }
                            if let Some((a, b)) = s.git_details.ahead_behind {
                                row(
                                    &format!("Ahead {a} · behind {b}"),
                                    "",
                                    CardIcon::Branch,
                                    None,
                                    false,
                                    false,
                                );
                            }
                            row(
                                "Compared with local refs",
                                "",
                                CardIcon::Link,
                                None,
                                false,
                                false,
                            );
                        }
                        if matches!(self.pull_request, Some(PrState::Unavailable)) {
                            row(
                                "GitHub status unavailable",
                                "",
                                CardIcon::Link,
                                None,
                                false,
                                false,
                            );
                        }
                        if matches!(self.pull_request, Some(PrState::None)) {
                            row(
                                "No PR for this branch",
                                "",
                                CardIcon::Link,
                                None,
                                false,
                                false,
                            );
                        }
                        for (i, b) in s.git_details.branches.iter().enumerate() {
                            row(
                                &b.name,
                                if b.current { "Current" } else { "" },
                                CardIcon::Branch,
                                Some(4000 + i),
                                false,
                                self.selected_item == Some(4000 + i),
                            );
                            if self.selected_item == Some(4000 + i) {
                                for line in display_lines(&format!(
                                    "{} {} {}",
                                    b.name, b.upstream, b.tracking
                                )) {
                                    row(&line, "", CardIcon::Branch, None, false, false);
                                }
                            }
                        }
                    }
                }
                7 => {
                    if let Some(s) = s {
                        for (i, w) in s.git_details.worktrees.iter().enumerate() {
                            let current = w.path == s.root;
                            row(
                                &w.branch,
                                if current {
                                    "Current"
                                } else if w.prunable {
                                    "Unavailable"
                                } else if w.locked {
                                    "Locked"
                                } else {
                                    "Open"
                                },
                                CardIcon::Folder,
                                (!w.prunable).then_some(3000 + i),
                                false,
                                false,
                            );
                            for line in display_lines(&w.path.display().to_string()) {
                                row(&line, "", CardIcon::Folder, None, false, false);
                            }
                        }
                    }
                }
                8 => {
                    if let Some(PrState::Found(pr)) = &self.pull_request {
                        for line in display_lines(&pr.title) {
                            row(&line, "", CardIcon::Link, None, false, false);
                        }
                        row(&pr.state, "", CardIcon::Link, None, false, false);
                        for line in display_lines(&pr.review) {
                            row(&line, "", CardIcon::Link, None, false, false);
                        }
                        if pr.checks.is_empty() {
                            row("No checks reported", "", CardIcon::Link, None, false, false);
                        }
                        for (name, status) in &pr.checks {
                            row(name, status, CardIcon::Link, None, false, false);
                        }
                        row(
                            "Open on GitHub",
                            "",
                            CardIcon::Link,
                            Some(6000),
                            false,
                            false,
                        );
                    }
                }
                2 => {
                    if let Some(s) = s {
                        for (name, status) in &s.agents {
                            let applies = if name == "Codex CLI" {
                                s.configs
                                    .iter()
                                    .any(|p| p == "AGENTS.md" || p.starts_with(".codex/"))
                            } else {
                                s.configs
                                    .iter()
                                    .any(|p| p == "CLAUDE.md" || p.starts_with(".claude/"))
                            };
                            if applies {
                                row(name, status, CardIcon::Agent, None, false, false);
                            }
                        }
                    }
                    row(
                        "Session adapter not connected",
                        "",
                        CardIcon::Link,
                        None,
                        false,
                        false,
                    );
                    row(
                        "No context or usage inferred",
                        "",
                        CardIcon::File,
                        None,
                        false,
                        false,
                    );
                }
                3 | 4 => {
                    if let Some(s) = s {
                        for (i, config) in s
                            .config_files
                            .iter()
                            .enumerate()
                            .filter(|(_, c)| c.mcp == (id == 4))
                        {
                            row(
                                &config.label,
                                if config.mcp { "Configured" } else { "Found" },
                                CardIcon::File,
                                Some(2000 + i),
                                false,
                                self.selected_item == Some(2000 + i),
                            );
                            if self.selected_item == Some(2000 + i) {
                                for line in display_lines(&format!("Scope: {}", config.scope))
                                    .into_iter()
                                    .chain(display_lines(&config.path.display().to_string()))
                                {
                                    row(&line, "", CardIcon::Folder, None, false, false);
                                }
                                for server in &config.servers {
                                    row(server, "Configured", CardIcon::Link, None, false, false);
                                }
                                for line in config
                                    .preview
                                    .iter()
                                    .flat_map(|l| display_lines(l))
                                    .take(40)
                                {
                                    row(&line, "", CardIcon::File, None, false, false);
                                }
                            }
                        }
                        if s.config_files.is_empty() {
                            // compatibility for synthetic/test snapshots
                            for config in
                                s.configs.iter().filter(|p| p.contains("mcp") == (id == 4))
                            {
                                row(config, "Found", CardIcon::File, None, false, false);
                            }
                            if id == 4 {
                                for name in &s.mcp_names {
                                    row(name, "Not linked", CardIcon::Link, None, false, false);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        for row in &mut card.rows {
            row.tone = match row.icon {
                CardIcon::Branch => CardTone::Purple,
                CardIcon::Play => CardTone::Blue,
                CardIcon::Agent => CardTone::Purple,
                CardIcon::Link => CardTone::Purple,
                CardIcon::File => CardTone::Cyan,
                CardIcon::Server | CardIcon::Folder => CardTone::Blue,
                _ => CardTone::Muted,
            };
            if row.section && row.action == Some(0) {
                if s.is_some_and(|s| s.error.is_some()) {
                    row.tone = CardTone::Red;
                }
                row.diff = s.and_then(|s| s.diff);
                if row.diff.is_some() {
                    row.detail.clear();
                }
            } else if row.action.is_some_and(|id| (200..300).contains(&id)) {
                row.tone = git_tone(&row.detail);
            } else if matches!(
                row.detail.as_str(),
                "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIR…"
            ) {
                row.tone = CardTone::Red;
            } else if matches!(row.detail.as_str(), "SUCCESS" | "APPROVED") {
                row.tone = CardTone::Green;
            } else if row.detail == "Configured" {
                row.tone = CardTone::Blue;
            } else if row.detail == "Unavailable"
                || row.detail == "Not linked"
                || row.detail == "Not connected"
                || row.detail == "Unknown API"
            {
                row.tone = CardTone::Amber;
            } else if row.detail == "Not found" {
                row.tone = CardTone::Red;
            } else if row.detail == "Installed"
                || row.detail == "Found"
                || (row.detail.ends_with(" loaded") && row.detail != "0 loaded")
            {
                row.tone = CardTone::Green;
            }
        }
        // A shortcut to search, on top of a card that has something to show.
        if !card.rows.is_empty() {
            card.rows.insert(
                0,
                CardRow {
                    label: "Search".into(),
                    detail: "⌘F".into(),
                    icon: CardIcon::Search,
                    action: Some(SEARCH_ROW),
                    expanded: false,
                    section: false,
                    tone: CardTone::Muted,
                    diff: None,
                },
            );
        }
        card
    }
}
fn display_lines(text: &str) -> Vec<String> {
    use unicode_width::UnicodeWidthChar;
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut width = 0;
    for c in text.chars().take(512) {
        let c = if c.is_control() { ' ' } else { c };
        let w = c.width().unwrap_or(0);
        if width + w > 27 {
            lines.push(std::mem::take(&mut line));
            width = 0;
        }
        line.push(c);
        width += w;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

/// Card row actions handled by the app (outside `activate`).
pub const SEARCH_ROW: usize = 1300;
pub const TASK_ROWS: usize = 1100;
pub const MAX_TASK_ROWS: usize = volt_config::tasks::MAX_TASKS;
pub const ADD_TASK_ROW: usize = 1202;
pub const EDIT_TASKS_ROW: usize = 1200;
pub const TRUST_ROW: usize = 1201;

fn section_available(id: usize, s: &Snapshot) -> bool {
    match id {
        0 => s.repository,
        // Any project or checkout can have its own tasks, so offer "Add task".
        1 => s.rust || !s.project.tasks.is_empty() || s.repository || !s.project.tags.is_empty(),
        2 => s.configs.iter().any(|p| {
            p == "AGENTS.md"
                || p == "CLAUDE.md"
                || p.starts_with(".codex/")
                || p.starts_with(".claude/")
        }),
        3 => s.configs.iter().any(|p| !p.contains("mcp")),
        4 => s.configs.iter().any(|p| p.contains("mcp")) || !s.mcp_names.is_empty(),
        5 => !s.project.tags.is_empty(),
        6 => s.repository,
        7 => s.git_details.worktrees.len() > 1,
        _ => false,
    }
}
fn git_tone(status: &str) -> CardTone {
    if status.contains('U') || matches!(status, "AA" | "DD") || status.contains('D') {
        CardTone::Red
    } else if status.contains('A') || status.contains('?') {
        CardTone::Green
    } else if status.contains('R') || status.contains('C') {
        CardTone::Blue
    } else {
        CardTone::Amber
    }
}
fn status_name(c: u8) -> &'static str {
    match c {
        b' ' => "unchanged",
        b'M' => "modified",
        b'A' => "added",
        b'D' => "deleted",
        b'R' => "renamed",
        b'C' => "copied",
        b'?' => "untracked",
        b'U' => "conflict",
        _ => "changed",
    }
}
pub(crate) fn safe_label(s: &str, max: usize) -> String {
    use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
    let clean: String = s
        .chars()
        // Git names and rule previews are untrusted UI text. Directional
        // formatting marks can visually reorder an otherwise safe label.
        .filter(|c| {
            !matches!(
                c,
                '\u{061c}' | '\u{200e}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{206f}'
            )
        })
        .map(|c| {
            if c.is_control() || matches!(c, '\u{2028}' | '\u{2029}') {
                ' '
            } else {
                c
            }
        })
        .collect();
    if clean.width() <= max {
        return clean;
    }
    if max == 0 {
        return String::new();
    }
    let mut label = String::new();
    let mut width = 0;
    for c in clean.chars() {
        let cells = c.width().unwrap_or(0);
        if width + cells > max - 1 {
            break;
        }
        label.push(c);
        width += cells;
    }
    label.push('…');
    label
}

fn parse_status(bytes: &[u8]) -> Vec<(String, String)> {
    let mut result = Vec::new();
    let mut records = bytes.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.len() < 4 {
            continue;
        }
        let status = String::from_utf8_lossy(&record[..2]).into_owned();
        let path = String::from_utf8_lossy(&record[3..]).into_owned();
        if record[..2].iter().any(|b| matches!(b, b'R' | b'C')) {
            records.next();
        }
        result.push((status, path));
    }
    result
}
/// No shell interpolation, bounded output, deadline and child reaping.
pub(crate) fn bounded_output(command: &mut Command) -> Option<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(262_145).read_to_end(&mut bytes);
        bytes
    });
    let until = Instant::now() + Duration::from_secs(3);
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(15)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let bytes = reader.join().ok()?;
    (success && bytes.len() <= 262_144).then_some(bytes)
}
pub(crate) fn git(path: &Path, args: &[&str]) -> Option<Vec<u8>> {
    bounded_output(
        Command::new("git")
            .arg("--no-optional-locks")
            .arg("-C")
            .arg(path)
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0"),
    )
}
fn collect(path: &Path) -> Snapshot {
    let mut s = Snapshot {
        root: path.to_path_buf(),
        ..Default::default()
    };
    if let Some(root) = git(path, &["rev-parse", "--show-toplevel"]) {
        s.repository = true;
        s.root = PathBuf::from(String::from_utf8_lossy(&root).trim_end_matches('\n'));
        s.branch = git(path, &["symbolic-ref", "--short", "HEAD"])
            .or_else(|| git(path, &["rev-parse", "--short", "HEAD"]))
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .unwrap_or_else(|| "Unknown branch".into());
        if let Some(status) = git(
            path,
            &["status", "--porcelain=v1", "-z", "--untracked-files=normal"],
        ) {
            s.files = parse_status(&status);
            // HEAD -> working tree, including index changes, without executing
            // external diff drivers or textconv filters. Untracked/binary lines
            // are not invented; unavailable totals remain absent.
            s.diff = git(
                &s.root,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--numstat",
                    "-z",
                    "HEAD",
                    "--",
                ],
            )
            .and_then(|bytes| parse_numstat(&bytes));
        } else {
            s.error = Some("Git status unavailable / timed out".into());
        }
    } else if has_git_marker(path) || std::env::var_os("GIT_DIR").is_some() {
        s.repository = true;
        s.error = Some("Git discovery unavailable".into());
    } else if !path.is_dir() {
        s.error = Some("Directory unavailable".into());
    }
    s.project = workspace_project::discover(path, s.repository.then_some(s.root.as_path()));
    let boundary = if s.repository {
        &s.root
    } else {
        &s.project.root
    };
    s.config_files = workspace_project::discover_configs(path, boundary);
    s.configs = s.config_files.iter().map(|c| c.label.clone()).collect();
    s.mcp_names = s
        .config_files
        .iter()
        .flat_map(|c| c.servers.iter().cloned())
        .collect();
    s.rust = s.project.tags.iter().any(|t| t == "Rust");
    if s.repository {
        s.git_details = workspace_git::discover(&s.root);
    }
    for (name, binary) in [("Codex CLI", "codex"), ("Claude Code", "claude")] {
        let installed = std::env::var_os("PATH")
            .is_some_and(|paths| std::env::split_paths(&paths).any(|p| p.join(binary).is_file()));
        s.agents.push((
            name.into(),
            if installed { "Installed" } else { "Not found" }.into(),
        ));
    }

    s
}

/// Query only the owned local shell, not OSC hostnames or arbitrary processes.
pub(crate) fn local_process_cwd(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: zeroed C POD, exact structure size, valid writable buffer.
        let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as i32;
        let count = unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDVNODEPATHINFO,
                0,
                (&mut info as *mut libc::proc_vnodepathinfo).cast(),
                size,
            )
        };
        if count != size {
            return None;
        }
        let bytes: Vec<u8> = info
            .pvi_cdir
            .vip_path
            .iter()
            .flatten()
            .take_while(|c| **c != 0)
            .map(|c| *c as u8)
            .collect();
        if bytes.is_empty() {
            None
        } else {
            Some(PathBuf::from(std::ffi::OsStr::from_bytes(&bytes)))
        }
    }
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/cwd")).ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

fn has_git_marker(path: &Path) -> bool {
    path.ancestors()
        .any(|p| match std::fs::symlink_metadata(p.join(".git")) {
            Ok(_) => true,
            Err(e) => e.kind() != std::io::ErrorKind::NotFound,
        })
}
fn parse_numstat(bytes: &[u8]) -> Option<(u64, u64)> {
    let mut total = (0u64, 0u64);
    let mut records = bytes.split(|b| *b == 0);
    while let Some(record) = records.next() {
        if record.is_empty() {
            continue;
        }
        let mut parts = record.splitn(3, |b| *b == b'\t');
        let added = parts.next()?;
        let removed = parts.next()?;
        let path = parts.next()?;
        if path.is_empty() {
            records.next()?;
            records.next()?;
        } // rename: two NUL paths
        if added == b"-" && removed == b"-" {
            continue;
        }
        total.0 = total
            .0
            .checked_add(std::str::from_utf8(added).ok()?.parse::<u64>().ok()?)?;
        total.1 = total
            .1
            .checked_add(std::str::from_utf8(removed).ok()?.parse::<u64>().ok()?)?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_and_pane_changes_clear_stale_state_and_reject_old_results() {
        let mut panel = WorkspacePanel::default();
        let a = Some(PathBuf::from("/repo/a"));
        let b = Some(PathBuf::from("/repo/b"));
        assert!(panel.update_scope(a.clone(), Some(1)));
        let revision_a = panel.scope_revision;
        assert!(panel.accept_snapshot(
            revision_a,
            Snapshot {
                root: PathBuf::from("/repo"),
                repository: true,
                ..Default::default()
            }
        ));
        assert_eq!(panel.card().title, "a"); // cwd, not the repository root
        panel.selected_file = Some(3);
        panel.scroll = 7;
        assert!(panel.update_scope(b, Some(1)));
        assert_eq!(panel.card().title, "b");
        assert!(panel.snapshot.is_none());
        assert_eq!(panel.selected_file, None);
        assert_eq!(panel.scroll, 0);
        assert!(!panel.accept_snapshot(revision_a, Snapshot::default()));
        assert!(panel.update_scope(a.clone(), Some(1)));
        assert!(!panel.accept_snapshot(revision_a, Snapshot::default()));
        let revision = panel.scope_revision;
        assert!(panel.update_scope(a.clone(), Some(2))); // another pane at the same path
        assert!(!panel.accept_snapshot(revision, Snapshot::default()));
        assert!(!panel.update_scope(a, Some(2)));
        assert!(panel.accept_snapshot(panel.scope_revision, Snapshot::default()));
        assert_eq!(panel.card().status, "");
    }

    #[test]
    fn directory_checks_continue_during_discovery_and_while_minimized() {
        let mut panel = WorkspacePanel {
            visible: true,
            minimized: true,
            ..Default::default()
        };
        panel.next_cwd_check = Instant::now() + Duration::from_millis(250);
        panel.next_refresh = Instant::now() + Duration::from_secs(15);
        assert_eq!(panel.refresh_deadline(), Some(panel.next_cwd_check));
        let (_tx, rx) = mpsc::channel();
        panel.pending = Some(rx);
        assert_eq!(panel.refresh_deadline(), Some(panel.next_cwd_check));
        panel.visible = false;
        assert_eq!(panel.refresh_deadline(), None);
    }

    #[test]
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn owned_shell_directory_follows_cd_without_prompt_hooks() {
        use std::io::Write;
        let target = std::env::temp_dir().canonicalize().unwrap();
        let mut child = Command::new("/bin/sh")
            .args([
                "-c",
                "cd /; read ignored; cd \"$1\"; read ignored",
                "cwd-test",
            ])
            .arg(&target)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let wait_for = |path: &Path| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while Instant::now() < deadline {
                if local_process_cwd(child.id()).as_deref() == Some(path) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            false
        };
        let first = wait_for(Path::new("/"));
        child
            .stdin
            .as_ref()
            .unwrap()
            .write_all(b"continue\n")
            .unwrap();
        let second = wait_for(&target);
        let _ = child.kill();
        let _ = child.wait();
        assert!(
            first && second,
            "native cwd lookup must observe both shell directories"
        );
    }

    #[test]
    fn parses_spaces_and_rename_without_splitting_paths() {
        let files =
            parse_status(b" M file with spaces.rs\0R  new name.rs\0old name.rs\0?? new.txt\0");
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].1, "file with spaces.rs");
        assert_eq!(files[1].1, "new name.rs");
    }
    #[test]
    fn truncation_is_unicode_safe_and_removes_controls() {
        assert_eq!(safe_label("a\nb\u{1b}c", 20), "a b c");
        assert_eq!(safe_label("日本語の長い名前", 4), "日…");
        assert_eq!(
            safe_label("branch\u{202e}txt.exe\u{202c}\u{2066}safe\u{2069}", 30),
            "branchtxt.exesafe"
        );
        assert_eq!(safe_label("a\u{200e}b\u{200f}c\u{061c}d", 10), "abcd");
        assert_eq!(safe_label("x\u{2028}y\u{2029}z\u{206a}w", 10), "x y zw");
    }
    #[test]
    fn only_one_section_is_expanded_at_a_time() {
        let mut panel = WorkspacePanel::default();
        panel.activate(2);
        assert_eq!(panel.expanded, Some(2));
        panel.activate(3);
        assert_eq!(panel.expanded, Some(3));
        panel.activate(3);
        assert_eq!(panel.expanded, None);
    }

    #[test]
    fn local_shell_directory_is_available_without_prompt_hooks() {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        assert_eq!(
            local_process_cwd(std::process::id()),
            std::env::current_dir().ok()
        );
    }
    #[test]
    fn config_discovery_never_claims_a_connected_session() {
        let panel = WorkspacePanel {
            expanded: Some(4),
            snapshot: Some(Snapshot {
                configs: vec![".mcp.json".into()],
                mcp_names: vec!["example".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let card = panel.card();
        assert!(card
            .rows
            .iter()
            .any(|r| r.label == "example" && r.detail == "Not linked"));
        assert!(!card.rows.iter().any(|r| r.detail == "Connected"));
    }
    #[test]
    fn failed_git_discovery_is_not_rendered_as_clean() {
        let panel = WorkspacePanel {
            expanded: Some(0),
            snapshot: Some(Snapshot {
                error: Some("Git failed".into()),
                repository: true,
                ..Default::default()
            }),
            ..Default::default()
        };
        let card = panel.card();
        assert!(!card.rows.iter().any(|r| r.label == "Working tree clean"));
    }
    #[test]
    fn subprocess_failure_and_output_limit_are_explicit() {
        assert!(bounded_output(Command::new("/bin/sh").args(["-c", "exit 1"])).is_none());
        assert!(
            bounded_output(Command::new("/bin/sh").args(["-c", "yes x | head -c 300000"]))
                .is_none()
        );
    }

    #[test]
    fn ordinary_directories_hide_card_without_disabling_it() {
        for root in ["/tmp", "/Users/example/Documents", "/work/empty"] {
            let panel = WorkspacePanel {
                visible: true,
                snapshot: Some(Snapshot {
                    root: PathBuf::from(root),
                    // Installed programs alone are not project context.
                    agents: vec![("Codex CLI".into(), "Installed".into())],
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(panel.presentation_card().is_none());
            assert!(panel.visible);
            assert!(panel.refresh_deadline().is_some());
            assert!(panel.card().rows.is_empty());
            assert_ne!(panel.card().status, "Local");
        }
    }

    #[test]
    fn useful_non_git_context_and_git_errors_stay_visible() {
        for snapshot in [
            Snapshot {
                rust: true,
                ..Default::default()
            },
            Snapshot {
                configs: vec!["AGENTS.md".into()],
                ..Default::default()
            },
            Snapshot {
                configs: vec![".mcp.json".into()],
                ..Default::default()
            },
            Snapshot {
                repository: true,
                error: Some("Git failed".into()),
                ..Default::default()
            },
        ] {
            let panel = WorkspacePanel {
                visible: true,
                snapshot: Some(snapshot),
                ..Default::default()
            };
            let card = panel
                .presentation_card()
                .expect("relevant context must render");
            assert!(!card.rows.is_empty());
            assert!(!card.rows.iter().any(|row| row.label == "Local models"));
            assert_ne!(card.subtitle, "Local directory");
        }
    }

    #[test]
    fn auto_hide_reappears_on_project_but_explicit_close_stays_closed() {
        let mut panel = WorkspacePanel {
            visible: true,
            ..Default::default()
        };
        let project = || Snapshot {
            repository: true,
            ..Default::default()
        };
        panel.update_scope(Some("/project".into()), Some(1));
        assert!(panel.presentation_card().is_none()); // no speculative card during discovery
        panel.accept_snapshot(panel.scope_revision, project());
        assert!(panel.presentation_card().is_some());
        panel.activate(102);
        panel.update_scope(Some("/tmp".into()), Some(1));
        panel.accept_snapshot(panel.scope_revision, Snapshot::default());
        assert!(panel.presentation_card().is_none());
        panel.update_scope(Some("/project".into()), Some(1));
        panel.accept_snapshot(panel.scope_revision, project());
        assert!(panel.presentation_card().unwrap().minimized);
        panel.activate(101);
        assert!(panel.presentation_card().is_none());
        assert!(panel.refresh_deadline().is_none());
    }

    #[test]
    fn plain_directory_hides_irrelevant_sections() {
        let panel = WorkspacePanel {
            snapshot: Some(Snapshot {
                root: PathBuf::from("/tmp/plain"),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(!panel.card().rows.iter().any(|r| r.section));
    }
    #[test]
    fn project_context_shows_only_applicable_sections() {
        let panel = WorkspacePanel {
            snapshot: Some(Snapshot {
                configs: vec!["AGENTS.md".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let sections: Vec<_> = panel
            .card()
            .rows
            .iter()
            .filter_map(|r| r.section.then_some(r.action.unwrap()))
            .collect();
        assert_eq!(sections, vec![2, 3]);
    }

    #[test]
    fn git_totals_handle_rename_tabs_binary_and_invalid_records() {
        assert_eq!(
            parse_numstat(
                b"10\t2\tfile with spaces\0-\t-\timage.png\x003\t1\t\0old name\0new name\0"
            ),
            Some((13, 3))
        );
        assert_eq!(parse_numstat(b"2\t1\tfile\twith\ttabs\0"), Some((2, 1)));
        assert_eq!(parse_numstat(b"garbage\0"), None);
        assert_eq!(parse_numstat(b""), Some((0, 0)));
    }
    #[test]
    fn git_status_colors_and_header_state_are_semantic() {
        assert_eq!(git_tone("??"), CardTone::Green);
        assert_eq!(git_tone("A "), CardTone::Green);
        assert_eq!(git_tone(" D"), CardTone::Red);
        assert_eq!(git_tone("UU"), CardTone::Red);
        assert_eq!(git_tone(" M"), CardTone::Amber);
        assert_eq!(git_tone("R "), CardTone::Blue);
        let mut panel = WorkspacePanel {
            snapshot: Some(Snapshot {
                repository: true,
                branch: "main".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(panel.card().status, "Clean");
        panel.snapshot.as_mut().unwrap().error = Some("timeout".into());
        assert_eq!(panel.card().status, "Unavailable");
        assert!(panel.card().rows.iter().any(|r| r.action == Some(0)));
    }
    #[test]
    fn minimizing_preserves_expansion_and_only_exposes_header_actions() {
        let mut panel = WorkspacePanel {
            visible: true,
            expanded: Some(3),
            ..Default::default()
        };
        panel.activate(102);
        assert!(panel.minimized);
        assert_eq!(panel.card().actions(), vec![103, 102, 100, 101]);
        assert_eq!(panel.expanded, Some(3));
        panel.activate(102);
        assert!(!panel.minimized);
        assert_eq!(panel.expanded, Some(3));
    }
}
