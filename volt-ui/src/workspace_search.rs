//! Local, bounded search providers. No shell execution, no network, no TUI input.
use crate::{
    prompt::{PromptKind, TextPrompt},
    workspace_git,
    workspace_panel::Snapshot,
    workspace_project::{discover, ProjectTask},
};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};
use volt_core::performer::Performer;
use volt_renderer::search_palette::{SearchRow, SearchView};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scope {
    #[default]
    All,
    Terminal,
    Files,
    Text,
    Git,
    Tasks,
}
impl Scope {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Terminal,
        Self::Files,
        Self::Text,
        Self::Git,
        Self::Tasks,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap()
    }
    pub fn name(self) -> &'static str {
        ["All", "Terminal", "Files", "Text", "Git", "Tasks"][self.index()]
    }
    fn includes(self, other: Self) -> bool {
        self == Self::All || self == other
    }
}
#[derive(Clone, Debug)]
pub enum Action {
    Terminal {
        text: String,
        alternate: bool,
        row: usize,
    },
    File {
        path: PathBuf,
        line: usize,
    },
    Branch(String),
    Worktree(PathBuf),
    Task(ProjectTask),
}
#[derive(Clone, Debug)]
pub struct ResultRow {
    pub kind: &'static str,
    pub title: String,
    pub detail: String,
    pub action: Action,
    pub score: i64,
    pub preview: Vec<String>,
}
#[derive(Clone, Debug, Default)]
pub struct SearchOutput {
    pub rows: Vec<ResultRow>,
    pub status: String,
    pub root: PathBuf,
}
pub struct SearchInput {
    pub query: String,
    pub scope: Scope,
    pub cwd: Option<PathBuf>,
    pub snapshot: Option<Snapshot>,
    pub performer: Arc<Mutex<Performer>>,
}

pub struct SearchPalette {
    pub visible: bool,
    pub input: TextPrompt,
    pub query_selected: bool,
    pub scope: Scope,
    pub selected: usize,
    pub output: SearchOutput,
    pub preview_only: bool,
    pub target: Option<u32>,
    pub cwd: Option<PathBuf>,
    generation: Arc<AtomicU64>,
    pending: Option<mpsc::Receiver<(u64, SearchOutput)>>,
    deadline: Instant,
    dirty: bool,
}
impl Default for SearchPalette {
    fn default() -> Self {
        Self {
            visible: false,
            input: TextPrompt::new(PromptKind::Find, ""),
            query_selected: false,
            scope: Scope::All,
            selected: 0,
            output: SearchOutput::default(),
            preview_only: false,
            target: None,
            cwd: None,
            generation: Arc::new(AtomicU64::new(0)),
            pending: None,
            deadline: Instant::now(),
            dirty: false,
        }
    }
}
impl Drop for SearchPalette {
    fn drop(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}
impl SearchPalette {
    pub fn open(&mut self, pid: Option<u32>, cwd: Option<PathBuf>) {
        self.visible = true;
        self.target = pid;
        self.cwd = cwd;
        self.input = TextPrompt::new(PromptKind::Find, "");
        self.query_selected = false;
        self.scope = Scope::All;
        self.changed();
    }
    pub fn close(&mut self) {
        self.visible = false;
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.dirty = false;
        self.output = SearchOutput::default();
    }
    pub fn changed(&mut self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.dirty = true;
        self.deadline = Instant::now() + Duration::from_millis(130);
        self.selected = 0;
        self.preview_only = false;
        self.output.rows.clear();
        self.output.status = "Searching locally…".into();
    }
    pub fn edit(&mut self, text: &str) {
        if self.query_selected {
            self.input = TextPrompt::new(PromptKind::Find, "");
            self.query_selected = false;
        }
        let remaining = 256usize.saturating_sub(self.input.text().chars().count());
        for c in text.chars().filter(|c| !c.is_control()).take(remaining) {
            self.input.insert_char(c);
        }
        self.changed();
    }
    pub fn delete_query(&mut self, backward: bool) {
        if self.query_selected {
            self.input = TextPrompt::new(PromptKind::Find, "");
            self.query_selected = false;
        } else if backward {
            self.input.backspace();
        } else {
            self.input.delete_forward();
        }
        self.changed();
    }
    pub fn select(&mut self, delta: isize) {
        if !self.output.rows.is_empty() {
            self.selected = (self.selected as isize + delta)
                .rem_euclid(self.output.rows.len() as isize) as usize;
            self.preview_only = false;
        }
    }
    pub fn set_scope(&mut self, scope: Scope) {
        self.scope = scope;
        self.changed();
    }
    pub fn deadline(&self) -> Option<Instant> {
        (self.visible && self.dirty && self.pending.is_none()).then_some(self.deadline)
    }
    fn poll_result(&mut self) -> bool {
        let mut changed = false;
        if let Some(rx) = &self.pending {
            match rx.try_recv() {
                Ok((generation, result)) => {
                    if self.visible && generation == self.generation.load(Ordering::Relaxed) {
                        self.output = result;
                        changed = true;
                    }
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        changed
    }
    // Never clone the macOS event-loop proxy on an idle/debounce tick; doing
    // so wakes the loop again. Workers alone need an owned proxy.
    pub fn tick(
        &mut self,
        input: impl FnOnce() -> SearchInput,
        proxy: &winit::event_loop::EventLoopProxy<crate::app::VoltEvent>,
    ) -> bool {
        let changed = self.poll_result();
        if self.visible && self.dirty && self.pending.is_none() && Instant::now() >= self.deadline {
            self.dirty = false;
            let input = input();
            let generation = self.generation.load(Ordering::Relaxed);
            let cancel = self.generation.clone();
            let (tx, rx) = mpsc::channel();
            self.pending = Some(rx);
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                let output = search(input, &cancel, generation);
                let _ = tx.send((generation, output));
                let _ = proxy.send_event(crate::app::VoltEvent::WorkspaceUpdated);
            });
        }
        changed
    }
    pub fn preview_selected(
        &mut self,
        proxy: &winit::event_loop::EventLoopProxy<crate::app::VoltEvent>,
    ) {
        if self.pending.is_some() {
            return;
        }
        let Some(row) = self.output.rows.get(self.selected) else {
            return;
        };
        self.preview_only = true;
        if let Action::File { path, line } = &row.action {
            let path = path.clone();
            let line = *line;
            let root = self.output.root.clone();
            let selected = self.selected;
            let mut output = self.output.clone();
            let generation = self.generation.load(Ordering::Relaxed);
            let (tx, rx) = mpsc::channel();
            self.pending = Some(rx);
            let proxy = proxy.clone();
            std::thread::spawn(move || {
                output.rows[selected].preview = file_preview(&path, &root, line);
                let _ = tx.send((generation, output));
                let _ = proxy.send_event(crate::app::VoltEvent::WorkspaceUpdated);
            });
        }
    }
    pub fn view(&self) -> Option<SearchView> {
        self.visible.then(|| {
            let row = self.output.rows.get(self.selected);
            let (preview, action) = if let Some(r) = row {
                (
                    r.preview.clone(),
                    match &r.action {
                        Action::File { .. } => "Enter: read-only preview",
                        Action::Terminal { .. } => "Enter: locate output (not an editor jump)",
                        Action::Branch(_) => "Enter: inspect branch (no checkout)",
                        Action::Worktree(_) => "Enter: review worktree",
                        Action::Task(_) => "Enter: review task (does not run)",
                    },
                )
            } else {
                (vec![], "Type to search · local only")
            };
            SearchView {
                query: self.input.text(),
                query_selected: self.query_selected,
                cursor: self.input.cursor,
                scope: self.scope.index(),
                selected: self.selected,
                rows: self
                    .output
                    .rows
                    .iter()
                    .map(|r| SearchRow {
                        kind: r.kind.into(),
                        title: r.title.clone(),
                        detail: r.detail.clone(),
                    })
                    .collect(),
                preview,
                status: if self.preview_only {
                    "Read-only preview · no application commands sent".into()
                } else {
                    self.output.status.clone()
                },
                root: self.output.root.display().to_string(),
                action: action.into(),
            }
        })
    }
}

/// Case-insensitive subsequence matching, with contiguous/prefix/boundary boosts.
/// Exact/literal matching is deliberately used instead for source and log lines.
pub fn fuzzy_score(query: &str, text: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let needle: Vec<_> = query.to_lowercase().chars().collect();
    let hay: Vec<_> = text.to_lowercase().chars().collect();
    let mut at = 0;
    let mut score = 0;
    let mut prev = None;
    for c in needle {
        let i = (at..hay.len()).find(|i| hay[*i] == c)?;
        score += 10;
        if i == 0 || matches!(hay[i - 1], '/' | '_' | '-' | ' ' | '.') {
            score += 15;
        }
        if prev == Some(i.saturating_sub(1)) {
            score += 20;
        }
        score -= (i - at) as i64;
        prev = Some(i);
        at = i + 1;
    }
    if text.to_lowercase().starts_with(&query.to_lowercase()) {
        score += 100;
    }
    Some(score - (hay.len() as i64 / 8))
}
fn literal(q: &str, s: &str) -> bool {
    !q.is_empty() && s.to_lowercase().contains(&q.to_lowercase())
}
fn clean(s: &str, max: usize) -> String {
    crate::workspace_panel::safe_label(s, max)
}
fn preview(text: &str, line: usize) -> Vec<String> {
    text.lines()
        .enumerate()
        .skip(line.saturating_sub(3))
        .take(10)
        .map(|(i, l)| format!("{:>4}  {}", i + 1, clean(l, 180)))
        .collect()
}
fn excluded(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules"
                | "target"
                | "build"
                | "dist"
                | "vendor"
                | "coverage"
                | "__pycache__"
                | "venv"
                | "env"
                | "secrets.json"
                | "credentials.json"
                | "credentials"
        )
        || name.ends_with(".pem")
        || name.ends_with(".key")
}
fn push(rows: &mut Vec<ResultRow>, row: ResultRow) {
    if rows.len() < 300 {
        rows.push(row);
    }
}

pub fn search(input: SearchInput, cancel: &AtomicU64, generation: u64) -> SearchOutput {
    let start = Instant::now();
    let stopped =
        || cancel.load(Ordering::Relaxed) != generation || start.elapsed() > Duration::from_secs(2);
    let mut output = SearchOutput {
        root: input.cwd.clone().unwrap_or_default(),
        ..Default::default()
    };
    let q = input.query.trim();
    if q.is_empty() {
        // Opening/clearing the palette must not walk disk, discover manifests,
        // run Git, or contend with the PTY. Reuse only already-known scope.
        if let Some(snapshot) = &input.snapshot {
            if !snapshot.project.root.as_os_str().is_empty() {
                output.root = snapshot.project.root.clone();
            }
        }
        output.status = "Type to search this workspace".into();
        return output;
    }
    let mut limited = false;
    let mut terminal_busy = false;
    if input.scope.includes(Scope::Terminal) && !q.is_empty() {
        // A bounded snapshot on the worker, never disk I/O or matching under the PTY lock.
        let snapshot = input.performer.try_lock().ok().map(|p| {
            let g = &p.grid;
            let total = g.scrollback_len() + g.rows;
            let from = total.saturating_sub(2000);
            let mut bytes = 0;
            let rows = (from..total)
                .rev()
                .take_while(|_| !stopped())
                .filter_map(|row| {
                    if bytes > 2 * 1024 * 1024 {
                        return None;
                    }
                    let text = if row < g.scrollback_len() {
                        g.row_text(g.scrollback_row(row))
                    } else {
                        g.row_text(g.row_cells(row - g.scrollback_len()))
                    };
                    bytes += text.len();
                    Some((row, text))
                })
                .collect::<Vec<_>>();
            (
                rows,
                p.alternate_screen_active(),
                from > 0 || bytes > 2 * 1024 * 1024,
            )
        });
        if let Some((rows, alternate, truncated)) = snapshot {
            limited |= truncated;
            for (row, text) in rows {
                if literal(q, &text) {
                    push(
                        &mut output.rows,
                        ResultRow {
                            kind: "OUTPUT",
                            title: clean(text.trim(), 150),
                            detail: format!(
                                "{} · row {}",
                                if alternate {
                                    "Current TUI screen"
                                } else {
                                    "Active terminal snapshot"
                                },
                                row + 1
                            ),
                            action: Action::Terminal {
                                text: text.clone(),
                                alternate,
                                row,
                            },
                            score: 100,
                            preview: vec![
                                clean(text.trim_end(), 180),
                                "Snapshot of displayed output; not an editor buffer".into(),
                            ],
                        },
                    );
                }
                if output.rows.len() >= 60 {
                    limited = true;
                    break;
                }
            }
        } else {
            terminal_busy = true;
            if input.scope == Scope::Terminal {
                output.status = "Terminal busy; type again to refresh".into();
            }
        }
    }
    if stopped() {
        return output;
    }
    if input.scope == Scope::Terminal {
        if output.status.is_empty() {
            output.status = format!(
                "{}{} matches · last 2,000 retained rows · snapshot",
                output.rows.len(),
                if limited { "+" } else { "" }
            );
        }
        return output;
    }
    let mut project = input
        .snapshot
        .as_ref()
        .map(|s| s.project.clone())
        .unwrap_or_default();
    if let Some(cwd) = input.cwd.as_ref() {
        if project.root.as_os_str().is_empty() {
            project = discover(cwd, None);
        }
        output.root = project.root.clone();
    }
    if input.scope.includes(Scope::Tasks) {
        for task in &project.tasks {
            if let Some(score) = fuzzy_score(q, &task.label) {
                push(
                    &mut output.rows,
                    ResultRow {
                        kind: "TASK",
                        title: task.label.clone(),
                        detail: format!("Review · {}", task.cwd.display()),
                        score,
                        preview: vec![
                            format!("Command: {}", task.argv.join(" ")),
                            format!("Directory: {}", task.cwd.display()),
                            format!("Definition: {}", clean(&task.definition, 180)),
                            "Enter opens review; never executes a task".into(),
                        ],
                        action: Action::Task(task.clone()),
                    },
                );
            }
        }
    }
    if input.scope.includes(Scope::Git) {
        let details = input
            .snapshot
            .as_ref()
            .filter(|s| s.repository)
            .map(|s| s.git_details.clone())
            .or_else(|| input.cwd.as_ref().map(|p| search_git(p, &stopped)));
        if let Some(d) = details {
            for b in &d.branches {
                if stopped() {
                    limited = true;
                    break;
                }
                if let Some(score) = fuzzy_score(q, &b.name) {
                    push(
                        &mut output.rows,
                        ResultRow {
                            kind: "BRANCH",
                            title: b.name.clone(),
                            detail: if b.current {
                                "Current branch".into()
                            } else {
                                "Inspect only".into()
                            },
                            score,
                            preview: vec![
                                b.name.clone(),
                                format!("Upstream: {}", b.upstream),
                                b.tracking.clone(),
                                "No checkout will be performed".into(),
                            ],
                            action: Action::Branch(b.name.clone()),
                        },
                    );
                }
            }
            for w in &d.worktrees {
                if let Some(score) = fuzzy_score(q, &format!("{} {}", w.branch, w.path.display())) {
                    push(
                        &mut output.rows,
                        ResultRow {
                            kind: "WORKTREE",
                            title: w.branch.clone(),
                            detail: w.path.display().to_string(),
                            score,
                            preview: vec![
                                w.path.display().to_string(),
                                if w.prunable {
                                    "Unavailable worktree".into()
                                } else {
                                    "Inspect path; no branch changes".into()
                                },
                            ],
                            action: Action::Worktree(w.path.clone()),
                        },
                    );
                }
            }
        }
    }
    if (input.scope.includes(Scope::Files) || input.scope.includes(Scope::Text))
        && !output.root.as_os_str().is_empty()
        && !stopped()
    {
        let root = output.root.clone();
        let mut walker = ignore::WalkBuilder::new(&root);
        walker
            .hidden(true)
            .follow_links(false)
            .require_git(false)
            .max_depth(Some(24))
            .max_filesize(Some(1024 * 1024))
            .filter_entry(|e| e.depth() == 0 || !excluded(&e.file_name().to_string_lossy()));
        let mut count = 0;
        let mut total_bytes = 0;
        let mut file_rows = Vec::new();
        let mut text_rows = Vec::new();
        for entry in walker.build() {
            if stopped() || count >= 10000 || total_bytes >= 16 * 1024 * 1024 {
                limited = true;
                break;
            }
            let Ok(entry) = entry else {
                limited = true;
                continue;
            };
            if excluded(&entry.file_name().to_string_lossy())
                || !entry.file_type().is_some_and(|t| t.is_file())
            {
                continue;
            }
            count += 1;
            let path = entry.path();
            let label = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .display()
                .to_string();
            if input.scope.includes(Scope::Files) {
                if let Some(score) = fuzzy_score(q, &label) {
                    file_rows.push(ResultRow {
                        kind: "FILE",
                        title: label.clone(),
                        detail: "Saved file · read-only preview".into(),
                        score,
                        preview: vec![
                            label.clone(),
                            "Enter to preview locally (not opened in nvim)".into(),
                        ],
                        action: Action::File {
                            path: path.into(),
                            line: 1,
                        },
                    });
                }
            }
            if input.scope.includes(Scope::Text) && !q.is_empty() && text_rows.len() < 100 {
                if let Some(text) = read_source(path, &root) {
                    total_bytes += text.len();
                    for (line, txt) in text.lines().enumerate() {
                        if literal(q, txt) {
                            text_rows.push(ResultRow {
                                kind: "TEXT",
                                title: clean(txt.trim(), 150),
                                detail: format!("{label}:{}", line + 1),
                                score: 80,
                                preview: preview(&text, line + 1),
                                action: Action::File {
                                    path: path.into(),
                                    line: line + 1,
                                },
                            });
                            if text_rows.len() >= 100 {
                                limited = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        file_rows.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
        if file_rows.len() > 80 {
            limited = true;
        }
        output.rows.extend(file_rows.into_iter().take(80));
        output.rows.extend(text_rows);
    }
    // Keep categories grouped; scores compare candidates within their own provider.
    let order = |kind: &str| match kind {
        "OUTPUT" => 0,
        "FILE" => 1,
        "TEXT" => 2,
        "BRANCH" | "WORKTREE" => 3,
        _ => 4,
    };
    output.rows.sort_by(|a, b| {
        order(a.kind)
            .cmp(&order(b.kind))
            .then_with(|| b.score.cmp(&a.score))
            .then_with(|| a.title.cmp(&b.title))
    });
    limited |= stopped();
    output.rows.truncate(300);
    if output.status.is_empty() {
        output.status = if limited {
            "Limited results · narrow query/scope · ignores hidden/build files".into()
        } else {
            format!(
                "{} results · local only · ignored/hidden files excluded",
                output.rows.len()
            )
        };
    }
    if terminal_busy {
        output
            .status
            .push_str(" · terminal busy; type again to refresh");
    }
    output
}
/// Revalidate containment and regular-file type before every preview/read.
pub fn read_source(path: &Path, root: &Path) -> Option<String> {
    if path
        .strip_prefix(root)
        .ok()?
        .components()
        .any(|c| excluded(&c.as_os_str().to_string_lossy()))
    {
        return None;
    }
    if !std::fs::symlink_metadata(path).ok()?.is_file() {
        return None;
    }
    let canonical = path.canonicalize().ok()?;
    if !canonical.starts_with(root.canonicalize().ok()?) {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > 1024 * 1024 {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(1024 * 1024 + 1).read_to_end(&mut bytes).ok()?;
    if bytes.len() > 1024 * 1024 || bytes.contains(&0) {
        return None;
    }
    String::from_utf8(bytes).ok()
}
pub fn file_preview(path: &Path, root: &Path, line: usize) -> Vec<String> {
    read_source(path, root)
        .map(|t| preview(&t, line))
        .unwrap_or_else(|| vec!["Unavailable, binary, too large, or outside workspace".into()])
}

// Read-only, local Git discovery: no remote, status scan, checkout, or hooks.
// Each process has the existing 3-second timeout and output cap; cancellation
// is checked between processes. Filesystem scanning has a separate 2s budget.
fn search_git(root: &Path, stopped: &impl Fn() -> bool) -> workspace_git::GitDetails {
    let mut details = workspace_git::GitDetails::default();
    if stopped() {
        return details;
    }
    if let Some(bytes) = crate::workspace_panel::git(
        root,
        &[
            "for-each-ref",
            "--count=64",
            "--format=%(HEAD)%00%(refname:short)%00%(upstream:short)",
            "refs/heads/",
        ],
    ) {
        for line in bytes.split(|b| *b == b'\n') {
            let fields: Vec<_> = line
                .split(|b| *b == 0)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            if fields.len() == 3 {
                details.branches.push(workspace_git::Branch {
                    current: fields[0] == "*",
                    name: fields[1].clone(),
                    upstream: fields[2].clone(),
                    tracking: String::new(),
                });
            }
        }
    }
    if !stopped() {
        if let Some(bytes) =
            crate::workspace_panel::git(root, &["worktree", "list", "--porcelain", "-z"])
        {
            details.worktrees = workspace_git::parse_worktrees(&bytes);
        }
    }
    details
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "volt-search-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn put(&self, path: &str, text: &str) {
            let p = self.0.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        fn input(&self, q: &str, scope: Scope) -> SearchInput {
            SearchInput {
                query: q.into(),
                scope,
                cwd: Some(self.0.clone()),
                snapshot: None,
                performer: Arc::new(Mutex::new(Performer::new(80, 24))),
            }
        }
        fn run(&self, q: &str, scope: Scope) -> SearchOutput {
            search(self.input(q, scope), &AtomicU64::new(0), 0)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn late_results_cannot_replace_new_query_or_reopened_scope() {
        let mut p = SearchPalette::default();
        p.open(Some(1), None);
        let (tx, rx) = mpsc::channel();
        let generation = p.generation.load(Ordering::Relaxed);
        p.pending = Some(rx);
        p.edit("new query");
        tx.send((
            generation,
            SearchOutput {
                status: "STALE".into(),
                ..Default::default()
            },
        ))
        .unwrap();
        assert!(!p.poll_result());
        assert_ne!(p.output.status, "STALE");
        let (tx, rx) = mpsc::channel();
        let generation = p.generation.load(Ordering::Relaxed);
        p.pending = Some(rx);
        p.close();
        p.open(Some(2), Some(PathBuf::from("/new")));
        tx.send((
            generation,
            SearchOutput {
                status: "OLD DIRECTORY".into(),
                ..Default::default()
            },
        ))
        .unwrap();
        assert!(!p.poll_result());
        assert_ne!(p.output.status, "OLD DIRECTORY");
    }
    #[test]
    fn matching_generation_is_accepted() {
        let mut p = SearchPalette::default();
        p.open(Some(1), None);
        let (tx, rx) = mpsc::channel();
        let generation = p.generation.load(Ordering::Relaxed);
        p.pending = Some(rx);
        tx.send((
            generation,
            SearchOutput {
                status: "CURRENT".into(),
                ..Default::default()
            },
        ))
        .unwrap();
        assert!(p.poll_result());
        assert_eq!(p.output.status, "CURRENT");
    }
    #[test]
    fn select_all_replaces_and_paste_is_bounded() {
        let mut p = SearchPalette::default();
        p.edit("original");
        p.query_selected = true;
        p.edit("你好");
        assert_eq!(p.input.text(), "你好");
        p.query_selected = true;
        p.delete_query(true);
        assert!(p.input.is_empty());
        p.edit(&"x".repeat(1000000));
        assert_eq!(p.input.text().len(), 256);
        p.edit("more");
        assert_eq!(p.input.text().len(), 256);
    }
    #[test]
    fn fuzzy_unicode_and_ranking() {
        assert!(fuzzy_score("wsr", "workspace_search.rs").is_some());
        assert!(fuzzy_score("你好", "src/你好吗.rs").is_some());
        assert!(fuzzy_score("zzz", "workspace.rs").is_none());
        assert!(
            fuzzy_score("main", "main.rs") > fuzzy_score("main", "src/my_application_index.rs")
        );
    }
    #[test]
    fn literal_not_regex() {
        assert!(literal("a[b]", "a[b]"));
        assert!(!literal("a.b", "axb"));
        assert!(literal("É", "café"));
    }
    #[test]
    fn ignored_secret_generated_and_hidden_files_are_excluded() {
        let f = Fixture::new();
        for file in [
            ".env",
            ".mcp.json",
            "secrets.json",
            "credentials.json",
            "private.key",
            "target/a.rs",
            "node_modules/a.js",
            "ignored/a.rs",
            "vendor/a.c",
            ".cache/a.rs",
        ] {
            f.put(file, "needle");
        }
        f.put(".gitignore", "ignored/\n");
        f.put("src/main.rs", "needle");
        let out = f.run("needle", Scope::Text);
        assert_eq!(out.rows.len(), 1, "{:?}", out.rows);
        assert!(out.rows[0].detail.starts_with("src/main.rs"));
    }
    #[test]
    fn filenames_fuzzy_and_text_literal() {
        let f = Fixture::new();
        f.put(
            "src/workspace_search.rs",
            "fn my_function() {}\nneedle[ok]\n",
        );
        assert_eq!(f.run("wssr", Scope::Files).rows.len(), 1);
        assert_eq!(f.run("needle[ok]", Scope::Text).rows.len(), 1);
        assert!(f.run("needle.*", Scope::Text).rows.is_empty());
    }
    #[test]
    fn empty_queries_do_not_populate_any_scope() {
        let f = Fixture::new();
        f.put("file.txt", "needle");
        f.put("package.json", r#"{"scripts":{"build":"echo build"}}"#);
        for query in ["", "  "] {
            for scope in Scope::ALL {
                let input = f.input(query, scope);
                let performer = input.performer.clone();
                let _busy = performer.lock().unwrap();
                let output = search(input, &AtomicU64::new(0), 0);
                assert!(output.rows.is_empty(), "{scope:?}: {:?}", output.rows);
                assert_eq!(output.root, f.0);
                assert_eq!(output.status, "Type to search this workspace");
            }
        }
    }

    #[test]
    fn busy_terminal_does_not_mask_other_provider_results() {
        let f = Fixture::new();
        f.put("needle.txt", "needle");
        let input = f.input("needle", Scope::All);
        let performer = input.performer.clone();
        let _busy = performer.lock().unwrap();
        let output = search(input, &AtomicU64::new(0), 0);
        assert_eq!(output.rows.len(), 2);
        assert!(output.status.starts_with("2 results"), "{}", output.status);
        assert!(output.status.contains("terminal busy"));
        let input = f.input("needle", Scope::Terminal);
        let performer = input.performer.clone();
        let _busy = performer.lock().unwrap();
        let output = search(input, &AtomicU64::new(0), 0);
        assert!(output.rows.is_empty());
        assert_eq!(output.status, "Terminal busy; type again to refresh");
    }

    #[test]
    fn cancelled_search_returns_no_results() {
        let f = Fixture::new();
        f.put("x.txt", "needle");
        let out = search(f.input("needle", Scope::All), &AtomicU64::new(1), 0);
        assert!(out.rows.is_empty());
    }
    #[test]
    fn binary_and_oversized_are_not_read() {
        let f = Fixture::new();
        f.put("nul.txt", "needle\0");
        f.put("large.txt", &"needle".repeat(180000));
        assert!(f.run("needle", Scope::Text).rows.is_empty());
        assert!(read_source(&f.0.join("large.txt"), &f.0).is_none());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_and_external_preview_are_rejected() {
        let f = Fixture::new();
        let outside = Fixture::new();
        outside.put("secret", "needle");
        std::os::unix::fs::symlink(outside.0.join("secret"), f.0.join("link")).unwrap();
        assert!(f.run("needle", Scope::Text).rows.is_empty());
        assert!(read_source(&outside.0.join("secret"), &f.0).is_none());
        assert!(read_source(&f.0.join("link"), &f.0).is_none());
    }
    #[test]
    fn results_are_bounded() {
        let f = Fixture::new();
        f.put("matches.txt", &"needle\n".repeat(500));
        let o = f.run("needle", Scope::Text);
        assert_eq!(o.rows.len(), 100);
        assert!(o.status.contains("Limited"));
    }
    #[test]
    fn query_edit_and_scope_cancel_prior_generation() {
        let mut p = SearchPalette::default();
        p.open(Some(1), None);
        let g = p.generation.load(Ordering::Relaxed);
        p.edit("你好abc");
        assert_eq!(p.input.text(), "你好abc");
        assert!(p.generation.load(Ordering::Relaxed) > g);
        p.input.backspace();
        assert_eq!(p.input.text(), "你好ab");
        p.set_scope(Scope::Files);
        p.close();
        assert!(!p.visible);
        assert!(p.deadline().is_none());
    }
    #[test]
    fn terminal_provider_does_not_scan_disk() {
        let f = Fixture::new();
        f.put("needle.txt", "needle");
        let input = f.input("needle", Scope::Terminal);
        {
            let mut p = input.performer.lock().unwrap();
            for (col, ch) in "needle displayed".chars().enumerate() {
                let mut cell = volt_core::cell::Cell::default();
                cell.set_char(ch);
                p.grid.put_char(col, 0, cell);
            }
        }
        let o = search(input, &AtomicU64::new(0), 0);
        assert_eq!(o.rows.len(), 1);
        assert!(matches!(
            o.rows[0].action,
            Action::Terminal {
                row: 0,
                alternate: false,
                ..
            }
        ));
    }
    #[test]
    fn tasks_are_data_not_execution() {
        let f = Fixture::new();
        f.put("package.json", r#"{"scripts":{"evil":"touch NEVER_RUN"}}"#);
        let out = f.run("evil", Scope::Tasks);
        assert_eq!(out.rows.len(), 1);
        assert!(!f.0.join("NEVER_RUN").exists());
        assert!(matches!(out.rows[0].action, Action::Task(_)));
    }
}
