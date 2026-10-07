//! Explicit, current-tab setup. The trusted built-in hooks are compiled into
//! Volt: no project script is sourced and no startup file is edited.
use crate::editor::shell_quote;

pub fn enable_command(shell: &str) -> Option<String> {
    let (hook, file) = match std::path::Path::new(shell).file_name()?.to_str()? {
        "zsh" => (
            include_str!("../../scripts/shell-integration/volt.zsh"),
            "volt.zsh",
        ),
        "bash" => (
            include_str!("../../scripts/shell-integration/volt.bash"),
            "volt.bash",
        ),
        _ => return None,
    };
    // Prefer a readable one-line source command for an app's own packaged
    // hook. A changed/symlinked resource never becomes executable project code:
    // only the exact built-in bytes qualify; otherwise use the compiled copy.
    if let Some(path) = packaged_hook(file, hook) {
        return Some(format!("source {}", shell_quote(path.to_str()?)));
    }
    // In a bare source build there may be no app resources. Scope the hook's
    // early return to a function, and encode newlines rather than typing a
    // multi-line quoted command into the interactive shell.
    let setup = format!(
        "_volt_enable_hooks() {{\n{hook}\n}}; _volt_enable_hooks; unset -f _volt_enable_hooks"
    );
    let escaped = setup
        .replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n");
    Some(format!("eval $'{escaped}'"))
}

fn packaged_hook(file: &str, expected: &str) -> Option<std::path::PathBuf> {
    use std::io::Read;
    let exe = std::env::current_exe().ok()?;
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    if macos.file_name()? != "MacOS" || contents.file_name()? != "Contents" {
        return None;
    }
    let path = contents.join("Resources/shell-integration").join(file);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() != expected.len() as u64 {
        return None;
    }
    let mut bytes = Vec::with_capacity(expected.len());
    std::fs::File::open(&path)
        .ok()?
        .take(expected.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes == expected.as_bytes() && !path.to_str()?.chars().any(char::is_control)).then_some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_supported_shells_receive_embedded_hooks() {
        assert!(enable_command("/bin/zsh").unwrap().starts_with("eval $'"));
        assert!(enable_command("/bin/bash")
            .unwrap()
            .contains("PROMPT_COMMAND"));
        for shell in ["/bin/sh", "fish", "zsh; echo bad", ""] {
            assert!(enable_command(shell).is_none());
        }
    }
    #[test]
    fn repeated_setup_keeps_the_shell_alive_and_hooks_installed() {
        for (shell, flag) in [
            ("/bin/zsh", "_VOLT_ZSH_INTEGRATION"),
            ("/bin/bash", "_VOLT_BASH_INTEGRATION"),
        ] {
            if !std::path::Path::new(shell).exists() {
                continue;
            }
            let enable = enable_command(shell).unwrap();
            let command = format!("{enable}; {enable}; test \"${flag}\" = 1 && command -v _volt_precmd && printf ALIVE");
            let mut child = std::process::Command::new(shell);
            if shell.ends_with("zsh") {
                child.arg("-f");
            } else {
                child.args(["--noprofile", "--norc"]);
            }
            let out = child
                .args(["-i", "-c", &command])
                .env("TERM_PROGRAM", "volt")
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(String::from_utf8_lossy(&out.stdout).ends_with("ALIVE"));
        }
    }

    #[test]
    fn generated_commands_are_valid_shell_syntax() {
        for shell in ["/bin/zsh", "/bin/bash"] {
            if !std::path::Path::new(shell).exists() {
                continue;
            }
            let status = std::process::Command::new(shell)
                .args(["-n", "-c", &enable_command(shell).unwrap()])
                .status()
                .unwrap();
            assert!(status.success());
        }
    }
}
