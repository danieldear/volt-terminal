//! Read-only Git and GitHub facts. GitHub uses gh's current-branch resolver,
//! including fork/default-repository handling, never a branch-name-only search.
use crate::workspace_panel::{bounded_output, git};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Clone, Debug, Default)]
pub struct Branch {
    pub name: String,
    pub upstream: String,
    pub tracking: String,
    pub current: bool,
}
#[derive(Clone, Debug, Default)]
pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub locked: bool,
    pub prunable: bool,
}
#[derive(Clone, Debug, Default)]
pub struct GitDetails {
    pub branches: Vec<Branch>,
    pub worktrees: Vec<Worktree>,
    pub upstream: String,
    pub ahead_behind: Option<(u64, u64)>,
    pub github: bool,
    pub head: String,
}
#[derive(Clone, Debug)]
pub struct PullRequest {
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub review: String,
    pub checks: Vec<(String, String)>,
}
#[derive(Clone, Debug)]
pub enum PrState {
    Found(PullRequest),
    None,
    Unavailable,
}
pub fn github_remote(raw: &str) -> bool {
    raw.starts_with("https://github.com/")
        || raw.starts_with("git@github.com:")
        || raw.starts_with("ssh://git@github.com/")
}
pub fn discover(root: &Path) -> GitDetails {
    let mut d = GitDetails::default();
    if let Some(bytes) = git(
        root,
        &[
            "for-each-ref",
            "--count=64",
            "--sort=-committerdate",
            "--format=%(HEAD)%00%(refname:short)%00%(upstream:short)%00%(upstream:track)",
            "refs/heads/",
        ],
    ) {
        for line in bytes.split(|b| *b == b'\n') {
            let f: Vec<_> = line
                .split(|b| *b == 0)
                .map(|b| String::from_utf8_lossy(b).into_owned())
                .collect();
            if f.len() == 4 {
                d.branches.push(Branch {
                    current: f[0] == "*",
                    name: f[1].clone(),
                    upstream: f[2].clone(),
                    tracking: f[3].clone(),
                });
            }
        }
    }
    d.upstream = git(
        root,
        &[
            "rev-parse",
            "--abbrev-ref",
            "--symbolic-full-name",
            "@{upstream}",
        ],
    )
    .map(|b| String::from_utf8_lossy(&b).trim().into())
    .unwrap_or_default();
    d.head = git(root, &["rev-parse", "HEAD"])
        .map(|b| String::from_utf8_lossy(&b).trim().into())
        .unwrap_or_default();
    if !d.upstream.is_empty() {
        if let Some(bytes) = git(
            root,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        ) {
            let text = String::from_utf8_lossy(&bytes);
            let mut n = text.split_whitespace().filter_map(|s| s.parse().ok());
            d.ahead_behind = n.next().zip(n.next());
        }
    }
    if let Some(bytes) = git(root, &["worktree", "list", "--porcelain", "-z"]) {
        d.worktrees = parse_worktrees(&bytes);
    }
    if let Some(bytes) = git(root, &["config", "--get-regexp", r"remote\..*\.url"]) {
        d.github = String::from_utf8_lossy(&bytes).lines().any(|line| {
            line.split_once(' ')
                .is_some_and(|(_, url)| github_remote(url))
        });
    }
    d
}
pub fn parse_worktrees(bytes: &[u8]) -> Vec<Worktree> {
    let mut trees = Vec::new();
    let mut current = Worktree::default();
    for field in bytes.split(|b| *b == 0) {
        if field.is_empty() {
            if !current.path.as_os_str().is_empty() {
                trees.push(std::mem::take(&mut current));
            }
            continue;
        }
        let s = String::from_utf8_lossy(field);
        if let Some(p) = s.strip_prefix("worktree ") {
            current.path = PathBuf::from(p);
        } else if let Some(b) = s.strip_prefix("branch refs/heads/") {
            current.branch = b.into();
        } else if s == "detached" {
            current.branch = "Detached HEAD".into();
        } else if s == "bare" {
            current.prunable = true;
        } else if s.starts_with("locked") {
            current.locked = true;
        } else if s.starts_with("prunable") {
            current.prunable = true;
        }
        if trees.len() >= 32 {
            break;
        }
    }
    if !current.path.as_os_str().is_empty() && trees.len() < 32 {
        trees.push(current);
    }
    trees
}
pub fn gh_installed() -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join("gh").is_file()))
}
pub fn collect_pr(root: &Path) -> PrState {
    let bytes = bounded_output(
        Command::new("gh")
            .current_dir(root)
            .args([
                "pr",
                "status",
                "--json",
                "number,title,url,state,isDraft,reviewDecision,statusCheckRollup",
            ])
            .env("GH_PROMPT_DISABLED", "1")
            .env("GH_PAGER", "cat")
            .env("GH_HTTP_TIMEOUT", "5"),
    );
    bytes
        .as_deref()
        .map(parse_pr)
        .unwrap_or(PrState::Unavailable)
}
pub fn valid_pr_url(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let p: Vec<_> = path.split('/').collect();
    p.len() == 4
        && !p[0].is_empty()
        && !p[1].is_empty()
        && p[0].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && p[1]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && p[2] == "pull"
        && p[3].parse::<u64>().is_ok()
}
pub fn parse_pr(bytes: &[u8]) -> PrState {
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return PrState::Unavailable;
    };
    let Some(pr) = v.get("currentBranch") else {
        // gh omits currentBranch (rather than emitting null) when none exists.
        return if v.get("createdBy").is_some_and(|v| v.is_array())
            && v.get("needsReview").is_some_and(|v| v.is_array())
        {
            PrState::None
        } else {
            PrState::Unavailable
        };
    };
    if pr.is_null() {
        return PrState::None;
    }
    let Some(number) = pr.get("number").and_then(|v| v.as_u64()) else {
        return PrState::Unavailable;
    };
    let Some(url) = pr
        .get("url")
        .and_then(|v| v.as_str())
        .filter(|v| valid_pr_url(v))
    else {
        return PrState::Unavailable;
    };
    let checks = pr
        .get("statusCheckRollup")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .take(30)
                .map(|c| {
                    let name = c
                        .get("name")
                        .or_else(|| c.get("context"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("Check");
                    let status = c
                        .get("conclusion")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .or_else(|| {
                            c.get("state")
                                .or_else(|| c.get("status"))
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or("UNKNOWN");
                    (name.into(), status.into())
                })
                .collect()
        })
        .unwrap_or_default();
    PrState::Found(PullRequest {
        number,
        title: pr
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Pull request")
            .into(),
        url: url.into(),
        state: if pr.get("isDraft").and_then(|v| v.as_bool()) == Some(true) {
            "DRAFT".into()
        } else {
            pr.get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("UNKNOWN")
                .into()
        },
        review: pr
            .get("reviewDecision")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or("No review decision")
            .into(),
        checks,
    })
}
