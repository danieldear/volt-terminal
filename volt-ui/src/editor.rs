//! Which editor opens a file, and the shell command line that opens it. Volt
//! types that line into the current terminal tab, so the file path is
//! shell-quoted and anything that could break out of the line is refused.
use std::path::Path;
use volt_config::EditorConfig;

/// Longest editor command accepted from the config.
const MAX_COMMAND: usize = 512;

/// Quote one argument for POSIX shells and fish: `'…'`, with `'` as `'\''`.
pub fn shell_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', r"'\''"))
}

fn usable(command: &str) -> Option<&str> {
    let command = command.trim();
    (!command.is_empty() && command.len() <= MAX_COMMAND && !command.chars().any(char::is_control))
        .then_some(command)
}

/// The editor command for `path`: a file-type rule (full file name, then
/// extension), else the default command, else $VISUAL / $EDITOR. `None`
/// means nothing is configured and the caller should use the system app.
pub fn editor_for(
    config: &EditorConfig,
    path: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    let name = path.file_name().and_then(|n| n.to_str());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let by_type = name.and_then(|n| config.filetypes.get(n)).or_else(|| {
        ext.as_ref().and_then(|e| {
            config
                .filetypes
                .iter()
                .find(|(k, _)| k.trim_start_matches('.').eq_ignore_ascii_case(e))
                .map(|(_, v)| v)
        })
    });
    by_type
        .and_then(|c| usable(c))
        .or_else(|| usable(&config.command))
        .map(str::to_string)
        .or_else(|| {
            ["VISUAL", "EDITOR"]
                .iter()
                .find_map(|var| env(var).as_deref().and_then(usable).map(str::to_string))
        })
}

/// How an editor takes a line number, judged by its program name.
fn line_style(editor: &str) -> &'static str {
    let program = editor.split_whitespace().next().unwrap_or("");
    let program = program.rsplit('/').next().unwrap_or(program);
    match program {
        "vi" | "vim" | "nvim" | "nano" | "micro" | "emacs" | "emacsclient" | "kak" | "mg"
        | "ne" | "joe" | "jed" => "+{line} {file}",
        "hx" | "helix" | "zed" | "subl" | "sublime_text" => "{file}:{line}",
        "code" | "code-insiders" | "codium" | "cursor" | "windsurf" => "-g {file}:{line}",
        _ => "{file}",
    }
}

/// The line to type: `editor` with `{file}` (quoted) and `{line}` filled in.
/// Commands without placeholders get the editor's usual line syntax appended.
/// `None` for paths that can't be typed safely (control characters).
pub fn command_line(editor: &str, path: &Path, line: Option<usize>) -> Option<String> {
    let path = path.to_str()?;
    if path.chars().any(char::is_control) {
        return None;
    }
    let file = shell_quote(path);
    let template = if editor.contains("{file}") {
        editor.to_string()
    } else {
        let style = if line.is_some() {
            line_style(editor)
        } else {
            "{file}"
        };
        format!("{editor} {style}")
    };
    Some(
        template
            .replace("{line}", &line.unwrap_or(1).to_string())
            .replace("{file}", &file),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn config(command: &str, types: &[(&str, &str)]) -> EditorConfig {
        EditorConfig {
            command: command.into(),
            filetypes: types
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        }
    }

    #[test]
    fn file_type_rules_beat_the_default_and_env_is_the_last_resort() {
        let c = config(
            "nvim",
            &[("md", "nano"), ("Makefile", "vim"), (".JSON", "zed")],
        );
        let none = |_: &str| None;
        assert_eq!(
            editor_for(&c, Path::new("a/README.md"), &none).unwrap(),
            "nano"
        );
        assert_eq!(editor_for(&c, Path::new("Makefile"), &none).unwrap(), "vim");
        assert_eq!(editor_for(&c, Path::new("x.json"), &none).unwrap(), "zed");
        assert_eq!(editor_for(&c, Path::new("main.rs"), &none).unwrap(), "nvim");

        let env = |v: &str| (v == "EDITOR").then(|| "hx".to_string());
        let empty = config("", &[]);
        assert_eq!(
            editor_for(&empty, Path::new("main.rs"), &env).unwrap(),
            "hx"
        );
        assert!(editor_for(&empty, Path::new("main.rs"), &none).is_none());
        // A command with a control character is ignored, not typed.
        let bad = config("nvim\nrm -rf ~", &[]);
        assert!(editor_for(&bad, Path::new("main.rs"), &none).is_none());
    }

    #[test]
    fn commands_jump_to_the_line_in_each_editors_own_syntax() {
        let p = Path::new("/w/src/main.rs");
        let cl = |e: &str, l| command_line(e, p, l).unwrap();
        assert_eq!(cl("nvim", Some(42)), "nvim +42 '/w/src/main.rs'");
        assert_eq!(
            cl("/opt/homebrew/bin/nano", Some(3)),
            "/opt/homebrew/bin/nano +3 '/w/src/main.rs'"
        );
        assert_eq!(cl("hx", Some(7)), "hx '/w/src/main.rs':7");
        assert_eq!(
            cl("code --wait", Some(9)),
            "code --wait -g '/w/src/main.rs':9"
        );
        assert_eq!(cl("nvim", None), "nvim '/w/src/main.rs'");
        assert_eq!(cl("myedit", Some(5)), "myedit '/w/src/main.rs'");
        assert_eq!(
            cl("ed --at={line} {file}", Some(5)),
            "ed --at=5 '/w/src/main.rs'"
        );
    }

    #[test]
    fn paths_are_quoted_and_unsafe_paths_refused() {
        let tricky = PathBuf::from("/w/it's $(rm) `x`.md");
        assert_eq!(
            command_line("nvim", &tricky, None).unwrap(),
            r"nvim '/w/it'\''s $(rm) `x`.md'"
        );
        assert!(command_line("nvim", Path::new("/w/a\nb.md"), None).is_none());
    }
}
