use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use volt_config::tasks::{self, load, save};
use volt_ui::{
    tasks::detected_task,
    workspace_git::{parse_pr, parse_worktrees, valid_pr_url, PrState},
    workspace_project::*,
};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "volt-discovery-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&p).unwrap();
        Self(p.canonicalize().unwrap())
    }
    fn put(&self, path: &str, text: &str) {
        let p = self.0.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn detected_cargo_command_becomes_a_trusted_reusable_task_only_after_import() {
    let project = Temp::new();
    let trust_store = Temp::new();
    project.put("Cargo.toml", "[package]\nname='sample'\nversion='0.1.0'\n");
    let found = discover(&project.0, Some(&project.0));
    let root = tasks::project_root(&project.0, None).unwrap();
    let before = load(&root, Some(&trust_store.0));
    assert!(!tasks::tasks_file(&root).exists());
    let build = detected_task(&found.tasks[0], &before).unwrap().unwrap();
    assert_eq!(build.run, "cargo build");
    assert!(build.confirm);
    save(&root, None, Some(&build), Some(&trust_store.0)).unwrap();
    let after = load(&root, Some(&trust_store.0));
    assert!(after.trusted);
    assert_eq!(after.tasks, vec![build]);
    assert!(detected_task(&found.tasks[0], &after).unwrap().is_none());
}
#[test]
fn monorepo_scopes_tasks_and_inherits_package_manager() {
    let t = Temp::new();
    t.put("Cargo.toml", "[workspace]\nmembers=[]");
    t.put("pnpm-lock.yaml", "");
    t.put("web/package.json",r#"{"scripts":{"dev":"vite","build":"vite build"},"devDependencies":{"vite":"*","typescript":"*"},"dependencies":{"react":"*"}}"#);
    std::fs::create_dir(t.0.join("web/src")).unwrap();
    let p = discover(&t.0.join("web/src"), Some(&t.0));
    assert_eq!(p.root, t.0.join("web"));
    assert!(p.tags.contains(&"Vite".into()));
    assert!(p.tags.contains(&"TypeScript".into()));
    assert_eq!(p.tasks.len(), 2);
    assert!(p
        .tasks
        .iter()
        .all(|t| t.argv[0] == "pnpm" && t.cwd.ends_with("web")));
    assert!(!p.tags.contains(&"Rust".into()));
}
#[test]
fn languages_and_real_tasks_without_executing_configuration() {
    let t = Temp::new();
    t.put(
        "python/pyproject.toml",
        "[tool.pytest.ini_options]\ntestpaths=['tests']",
    );
    t.put("python/uv.lock", "");
    let p = discover(&t.0.join("python"), Some(&t.0));
    assert!(p.tags.contains(&"Python".into()));
    assert_eq!(p.tasks[0].argv, vec!["uv", "run", "pytest"]);
    t.put("cpp/CMakeLists.txt", "project(example)");
    let c = discover(&t.0.join("cpp"), Some(&t.0));
    assert!(c.tags.contains(&"CMake".into()));
    assert_eq!(c.tasks.len(), 1);
    t.put(
        "rust/Cargo.toml",
        "[package]\nname='fixture'\nversion='0.1.0'",
    );
    assert_eq!(discover(&t.0.join("rust"), Some(&t.0)).tasks.len(), 2);
    t.put("go/go.mod", "module example.com/demo");
    assert_eq!(discover(&t.0.join("go"), Some(&t.0)).tasks.len(), 2);
    t.put("swift/Package.swift", "fatalError(\"never execute\")");
    assert!(discover(&t.0.join("swift"), Some(&t.0))
        .tags
        .contains(&"Swift".into()));
    t.put("standalone/main.cpp", "int main(){}");
    assert!(discover(&t.0.join("standalone"), Some(&t.0))
        .tags
        .contains(&"C++".into()));
}
#[test]
fn broken_manifest_never_invents_scripts_or_executes_dynamic_values() {
    let t = Temp::new();
    t.put("package.json", "{bad json");
    assert!(discover(&t.0, Some(&t.0)).tasks.is_empty());
    t.put(
        "package.json",
        r#"{"scripts":{"--bad":"echo nope","dev":"touch SHOULD_NOT_EXIST"}}"#,
    );
    let p = discover(&t.0, Some(&t.0));
    assert_eq!(p.tasks.len(), 1);
    assert!(!t.0.join("SHOULD_NOT_EXIST").exists());
}
#[test]
fn config_scopes_and_mcp_preview_do_not_expose_credentials() {
    let t = Temp::new();
    t.put("AGENTS.md", "Root instructions");
    t.put("sub/.rules", "Nested instructions");
    t.put("sub/.mcp.json",r#"{"mcpServers":{"example":{"command":"evil","env":{"TOKEN":"supersecret"},"url":"https://secret"}}}"#);
    let list = discover_configs(&t.0.join("sub"), &t.0);
    assert!(list.iter().any(|c| c.label == ".rules" && c.scope == "sub"));
    assert!(list.iter().any(|c| c.label == "AGENTS.md"));
    let m = list.iter().find(|c| c.mcp).unwrap();
    assert_eq!(m.servers, vec!["example"]);
    let display = m.preview.join(" ");
    for secret in ["supersecret", "evil", "https://secret"] {
        assert!(!display.contains(secret));
    }
}
#[test]
fn command_arguments_are_not_shell_source() {
    let t = Temp::new();
    let dangerous = "$(touch INJECTED); echo bad";
    let task = ProjectTask {
        label: "test".into(),
        argv: vec!["/bin/echo".into(), dangerous.into()],
        cwd: t.0.clone(),
        definition: String::new(),
    };
    let output = Command::new("/bin/sh")
        .args(task_shell_args(&task, "/usr/bin/true"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains(dangerous));
    assert!(!t.0.join("INJECTED").exists());
}
#[test]
fn github_parser_distinguishes_missing_unavailable_draft_and_checks() {
    assert!(matches!(
        parse_pr(br#"{"createdBy":[],"needsReview":[]}"#),
        PrState::None
    ));
    assert!(matches!(
        parse_pr(br#"{"currentBranch":null}"#),
        PrState::None
    ));
    assert!(matches!(parse_pr(b"garbage"), PrState::Unavailable));
    let PrState::Found(pr)=parse_pr(br#"{"currentBranch":{"number":12,"title":"test","url":"https://github.com/owner/repo/pull/12","isDraft":true,"statusCheckRollup":[{"name":"CI","status":"COMPLETED","conclusion":"FAILURE"},{"context":"lint","state":"SUCCESS"}]}}"#) else{panic!("expected PR")};
    assert_eq!(pr.state, "DRAFT");
    assert_eq!(pr.checks[0].1, "FAILURE");
    assert_eq!(pr.checks[1].1, "SUCCESS");
    for url in [
        "file:///tmp/x",
        "https://github.com.evil.test/o/r/pull/1",
        "https://github.com/o/r/pull/1?secret",
        "javascript:alert(1)",
    ] {
        assert!(!valid_pr_url(url));
    }
}
#[test]
fn nul_worktree_paths_and_flags() {
    let w=parse_worktrees(b"worktree /tmp/space and\nnewline\0HEAD abc\0branch refs/heads/main\0locked reason\0\0worktree /tmp/other\0detached\0prunable missing\0\0");
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].path, Path::new("/tmp/space and\nnewline"));
    assert!(w[0].locked);
    assert!(w[1].prunable);
}
#[test]
fn actual_git_worktree_and_branch_discovery() {
    let t = Temp::new();
    let repo = t.0.join("repo");
    let other = t.0.join("other");
    std::fs::create_dir(&repo).unwrap();
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .current_dir(&repo)
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-b", "main"]);
    git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "--allow-empty",
        "-m",
        "fixture",
    ]);
    git(&["worktree", "add", "-b", "feature", other.to_str().unwrap()]);
    let d = volt_ui::workspace_git::discover(&other);
    assert_eq!(d.worktrees.len(), 2);
    assert!(d.branches.iter().any(|b| b.name == "feature" && b.current));
    assert!(d.upstream.is_empty());
}
#[test]
fn placement_setting_defaults_and_parses() {
    use volt_config::{config::WorkspaceLayout, Config};
    assert_eq!(
        toml::from_str::<Config>("").unwrap().workspace.layout,
        WorkspaceLayout::Docked
    );
    assert_eq!(
        toml::from_str::<Config>("[workspace]\nlayout='floating'")
            .unwrap()
            .workspace
            .layout,
        WorkspaceLayout::Floating
    );
    assert!(toml::from_str::<Config>("[workspace]\nlayout='invalid'").is_err());
}

#[cfg(unix)]
#[test]
fn configuration_symlinks_cannot_escape_workspace() {
    let t = Temp::new();
    let outside = Temp::new();
    outside.put("secret.rules", "private contents");
    std::os::unix::fs::symlink(outside.0.join("secret.rules"), t.0.join(".rules")).unwrap();
    assert!(discover_configs(&t.0, &t.0).is_empty());
    std::fs::create_dir(t.0.join("sub")).unwrap();
    std::os::unix::fs::symlink(&outside.0, t.0.join("sub/.rules")).unwrap();
    assert!(discover_configs(&t.0.join("sub"), &t.0).is_empty());
}
