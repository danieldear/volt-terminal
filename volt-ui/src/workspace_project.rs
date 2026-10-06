//! Bounded, manifest-first discovery. Never executes project configuration.
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectTask {
    pub label: String,
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    pub definition: String,
}
#[derive(Clone, Debug, Default)]
pub struct Project {
    pub root: PathBuf,
    pub tags: Vec<String>,
    pub evidence: Vec<String>,
    pub tasks: Vec<ProjectTask>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigFile {
    pub label: String,
    pub path: PathBuf,
    pub scope: String,
    pub preview: Vec<String>,
    pub mcp: bool,
    pub servers: Vec<String>,
}
pub fn read_text(path: &Path) -> Option<String> {
    let meta = std::fs::symlink_metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 65536 {
        return None;
    }
    let mut data = String::new();
    std::fs::File::open(path)
        .ok()?
        .take(65537)
        .read_to_string(&mut data)
        .ok()?;
    (data.len() <= 65536).then_some(data)
}
fn json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&read_text(path)?).ok()
}
fn toml(path: &Path) -> Option<toml::Value> {
    toml::from_str(&read_text(path)?).ok()
}
const MARKERS: &[&str] = &[
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "CMakeLists.txt",
    "CMakePresets.json",
    "compile_commands.json",
    "Makefile",
    "package.json",
    "go.mod",
    "go.work",
    "Package.swift",
];
pub(crate) fn marked(path: &Path) -> bool {
    MARKERS.iter().any(|m| path.join(m).is_file())
}
impl Project {
    fn tag(&mut self, value: &str) {
        if !self.tags.iter().any(|v| v == value) {
            self.tags.push(value.into());
        }
    }
    fn task(&mut self, argv: &[&str], definition: &str) {
        if self.tasks.len() < 32 {
            self.tasks.push(ProjectTask {
                label: argv.join(" "),
                argv: argv.iter().map(|s| (*s).into()).collect(),
                cwd: self.root.clone(),
                definition: definition.into(),
            });
        }
    }
}
pub fn discover(cwd: &Path, repo: Option<&Path>) -> Project {
    let mut root = cwd.to_path_buf();
    for dir in cwd.ancestors().take(16) {
        if marked(dir) {
            root = dir.to_path_buf();
            break;
        }
        if Some(dir) == repo || Some(dir) == dirs::home_dir().as_deref() {
            break;
        }
    }
    let mut p = Project {
        root: root.clone(),
        ..Default::default()
    };
    p.evidence = MARKERS
        .iter()
        .filter(|m| root.join(m).is_file())
        .map(|m| (*m).into())
        .collect();
    if root.join("Cargo.toml").is_file() {
        p.tag("Rust");
        p.tag("Cargo");
        if toml(&root.join("Cargo.toml")).is_some() {
            p.task(&["cargo", "build"], "Cargo.toml");
            p.task(&["cargo", "test"], "Cargo.toml");
        }
    }
    if ["pyproject.toml", "requirements.txt", "setup.py"]
        .iter()
        .any(|f| root.join(f).is_file())
    {
        p.tag("Python");
        if root.join("uv.lock").is_file() {
            p.tag("uv");
        }
        if root.join("poetry.lock").is_file() {
            p.tag("Poetry");
        }
        let py = toml(&root.join("pyproject.toml"));
        let pytest = py
            .as_ref()
            .and_then(|v| v.get("tool"))
            .and_then(|v| v.get("pytest"))
            .is_some();
        if pytest {
            if root.join("uv.lock").is_file() {
                p.task(&["uv", "run", "pytest"], "[tool.pytest] in pyproject.toml");
            } else if root.join(".venv/bin/python").is_file() {
                p.task(
                    &[".venv/bin/python", "-m", "pytest"],
                    "[tool.pytest] in pyproject.toml",
                );
            } else {
                p.task(
                    &["python3", "-m", "pytest"],
                    "[tool.pytest] in pyproject.toml",
                );
            }
        }
    }
    if let Some(pkg) = json(&root.join("package.json")) {
        p.tag(if root.join("tsconfig.json").is_file() {
            "TypeScript"
        } else {
            "JavaScript"
        });
        let deps: Vec<&str> = ["dependencies", "devDependencies"]
            .iter()
            .filter_map(|k| pkg.get(k)?.as_object())
            .flat_map(|d| d.keys().map(String::as_str))
            .collect();
        for (dep, tag) in [
            ("vite", "Vite"),
            ("react", "React"),
            ("vue", "Vue"),
            ("next", "Next.js"),
            ("svelte", "Svelte"),
            ("typescript", "TypeScript"),
        ] {
            if deps.contains(&dep) {
                p.tag(tag);
            }
        }
        let mut manager = pkg
            .get("packageManager")
            .and_then(|v| v.as_str())
            .and_then(|v| v.split('@').next())
            .filter(|v| ["npm", "pnpm", "yarn", "bun"].contains(v))
            .map(str::to_owned);
        if manager.is_none() {
            for dir in root.ancestors().take(16) {
                for (file, name) in [
                    ("pnpm-lock.yaml", "pnpm"),
                    ("yarn.lock", "yarn"),
                    ("bun.lock", "bun"),
                    ("bun.lockb", "bun"),
                    ("package-lock.json", "npm"),
                ] {
                    if dir.join(file).is_file() {
                        manager = Some(name.into());
                        break;
                    }
                }
                if manager.is_some()
                    || Some(dir) == repo
                    || Some(dir) == dirs::home_dir().as_deref()
                {
                    break;
                }
            }
        }
        let manager = manager.unwrap_or_else(|| "npm".into());
        p.tag(&manager);
        if let Some(scripts) = pkg.get("scripts").and_then(|v| v.as_object()) {
            for (name, script) in scripts.iter().take(32) {
                if name.starts_with('-') || name.chars().any(char::is_control) {
                    continue;
                }
                if let Some(definition) = script.as_str() {
                    p.task(&[&manager, "run", name], definition);
                }
            }
        }
    }
    if [
        "CMakeLists.txt",
        "CMakePresets.json",
        "compile_commands.json",
    ]
    .iter()
    .any(|f| root.join(f).is_file())
    {
        p.tag("C/C++");
        if root.join("CMakeLists.txt").is_file() {
            p.tag("CMake");
            p.task(
                &["cmake", "-S", ".", "-B", "build"],
                "Configure CMakeLists.txt into build/",
            );
            if root.join("build/CMakeCache.txt").is_file() {
                p.task(
                    &["cmake", "--build", "build"],
                    "Existing build/CMakeCache.txt",
                );
            }
        }
    }
    if root.join("Makefile").is_file() {
        p.tag("Make"); /* Dynamic targets are not executed/invented during discovery. */
    }
    if root.join("go.mod").is_file() || root.join("go.work").is_file() {
        p.tag("Go");
        if root.join("go.mod").is_file() {
            p.task(&["go", "build", "./..."], "go.mod");
            p.task(&["go", "test", "./..."], "go.mod");
        }
    }
    if root.join("Package.swift").is_file() {
        p.tag("Swift");
        p.tag("SwiftPM");
        p.task(&["swift", "build"], "Package.swift");
        p.task(&["swift", "test"], "Package.swift");
    }
    // Small source-only folders are useful too; never recursively index dependencies.
    if p.tags.is_empty() {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.take(128).filter_map(Result::ok) {
                if !entry.file_type().is_ok_and(|t| t.is_file()) {
                    continue;
                }
                match entry.path().extension().and_then(|s| s.to_str()) {
                    Some("py") => p.tag("Python"),
                    Some("rs") => p.tag("Rust"),
                    Some("c" | "h") => p.tag("C"),
                    Some("cpp" | "cc" | "cxx" | "hpp") => p.tag("C++"),
                    Some("ts" | "tsx") => p.tag("TypeScript"),
                    Some("js" | "jsx" | "mjs") => p.tag("JavaScript"),
                    Some("go") => p.tag("Go"),
                    Some("swift") => p.tag("Swift"),
                    _ => {}
                }
            }
        }
    }
    p
}

/// Explicit source paths, not merged/effective agent policy. Values in tool
/// configuration are never displayed; rules are previewed only on user expansion.
pub fn discover_configs(cwd: &Path, boundary: &Path) -> Vec<ConfigFile> {
    let mut result = Vec::new();
    for dir in cwd.ancestors().take(16) {
        for name in [
            "AGENTS.md",
            "CLAUDE.md",
            ".rules",
            ".mcp.json",
            ".codex/config.toml",
            ".claude/settings.json",
        ] {
            add_config(&mut result, dir, boundary, name);
        }
        for folder in [".rules", ".claude/rules", ".cursor/rules"] {
            if !std::fs::symlink_metadata(dir.join(folder)).is_ok_and(|m| m.is_dir()) {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir.join(folder)) {
                let mut paths: Vec<_> = entries
                    .take(32)
                    .filter_map(Result::ok)
                    .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
                    .map(|e| e.path())
                    .collect();
                paths.sort();
                for path in paths {
                    if let Ok(name) = path.strip_prefix(dir) {
                        add_config(&mut result, dir, boundary, &name.to_string_lossy());
                    }
                }
            }
        }
        if dir == boundary || result.len() >= 64 {
            break;
        }
    }
    result.truncate(64);
    result
}
fn add_config(out: &mut Vec<ConfigFile>, dir: &Path, boundary: &Path, name: &str) {
    let path = dir.join(name);
    if !path
        .canonicalize()
        .ok()
        .zip(boundary.canonicalize().ok())
        .is_some_and(|(path, root)| path.starts_with(root))
    {
        return;
    }
    let Some(text) = read_text(&path) else {
        return;
    };
    let mcp = name == ".mcp.json";
    let scope = if dir == boundary {
        "Project root".into()
    } else {
        dir.strip_prefix(boundary)
            .unwrap_or(dir)
            .display()
            .to_string()
    };
    let mut servers = Vec::new();
    let preview = if mcp {
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(v) => {
                if let Some(map) = v.get("mcpServers").and_then(|v| v.as_object()) {
                    servers.extend(map.keys().take(20).cloned());
                }
                vec![
                    "Configuration found; connection unknown".into(),
                    "Commands, URLs and secrets hidden".into(),
                ]
            }
            Err(_) => vec!["Invalid JSON; not loaded".into()],
        }
    } else if name.ends_with(".toml") || name.ends_with(".json") {
        vec![
            "Configuration found, not loaded".into(),
            "Values hidden to protect credentials".into(),
        ]
    } else {
        text.lines()
            .take(24)
            .map(|s| super::workspace_panel::safe_label(s, 240))
            .collect()
    };
    out.push(ConfigFile {
        label: name.into(),
        path,
        scope,
        preview,
        mcp,
        servers,
    });
}

/// All dynamic strings are argv, not shell source. The task still intentionally
/// executes project build scripts, only after the user's explicit Run action.
pub fn task_shell_args(task: &ProjectTask, shell: &str) -> Vec<String> {
    let mut args = vec!["-c".into(),
        "cd -- \"$1\" || exit; shell=$2; shift 2; \"$@\"; result=$?; printf '\\nTask exited with status %s\\n' \"$result\"; exec \"$shell\"".into(),
        "volt-task".into(), task.cwd.to_string_lossy().into_owned(), shell.into()];
    args.extend(task.argv.clone());
    args
}
