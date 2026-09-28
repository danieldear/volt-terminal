//! Validated application bindings. No terminal escape parsing or command execution.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Copy,
    Paste,
    Find,
    SearchWorkspace,
    ToggleWorkspace,
    ToggleWorkspaceLayout,
    NewTab,
    NewWindow,
    ClosePane,
    NextTab,
    PreviousTab,
    PreviousPrompt,
    NextPrompt,
    SplitRight,
    SplitLeft,
    SplitDown,
    SplitUp,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    IncreaseFontSize,
    DecreaseFontSize,
    ToggleFullscreen,
    ReloadConfig,
    OpenConfig,
    Quit,
    Ignore,
    Unbind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "String", into = "String")]
pub struct Chord {
    pub key: String,
    /// ctrl=1, alt=2, shift=4, super=8; exact modifier matching.
    pub modifiers: u8,
}
impl TryFrom<String> for Chord {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.len() > 80 {
            return Err("keybinding trigger is too long".into());
        }
        let raw = raw.to_ascii_lowercase();
        let mut parts: Vec<_> = raw.split('+').map(str::trim).collect();
        let key = parts.pop().unwrap_or("");
        let valid = (key.len() == 1 && key.bytes().all(|c| c.is_ascii_alphanumeric()))
            || matches!(
                key,
                "enter"
                    | "escape"
                    | "tab"
                    | "space"
                    | "backspace"
                    | "delete"
                    | "home"
                    | "end"
                    | "pageup"
                    | "pagedown"
                    | "left"
                    | "right"
                    | "up"
                    | "down"
                    | "equal"
                    | "minus"
                    | "comma"
                    | "period"
                    | "slash"
                    | "semicolon"
                    | "quote"
                    | "backquote"
                    | "bracketleft"
                    | "bracketright"
                    | "backslash"
            )
            || key
                .strip_prefix('f')
                .and_then(|s| s.parse::<u8>().ok())
                .is_some_and(|n| (1..=24).contains(&n));
        if !valid {
            return Err(format!("invalid keybinding key: {key:?}"));
        }
        let mut modifiers = 0;
        for part in parts {
            let bit = match part {
                "ctrl" | "control" => 1,
                "alt" | "opt" | "option" => 2,
                "shift" => 4,
                "super" | "cmd" | "command" => 8,
                _ => return Err(format!("invalid keybinding modifier: {part:?}")),
            };
            if modifiers & bit != 0 {
                return Err("duplicate keybinding modifier".into());
            }
            modifiers |= bit;
        }
        Ok(Self {
            key: key.into(),
            modifiers,
        })
    }
}
impl From<Chord> for String {
    fn from(chord: Chord) -> Self {
        let mut s = String::new();
        for (bit, name) in [(1, "ctrl+"), (2, "alt+"), (4, "shift+"), (8, "super+")] {
            if chord.modifiers & bit != 0 {
                s.push_str(name);
            }
        }
        s.push_str(&chord.key);
        s
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyBinding {
    pub key: Chord,
    pub action: Action,
}

pub fn deserialize_bindings<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Vec<KeyBinding>, D::Error> {
    let bindings = Vec::<KeyBinding>::deserialize(d)?;
    if bindings.len() > 128 {
        return Err(serde::de::Error::custom(
            "at most 128 keybindings are supported",
        ));
    }
    Ok(bindings)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chords_validate_and_round_trip() {
        let c = Chord::try_from("Cmd+SHIFT+f".to_owned()).unwrap();
        assert_eq!(c.modifiers, 12);
        assert_eq!(String::from(c), "shift+super+f");
        for bad in [
            "",
            "cmd+",
            "cmd+cmd+a",
            "hyper+a",
            "ctrl+f25",
            "ctrl+launch_app",
        ] {
            assert!(Chord::try_from(bad.to_owned()).is_err(), "{bad}");
        }
    }
    #[test]
    fn config_defaults_and_actions_are_validated() {
        let c: crate::Config =
            toml::from_str("[[keybindings]]\nkey='cmd+shift+s'\naction='search_workspace'")
                .unwrap();
        assert_eq!(c.keybindings[0].action, Action::SearchWorkspace);
        assert!(toml::from_str::<crate::Config>(
            "[[keybindings]]\nkey='cmd+s'\naction='run_shell'"
        )
        .is_err());
        assert!(crate::Config::default().keybindings.is_empty());
        assert!(toml::from_str::<crate::Config>(
            &"[[keybindings]]\nkey='cmd+s'\naction='ignore'\n".repeat(129)
        )
        .is_err());
    }
}
