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
    /// Same order as `volt_renderer::search_palette::SCOPES`.
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
        volt_renderer::search_palette::SCOPES[self.index()]
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
    /// Matched characters in `title`, as char ranges.
    pub hits: Vec<(usize, usize)>,
    pub action: Action,
    pub score: i64,
}
#[derive(Clone, Debug, Default)]
pub struct SearchOutput {
    pub rows: Vec<ResultRow>,
    pub status: String,
    pub root: PathBuf,
    /// The trimmed query these rows answer.
    pub query: String,
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
        self.output.rows.clear();
        self.output.status = "Searching…".into();
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
    pub fn view(&self) -> Option<SearchView> {
        self.visible.then(|| {
            let row = self.output.rows.get(self.selected);
            let hint = match row.map(|r| &r.action) {
                Some(Action::File { .. }) => "Enter to open",
                Some(Action::Terminal { .. }) => "Enter to jump to line",
                _ => "Esc to close",
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
                        kind: kind_label(r.kind).into(),
                        title: r.title.clone(),
                        detail: r.detail.clone(),
                        hits: r.hits.clone(),
                    })
                    .collect(),
                status: self.output.status.clone(),
                hint: hint.into(),
                root: self.output.root.display().to_string(),
            }
        })
    }
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "OUTPUT" => "Output",
        "FILE" => "File",
        "TEXT" => "Text",
        "BRANCH" => "Branch",
        "WORKTREE" => "Worktree",
        "TASK" => "Task",
        _ => "",
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

/// Lowercase each char to exactly one char, so indices stay aligned with `s`.
fn folded(s: &str) -> Vec<char> {
    s.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

fn find_from(hay: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.is_empty() || needle.len() > hay.len() {
        return None;
    }
    (from..=hay.len() - needle.len()).find(|&i| hay[i..i + needle.len()] == *needle)
}

/// Every case-insensitive occurrence of `q` in `s`, as char ranges.
pub fn literal_hits(q: &str, s: &str) -> Vec<(usize, usize)> {
    let (hay, needle) = (folded(s), folded(q));
    let mut hits = Vec::new();
    let mut at = 0;
    while let Some(i) = find_from(&hay, &needle, at) {
        hits.push((i, i + needle.len()));
        at = i + needle.len();
        if hits.len() >= 64 {
            break;
        }
    }
    hits
}

/// Greedy subsequence positions of `q` in `s` from char `start`, merged into ranges.
fn subsequence(q: &[char], s: &[char], start: usize) -> Option<Vec<(usize, usize)>> {
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut at = start;
    for c in q {
        let i = (at..s.len()).find(|&i| s[i] == *c)?;
        match ranges.last_mut() {
            Some(last) if last.1 == i => last.1 = i + 1,
            _ => ranges.push((i, i + 1)),
        }
        at = i + 1;
    }
    Some(ranges)
}

/// Rank a relative path for a file-name query, with the characters to
/// highlight. Substrings beat scattered letters, and the file name beats the
/// directories, so "volt" never prefers `venue/FloorList.vue` over `volt.rs`.
pub fn path_match(q: &str, path: &str) -> Option<(i64, Vec<(usize, usize)>)> {
    let (p, n) = (folded(path), folded(q));
    if n.is_empty() {
        return None;
    }
    let base = p.iter().rposition(|c| *c == '/').map_or(0, |i| i + 1);
    let length_penalty = p.len() as i64 / 4;
    if let Some(i) = find_from(&p, &n, base) {
        let prefix = if i == base { 200 } else { 0 };
        return Some((1000 + prefix - length_penalty, vec![(i, i + n.len())]));
    }
    if let Some(i) = (0..base)
        .rev()
        .find(|&i| find_from(&p[..base], &n, i) == Some(i))
    {
        return Some((600 - length_penalty, vec![(i, i + n.len())]));
    }
    if let Some(ranges) = subsequence(&n, &p, base) {
        return Some((300 - 10 * ranges.len() as i64 - length_penalty, ranges));
    }
    // Scattered across directories: only when the letters stay close together.
    let ranges = subsequence(&n, &p, 0)?;
    let span = ranges.last()?.1 - ranges.first()?.0;
    (span <= n.len() * 2 + 2).then(|| (100 - 10 * ranges.len() as i64 - length_penalty, ranges))
}

/// Highlight for a short label (branch, task): substring if present, else letters.
fn label_hits(q: &str, label: &str) -> Vec<(usize, usize)> {
    let hits = literal_hits(q, label);
    if !hits.is_empty() {
        return hits;
    }
    subsequence(&folded(q), &folded(label), 0).unwrap_or_default()
}

fn literal(q: &str, s: &str) -> bool {
    !q.is_empty() && s.to_lowercase().contains(&q.to_lowercase())
}
fn clean(s: &str, max: usize) -> String {
    crate::workspace_panel::safe_label(s, max)
}
/// "Modified 3 hours ago", coarse on purpose.
fn modified_ago(at: std::time::SystemTime) -> String {
    let secs = at.elapsed().map_or(0, |d| d.as_secs());
    let (n, unit) = match secs {
        0..60 => return "Modified just now".into(),
        60..3600 => (secs / 60, "minute"),
        3600..86400 => (secs / 3600, "hour"),
        86400..2_592_000 => (secs / 86400, "day"),
        2_592_000..31_536_000 => (secs / 2_592_000, "month"),
        _ => (secs / 31_536_000, "year"),
    };
    format!("Modified {n} {unit}{} ago", if n == 1 { "" } else { "s" })
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

/// The enclosing Git checkout, so a search from `crate/src` covers the whole
/// workspace. Stops at the home directory: a dotfiles repo in `~` is not a project.
fn repo_root(cwd: &Path) -> Option<PathBuf> {
    let home = dirs::home_dir();
    for dir in cwd.ancestors().take(32) {
        if Some(dir) == home.as_deref() {
            return None;
        }
        if dir.join(".git").exists() {
            return Some(dir.to_path_buf());
        }
    }
    None
}

/// Walking the home directory or `/` would take seconds and return noise.
fn too_broad(root: &Path) -> bool {
    root.parent().is_none() || dirs::home_dir().is_some_and(|home| root == home)
}

/// Output rows listed in the panel; Enter on one highlights every match in the terminal.
const OUTPUT_ROWS: usize = 60;
/// Text copied from the scrollback for one search.
const OUTPUT_BUDGET: usize = 32 * 1024 * 1024;

pub fn search(input: SearchInput, cancel: &AtomicU64, generation: u64) -> SearchOutput {
    let start = Instant::now();
    let stopped =
        || cancel.load(Ordering::Relaxed) != generation || start.elapsed() > Duration::from_secs(2);
    let q = input.query.trim().to_string();
    let q = q.as_str();
    let mut output = SearchOutput {
        root: input.cwd.clone().unwrap_or_default(),
        query: q.to_string(),
        ..Default::default()
    };
    if q.is_empty() {
        // Opening/clearing the palette must not walk disk, discover manifests,
        // run Git, or contend with the PTY. Reuse only already-known scope.
        if let Some(snapshot) = &input.snapshot {
            if !snapshot.project.root.as_os_str().is_empty() {
                output.root = snapshot.project.root.clone();
            }
        }
        output.status = "Type to search".into();
        return output;
    }
    let mut limited = false;
    let mut terminal_busy = false;
    let mut output_lines = 0;
    let mut notes: Vec<&str> = Vec::new();
    if input.scope.includes(Scope::Terminal) {
        // A bounded snapshot on the worker, never disk I/O or matching under the PTY lock.
        let snapshot = input.performer.try_lock().ok().map(|p| {
            let g = &p.grid;
            // The full scrollback, like Find, within a byte budget.
            let total = g.scrollback_len() + g.rows;
            let mut bytes = 0;
            let rows = (0..total)
                .take_while(|_| !stopped())
                .map_while(|row| {
                    if bytes > OUTPUT_BUDGET {
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
            let truncated = rows.len() < total;
            (rows, p.alternate_screen_active(), truncated)
        });
        if let Some((rows, alternate, truncated)) = snapshot {
            limited |= truncated;
            let mut found = 0;
            // Newest output first; count every matching line, list the latest.
            for idx in (0..rows.len()).rev() {
                let (row, text) = &rows[idx];
                if !literal(q, text) {
                    continue;
                }
                found += 1;
                if found > OUTPUT_ROWS {
                    limited = true;
                    continue;
                }
                let title = clean(text.trim(), 150);
                push(
                    &mut output.rows,
                    ResultRow {
                        kind: "OUTPUT",
                        hits: literal_hits(q, &title),
                        title,
                        detail: format!(
                            "{}, line {}",
                            if alternate {
                                "Current full-screen app"
                            } else {
                                "Terminal output"
                            },
                            row + 1
                        ),
                        action: Action::Terminal {
                            text: text.clone(),
                            alternate,
                            row: *row,
                        },
                        score: 100,
                    },
                );
            }
            output_lines = found;
        } else {
            terminal_busy = true;
            if input.scope == Scope::Terminal {
                output.status = "The terminal is busy. Type again to refresh.".into();
            }
        }
    }
    if stopped() {
        return output;
    }
    if input.scope == Scope::Terminal {
        if output.status.is_empty() {
            output.status = match output_lines {
                0 => "No matches in this terminal's output".into(),
                1 => "1 matching line in this terminal's output".into(),
                n if n > OUTPUT_ROWS => {
                    format!("{n} matching lines; showing the latest {OUTPUT_ROWS}. Enter shows them all in the terminal.")
                }
                n => format!("{n} matching lines in this terminal's output"),
            };
        }
        return output;
    }
    let mut project = input
        .snapshot
        .as_ref()
        .map(|s| s.project.clone())
        .unwrap_or_default();
    if let Some(cwd) = input.cwd.as_ref() {
        let repo = repo_root(cwd);
        if project.root.as_os_str().is_empty() {
            project = discover(cwd, repo.as_deref());
        }
        output.root = repo.unwrap_or_else(|| project.root.clone());
    }
    if input.scope.includes(Scope::Tasks) {
        for task in &project.tasks {
            if let Some(score) = fuzzy_score(q, &task.label) {
                push(
                    &mut output.rows,
                    ResultRow {
                        kind: "TASK",
                        hits: label_hits(q, &task.label),
                        title: task.label.clone(),
                        detail: format!("Defined in {}", clean(&task.definition, 80)),
                        score,
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
                            hits: label_hits(q, &b.name),
                            title: b.name.clone(),
                            detail: if b.current {
                                "Current branch".into()
                            } else if b.upstream.is_empty() {
                                "Local branch".into()
                            } else {
                                format!("Tracks {}", b.upstream)
                            },
                            score,
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
                            hits: label_hits(q, &w.branch),
                            title: w.branch.clone(),
                            detail: if w.prunable {
                                format!("Folder missing: {}", w.path.display())
                            } else {
                                w.path.display().to_string()
                            },
                            score,
                            action: Action::Worktree(w.path.clone()),
                        },
                    );
                }
            }
        }
    }
    let wants_files = input.scope.includes(Scope::Files) || input.scope.includes(Scope::Text);
    if wants_files && too_broad(&output.root) {
        notes.push("Files and text are searched inside a project. cd into one first.");
    } else if wants_files && !output.root.as_os_str().is_empty() && !stopped() {
        let root = output.root.clone();
        let mut walker = ignore::WalkBuilder::new(&root);
        walker
            .hidden(true)
            .follow_links(false)
            .require_git(false)
            .max_depth(Some(24))
            .max_filesize(Some(1024 * 1024))
            .sort_by_file_name(|a, b| a.cmp(b))
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
                if let Some((score, hits)) = path_match(q, &label) {
                    let modified = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .map(modified_ago);
                    file_rows.push(ResultRow {
                        kind: "FILE",
                        title: label.clone(),
                        hits,
                        detail: modified.unwrap_or_else(|| "File".into()),
                        score,
                        action: Action::File {
                            path: path.into(),
                            line: 1,
                        },
                    });
                }
            }
            if input.scope.includes(Scope::Text) && text_rows.len() < 100 {
                if let Some(text) = read_source(path, &root) {
                    total_bytes += text.len();
                    for (line, txt) in text.lines().enumerate() {
                        if literal(q, txt) {
                            let title = clean(txt.trim(), 150);
                            text_rows.push(ResultRow {
                                kind: "TEXT",
                                hits: literal_hits(q, &title),
                                title,
                                detail: format!("{label}:{}", line + 1),
                                score: 0,
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
        // Text matches stay in reading order: file by file, line by line.
        output.rows.extend(text_rows);
    }
    // Keep categories grouped; scores compare candidates within their own
    // provider, and the stable sort keeps each provider's own order on ties.
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
    });
    limited |= stopped();
    output.rows.truncate(300);
    if output.status.is_empty() {
        let n = output.rows.len();
        output.status = if limited {
            format!("Showing the first {n} results. Type more to narrow them down.")
        } else if n == 1 {
            "1 result, searched locally".into()
        } else {
            format!("{n} results, searched locally")
        };
    }
    if terminal_busy {
        notes.push("Terminal output was busy; type again to include it.");
    }
    if !notes.is_empty() {
        let notes = notes.join(" ");
        output.status = if output.rows.is_empty() {
            notes
        } else {
            format!("{}. {notes}", output.status)
        };
    }
    output
}
/// Containment and regular-file checks shared by reading and opening.
pub fn contained_file(path: &Path, root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return false;
    };
    if rel
        .components()
        .any(|c| excluded(&c.as_os_str().to_string_lossy()))
    {
        return false;
    }
    if !std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file()) {
        return false;
    }
    match (path.canonicalize(), root.canonicalize()) {
        (Ok(canonical), Ok(root)) => canonical.starts_with(root),
        _ => false,
    }
}
/// Revalidate containment and regular-file type before every read.
pub fn read_source(path: &Path, root: &Path) -> Option<String> {
    if !contained_file(path, root) {
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
                assert_eq!(output.status, "Type to search");
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
        assert!(output.status.contains("Terminal output was busy"));
        let input = f.input("needle", Scope::Terminal);
        let performer = input.performer.clone();
        let _busy = performer.lock().unwrap();
        let output = search(input, &AtomicU64::new(0), 0);
        assert!(output.rows.is_empty());
        assert_eq!(
            output.status,
            "The terminal is busy. Type again to refresh."
        );
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
        assert!(o.status.starts_with("Showing the first"), "{}", o.status);
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
    fn file_names_beat_scattered_letters_and_highlight_what_matched() {
        let (score, hits) = path_match("volt", "src/volt.rs").unwrap();
        assert_eq!(hits, vec![(4, 8)]);
        assert!(path_match("volt", "temp/venue/FloorList.vue").is_none());
        let in_dir = path_match("volt", "volt/src/main.rs").unwrap().0;
        assert!(score > in_dir, "file name beats directory");
        assert!(in_dir > path_match("wssr", "src/workspace_search.rs").unwrap().0);
        // Letters inside the file name still match, highlighted one by one.
        assert_eq!(
            path_match("wssr", "src/workspace_search.rs").unwrap().1,
            vec![(4, 5), (8, 9), (14, 15), (17, 18)]
        );
        assert_eq!(literal_hits("ab", "xABxab"), vec![(1, 3), (4, 6)]);
        assert_eq!(literal_hits("É", "café"), vec![(3, 4)]);
    }
    #[test]
    fn searches_cover_the_whole_checkout_from_a_subfolder() {
        let f = Fixture::new();
        std::fs::create_dir_all(f.0.join(".git")).unwrap();
        f.put("crate-a/Cargo.toml", "[package]\nname = \"a\"\n");
        f.put("crate-a/src/lib.rs", "// a");
        f.put("crate-b/src/renderer.rs", "// b");
        let input = SearchInput {
            cwd: Some(f.0.join("crate-a/src")),
            ..f.input("renderer", Scope::Files)
        };
        let out = search(input, &AtomicU64::new(0), 0);
        assert_eq!(out.root, f.0);
        assert_eq!(out.rows.len(), 1);
        assert_eq!(out.rows[0].title, "crate-b/src/renderer.rs");
    }
    #[test]
    fn home_and_filesystem_root_are_not_walked() {
        assert!(too_broad(Path::new("/")));
        if let Some(home) = dirs::home_dir() {
            assert!(too_broad(&home));
            assert!(!too_broad(&home.join("project")));
        }
    }
    #[test]
    fn text_matches_stay_in_reading_order() {
        let f = Fixture::new();
        f.put("b.rs", "needle two\n");
        f.put("a.rs", "zzz\n\"needle\" quoted\nneedle again\n");
        let out = f.run("needle", Scope::Text);
        let details: Vec<_> = out.rows.iter().map(|r| r.detail.as_str()).collect();
        assert_eq!(details, ["a.rs:2", "a.rs:3", "b.rs:1"]);
        assert_eq!(out.rows[0].hits, vec![(1, 7)]);
        assert_eq!(out.query, "needle");
    }
    #[test]
    fn only_regular_files_inside_the_project_can_be_opened() {
        let f = Fixture::new();
        f.put("src/main.rs", "fn main() {}");
        f.put(".env", "SECRET=1");
        let outside = Fixture::new();
        outside.put("x.rs", "secret");
        assert!(contained_file(&f.0.join("src/main.rs"), &f.0));
        assert!(!contained_file(&f.0.join(".env"), &f.0));
        assert!(!contained_file(&f.0.join("src"), &f.0));
        assert!(!contained_file(&outside.0.join("x.rs"), &f.0));
    }
    #[test]
    fn terminal_search_covers_the_full_scrollback() {
        let f = Fixture::new();
        let input = f.input("needle", Scope::Terminal);
        {
            let mut p = input.performer.lock().unwrap();
            let mut text = String::from("needle at the very top\r\n");
            for i in 0..3000 {
                text.push_str(&format!("line {i}\r\n"));
            }
            vte::Parser::new().advance(&mut *p, text.as_bytes());
            assert!(
                p.grid.scrollback_len() > 2000,
                "fixture must exceed the old limit"
            );
        }
        let o = search(input, &AtomicU64::new(0), 0);
        assert_eq!(o.rows.len(), 1);
        assert!(matches!(o.rows[0].action, Action::Terminal { row: 0, .. }));
        assert_eq!(o.status, "1 matching line in this terminal's output");
    }
    #[test]
    fn many_output_matches_list_the_latest_and_count_them_all() {
        let f = Fixture::new();
        let input = f.input("hit", Scope::Terminal);
        {
            let mut p = input.performer.lock().unwrap();
            let text: String = (0..100).map(|i| format!("hit {i}\r\n")).collect();
            vte::Parser::new().advance(&mut *p, text.as_bytes());
        }
        let o = search(input, &AtomicU64::new(0), 0);
        assert_eq!(o.rows.len(), OUTPUT_ROWS);
        assert_eq!(o.rows[0].title, "hit 99", "newest first");
        assert!(o.status.starts_with("100 matching lines"), "{}", o.status);
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
