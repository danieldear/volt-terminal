//! Bundled Meow: adapted from the user's local prompt. No network or plugins.
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
fn safe(s: &str) -> String {
    s.chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(120)
        .collect()
}
#[derive(Debug, Default, PartialEq)]
struct GitStatus {
    branch: String,
    staged: bool,
    dirty: bool,
    ahead: usize,
    behind: usize,
}
fn parse_git(text: &str) -> Option<GitStatus> {
    let header = text.lines().next()?.strip_prefix("## ")?;
    let branch = header.split("...").next()?.split(" [").next()?;
    let branch = branch
        .strip_prefix("No commits yet on ")
        .or_else(|| branch.strip_prefix("Initial commit on "))
        .unwrap_or(branch);
    let mut status = GitStatus {
        branch: safe(branch),
        ..Default::default()
    };
    if let Some((_, counts)) = header.split_once('[') {
        for count in counts.trim_end_matches(']').split(',') {
            let count = count.trim();
            if let Some(n) = count.strip_prefix("ahead ").and_then(|n| n.parse().ok()) {
                status.ahead = n;
            }
            if let Some(n) = count.strip_prefix("behind ").and_then(|n| n.parse().ok()) {
                status.behind = n;
            }
        }
    }
    for line in text.lines().skip(1) {
        let b = line.as_bytes();
        if b.len() >= 2 && !b.starts_with(b"??") {
            status.staged |= b[0] != b' ';
            status.dirty |= b[1] != b' ';
        }
    }
    Some(status)
}
fn git() -> Option<GitStatus> {
    let mut child = Command::new("git")
        .args([
            "-c",
            "core.fsmonitor=false",
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "--branch",
            "--untracked-files=no",
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take()?;
    // Drain in parallel: never block on a full pipe. Retain only bounded data.
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.take(8192).read_to_string(&mut text);
        let _ = tx.send(text);
    });
    let start = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s.success(),
            Ok(None) if start.elapsed() < Duration::from_millis(100) => {
                std::thread::sleep(Duration::from_millis(2))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let text = rx.recv_timeout(Duration::from_millis(10)).ok()?;
    if !success {
        return None;
    }
    parse_git(&text)
}
fn literal(shell: &str, s: &str) -> String {
    let s = safe(s);
    match shell {
        "zsh" => s.replace('%', "%%"),
        "bash" => s.replace('\\', "\\\\"),
        _ => s,
    }
}
fn clock() -> String {
    #[cfg(unix)]
    {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as libc::time_t)
            .unwrap_or(0);
        // libc fills the fully initialized local tm; no process or network is used.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if !unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
            return format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
        }
    }
    "--:--".into()
}

pub fn run(args: &[String]) {
    let shell = args.first().map(String::as_str).unwrap_or("zsh");
    let code = args.get(1).and_then(|s| s.parse::<i32>().ok()).unwrap_or(0);
    let color = |n| match shell {
        "zsh" => format!("%{{\x1b[{n}m%}}"),
        "bash" => format!("\x01\x1b[{n}m\x02"),
        _ => format!("\x1b[{n}m"),
    };
    let mut dir = std::env::current_dir()
        .ok()
        .map(|p| {
            p.file_name()
                .unwrap_or(p.as_os_str())
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "?".into());
    dir = literal(shell, &dir);
    let user = literal(
        shell,
        &std::env::var("USER").unwrap_or_else(|_| "user".into()),
    );
    let mut branch = String::new();
    if let Some(status) = git() {
        let mut indicators = String::new();
        if status.staged {
            indicators.push_str(&format!("{}✚", color(33)));
        }
        if status.dirty {
            indicators.push_str(&format!("{}!", color(31)));
        }
        if status.ahead > 0 {
            indicators.push_str(&format!("{}⇡{}", color(95), status.ahead));
        }
        if status.behind > 0 {
            indicators.push_str(&format!("{}⇣{}", color(31), status.behind));
        }
        if indicators.is_empty() {
            indicators = format!("{}✔", color(32));
        }
        branch = format!(
            "{}{} {} {}❯{} ",
            color(95),
            literal(shell, &status.branch),
            indicators,
            color(33),
            color(0)
        );
    }
    // Keep Meow's compact colored-chevrons / user / directory / Git / time layout.
    // Plain Unicode avoids requiring a patched Nerd Font for a usable prompt.
    print!(
        "{}❯{}❯{}❯{} {} {}{}{} {}❯{} {}{}{} ➜ {}»{} ",
        color(35),
        color(32),
        color(36),
        color(0),
        user,
        color(36),
        dir,
        color(0),
        color(33),
        color(0),
        branch,
        color(37),
        clock(),
        color(if code == 0 { 32 } else { 31 }),
        color(0)
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn untrusted_metadata_is_bounded_and_controls_removed() {
        assert_eq!(safe("a\x1b\n\rb"), "ab");
        assert_eq!(safe(&"x".repeat(1000)).len(), 120);
    }
    #[test]
    fn status_preserves_staged_dirty_and_upstream_indicators() {
        let s = parse_git("## main...origin/main [ahead 2, behind 1]\nM  a\n M b\n").unwrap();
        assert_eq!(s.branch, "main");
        assert!(s.staged && s.dirty);
        assert_eq!((s.ahead, s.behind), (2, 1));
        assert_eq!(
            parse_git("## No commits yet on main\n").unwrap().branch,
            "main"
        );
    }
    #[test]
    fn shell_literals_cannot_create_prompt_escapes() {
        assert_eq!(literal("zsh", "%n"), "%%n");
        assert_eq!(literal("bash", r"\u"), r"\\u");
        assert_eq!(safe("a\u{202e}b"), "ab");
        assert_eq!(clock().len(), 5);
    }
}
