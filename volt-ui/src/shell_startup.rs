//! App-scoped shell setup. Never edits startup files or sources project scripts.
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use volt_config::{config::PromptMode, Config};
static ROOT: OnceLock<PathBuf> = OnceLock::new();
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
fn write(path: &Path, text: &str) -> anyhow::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(text.as_bytes())?;
    Ok(())
}
fn root() -> anyhow::Result<PathBuf> {
    if let Some(p) = ROOT.get() {
        return Ok(p.clone());
    }
    let p = std::env::temp_dir().join(format!(
        "volt-shell-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    let mut b = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        b.mode(0o700);
    }
    b.create(&p)?;
    let _ = ROOT.set(p.clone());
    Ok(p)
}
pub fn prepare(c: &Config) -> anyhow::Result<(String, Vec<String>)> {
    if !c.shell.integration && c.shell.prompt != PromptMode::Meow {
        return Ok((c.shell.program.clone(), c.shell.args.clone()));
    }
    prepare_for(
        c,
        &std::env::current_exe()?,
        &std::env::var("HOME").unwrap_or_default(),
        std::env::var("ZDOTDIR").ok().as_deref(),
    )
}
fn prepare_for(
    c: &Config,
    exe: &Path,
    home: &str,
    zdotdir: Option<&str>,
) -> anyhow::Result<(String, Vec<String>)> {
    static BUILD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _lock = BUILD
        .lock()
        .map_err(|_| anyhow::anyhow!("Shell setup lock poisoned"))?;
    if !c.shell.integration && c.shell.prompt != PromptMode::Meow {
        return Ok((c.shell.program.clone(), c.shell.args.clone()));
    }
    let name = Path::new(&c.shell.program)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    anyhow::ensure!(
        matches!(name, "bash" | "zsh" | "fish"),
        "Automatic setup supports Bash, Zsh and Fish"
    );
    anyhow::ensure!(
        c.shell
            .args
            .iter()
            .all(|a| matches!(a.as_str(), "-l" | "--login" | "-i" | "--interactive")),
        "Use default login/interactive arguments for bundled prompt setup"
    );
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (
        &c.shell.program,
        &c.shell.args,
        c.shell.integration,
        c.shell.prompt == PromptMode::Meow,
        exe,
        home,
        zdotdir,
    )
        .hash(&mut hash);
    let dir = root()?.join(format!("{name}-{:x}", hash.finish()));
    if dir.exists() && !dir.join("ready").is_file() {
        std::fs::remove_dir_all(&dir)?;
    }
    if !dir.exists() {
        let mut b = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            b.mode(0o700);
        }
        b.create(&dir)?;
        let exe = exe
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Executable path isn't UTF-8"))?;
        anyhow::ensure!(
            !exe.chars().any(char::is_control),
            "Invalid executable path"
        );
        let hook = match name {
            "zsh" => include_str!("../../scripts/shell-integration/volt.zsh"),
            "bash" => include_str!("../../scripts/shell-integration/volt.bash"),
            _ => "",
        };
        let prompt = if c.shell.prompt == PromptMode::Meow {
            match name {
            "zsh"=>format!("\nunsetopt promptsubst promptbang\n_volt_meow_precmd() {{ local last_status=$?; PROMPT=\"$({} --meow zsh $last_status)\"; }}\nautoload -Uz add-zsh-hook\nadd-zsh-hook precmd _volt_meow_precmd\n",quote(exe)),
            "bash"=>format!(r#"
shopt -u promptvars
_volt_meow_precmd() {{ local status=$?; PS1="$({} --meow bash $status)"; return "$status"; }}
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )) &&
    [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a '* ]]; then
    PROMPT_COMMAND=(_volt_meow_precmd "${{PROMPT_COMMAND[@]}}")
else
    PROMPT_COMMAND="_volt_meow_precmd${{PROMPT_COMMAND:+; $PROMPT_COMMAND}}"
fi
"#,quote(exe)),
            _=>format!("\nfunction fish_prompt\n set -l last_status $status\n {} --meow fish $last_status\nend\n",quote(exe))}
        } else {
            String::new()
        };
        match name {
            "zsh" => {
                let orig = zdotdir.filter(|s| !s.is_empty()).unwrap_or(home);
                anyhow::ensure!(!orig.chars().any(char::is_control), "Invalid shell home");
                let login = c.shell.args.iter().any(|a| a == "-l" || a == "--login");
                for file in [".zshenv", ".zprofile", ".zshrc", ".zlogin"] {
                    let mut text = String::new();
                    if file == ".zshenv" {
                        text.push_str(&format!("typeset -g _VOLT_USER_ZDOTDIR={}\n", quote(orig)));
                    }
                    text.push_str(&format!("ZDOTDIR=$_VOLT_USER_ZDOTDIR\n[[ ! -r $ZDOTDIR/{file} ]] || source $ZDOTDIR/{file}\ntypeset -g _VOLT_USER_ZDOTDIR=${{ZDOTDIR:-$HOME}}\n"));
                    let final_file = file == if login { ".zlogin" } else { ".zshrc" };
                    if final_file {
                        // Restore the user's directory for child shells and .zlogout.
                        text.push_str("ZDOTDIR=$_VOLT_USER_ZDOTDIR\nunset _VOLT_USER_ZDOTDIR\n");
                        // Both hooks see the original last status in Zsh's hook dispatch.
                        if c.shell.integration {
                            text.push_str(hook);
                        }
                        text.push_str(&prompt);
                    } else {
                        text.push_str(&format!("ZDOTDIR={}\n", quote(&dir.to_string_lossy())));
                    }
                    write(&dir.join(file), &text)?;
                }
            }
            "bash" => {
                let login = c.shell.args.iter().any(|a| a == "-l" || a == "--login");
                let mut text: String = if login {
                    "[[ ! -r /etc/profile ]] || source /etc/profile\nfor _volt_profile in \"$HOME/.bash_profile\" \"$HOME/.bash_login\" \"$HOME/.profile\"; do\n if [[ -r $_volt_profile ]]; then source \"$_volt_profile\"; break; fi\ndone\nunset _volt_profile\n".into()
                } else {
                    "[[ ! -r $HOME/.bashrc ]] || source \"$HOME/.bashrc\"\n".into()
                };
                if c.shell.integration {
                    text.push_str(hook);
                }
                text.push_str(&prompt);
                write(&dir.join("bashrc"), &text)?;
            }
            _ => {
                let mut init = if c.shell.integration {
                    String::from(
                        r#"
if test (string split . -- $version)[1] -lt 4
    function _volt_fish_preexec --on-event fish_preexec
        printf '\e]133;C\a'
    end
    function _volt_fish_postexec --on-event fish_postexec
        set -l last_status $status
        printf '\e]133;D;%d\a' $last_status
    end
    function _volt_fish_prompt --on-event fish_prompt
        printf '\e]133;A\a'
    end
end
"#,
                    )
                } else {
                    String::new()
                };
                init.push_str(&prompt);
                write(&dir.join("init.fish"), &init)?;
            }
        }
    }
    if !dir.join("ready").exists() {
        write(&dir.join("ready"), "ready")?;
    }
    Ok(match name {
        "zsh" => {
            let mut args = vec![
                format!("ZDOTDIR={}", dir.display()),
                c.shell.program.clone(),
            ];
            args.extend(c.shell.args.clone());
            args.push("-i".into());
            ("/usr/bin/env".into(), args)
        }
        "bash" => (
            c.shell.program.clone(),
            vec![
                "--rcfile".into(),
                dir.join("bashrc").to_string_lossy().into_owned(),
                "-i".into(),
            ],
        ),
        _ => {
            let mut a = c.shell.args.clone();
            a.extend([
                "-i".into(),
                "--init-command".into(),
                format!("source {}", quote(&dir.join("init.fish").to_string_lossy())),
            ]);
            (c.shell.program.clone(), a)
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_prompt_is_exact_passthrough() {
        let c = Config::default();
        let p = prepare(&c).unwrap();
        assert_eq!(p.0, c.shell.program);
        assert_eq!(p.1, c.shell.args);
    }
    #[test]
    fn unknown_shell_is_not_rewritten() {
        let mut c = Config::default();
        c.shell.prompt = PromptMode::Meow;
        c.shell.program = "/bin/sh".into();
        assert!(prepare(&c).is_err());
    }
    #[test]
    fn real_shells_source_user_rc_without_modifying_it() {
        use std::os::unix::fs::PermissionsExt;
        let home = root().unwrap().join("test home 'quoted'");
        std::fs::create_dir_all(home.join(".config/fish")).unwrap();
        let exe = home.join("prompt stub");
        std::fs::write(&exe, "#!/bin/sh\nprintf 'Meow-%s-%s' \"$2\" \"$3\"\n").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        let rc = "export VOLT_TEST_RC=yes\nPS1='existing'\n";
        for file in [".zshrc", ".bashrc", ".bash_profile"] {
            std::fs::write(home.join(file), rc).unwrap();
        }
        std::fs::write(
            home.join(".config/fish/config.fish"),
            "set -gx VOLT_TEST_RC yes\n",
        )
        .unwrap();
        for shell in ["/bin/zsh", "/bin/bash", "/opt/homebrew/bin/fish"] {
            if !Path::new(shell).exists() {
                continue;
            }
            for login in [false, true] {
                let mut c = Config::default();
                c.shell.program = shell.into();
                c.shell.args = if login { vec!["-l".into()] } else { vec![] };
                c.shell.prompt = PromptMode::Meow;
                c.shell.integration = true;
                let (program, mut args) =
                    prepare_for(&c, &exe, home.to_str().unwrap(), None).unwrap();
                let command = if shell.ends_with("fish") {
                    "test $VOLT_TEST_RC = yes; or exit 90; false; fish_prompt"
                } else if shell.ends_with("zsh") {
                    "test $VOLT_TEST_RC = yes || exit 90; [[ $ZDOTDIR == $HOME ]] || exit 91; false; _volt_meow_precmd; print -r -- $PROMPT"
                } else {
                    "test $VOLT_TEST_RC = yes || exit 90; false; _volt_meow_precmd; printf '%s' \"$PS1\"; true"
                };
                args.extend(["-c".into(), command.into()]);
                let output = std::process::Command::new(program)
                    .args(args)
                    .env("HOME", &home)
                    .env("XDG_CONFIG_HOME", home.join(".config"))
                    .env("TERM_PROGRAM", "volt")
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{shell}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(
                    String::from_utf8_lossy(&output.stdout).contains(&format!(
                        "Meow-{}-1",
                        Path::new(shell).file_name().unwrap().to_str().unwrap()
                    )),
                    "{shell}: {}",
                    String::from_utf8_lossy(&output.stdout)
                );
            }
        }
        for file in [".zshrc", ".bashrc", ".bash_profile"] {
            assert_eq!(std::fs::read_to_string(home.join(file)).unwrap(), rc);
        }
    }
    #[test]
    fn prompt_hooks_report_failure_through_real_pty() {
        use std::time::{Duration, Instant};
        use volt_core::{events::CoreEvent, pty::Pty};
        let home = root().unwrap().join("pty-home");
        std::fs::create_dir_all(&home).unwrap();
        let exe = home.join("stub");
        std::fs::write(&exe, "#!/bin/sh\nprintf 'test-status-%s > ' \"$3\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o700)).unwrap();
        for shell in ["/bin/zsh", "/bin/bash", "/opt/homebrew/bin/fish"] {
            if !Path::new(shell).exists() {
                continue;
            }
            let mut c = Config::default();
            c.shell.program = shell.into();
            c.shell.args = vec![];
            c.shell.integration = true;
            c.shell.prompt = PromptMode::Meow;
            let (program, args) = prepare_for(&c, &exe, home.to_str().unwrap(), None).unwrap();
            let mut envargs = vec![
                format!("HOME={}", home.display()),
                format!("XDG_CONFIG_HOME={}", home.join(".config").display()),
                program,
            ];
            envargs.extend(args);
            let (mut pty, performer, mut rx) =
                Pty::spawn("/usr/bin/env", &envargs, 80, 24, || {}).unwrap();
            std::thread::sleep(Duration::from_millis(300));
            while rx.try_recv().is_ok() {}
            pty.write(b"false\n").unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut found = false;
            while Instant::now() < deadline {
                while let Ok(e) = rx.try_recv() {
                    if matches!(e, CoreEvent::CommandFinished { exit_code: 1, .. }) {
                        found = true;
                    }
                }
                let prompt_reported = {
                    let p = performer.lock().unwrap();
                    (0..p.grid.rows).any(|row| {
                        p.grid
                            .row_text(p.grid.row_cells(row))
                            .contains("test-status-1")
                    })
                };
                if found && prompt_reported {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(found, "{shell}: failing command did not emit exit status");
            let p = performer.lock().unwrap();
            assert!(
                (0..p.grid.rows).any(|row| p
                    .grid
                    .row_text(p.grid.row_cells(row))
                    .contains("test-status-1")),
                "{shell}: Meow lost command status"
            );
            drop(p);
            pty.write(b"exit\n").unwrap();
        }
    }
}
