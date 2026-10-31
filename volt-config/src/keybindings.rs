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
    MoveTabLeft,
    MoveTabRight,
    MovePaneLeft,
    MovePaneRight,
    MovePaneUp,
    MovePaneDown,
    SelectCommandOutput,
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
    /// Open the theme editor on the current theme. Unbound by default; on
    /// macOS it's also in Volt ▸ Theme ▸ Customize Theme….
    CustomizeTheme,
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
    fn move_pane_actions_serde_round_trip() {
        for (name, action) in [
            ("move_pane_left", Action::MovePaneLeft),
            ("move_pane_right", Action::MovePaneRight),
            ("move_pane_up", Action::MovePaneUp),
            ("move_pane_down", Action::MovePaneDown),
        ] {
            let binding: KeyBinding =
                toml::from_str(&format!("key='ctrl+shift+h'\naction='{name}'")).unwrap();
            assert_eq!(binding.action, action);
            let decoded: KeyBinding = toml::from_str(&toml::to_string(&binding).unwrap()).unwrap();
            assert_eq!(decoded.action, action);
            assert_eq!(decoded.key, binding.key);
            let config: AdvancedBindingsConfig = toml::from_str(&format!(
                "[[bindings]]\nsequence=['ctrl+k','h']\ntarget={{kind='action',value='{name}'}}"
            ))
            .unwrap();
            assert_eq!(config.bindings[0].target, BindingTarget::Action(action));
        }
    }

    #[test]
    fn config_defaults_and_actions_are_validated() {
        let c: crate::Config =
            toml::from_str("[[keybindings]]\nkey='cmd+shift+s'\naction='search_workspace'")
                .unwrap();
        assert_eq!(c.keybindings[0].action, Action::SearchWorkspace);
        let move_tab: crate::Config =
            toml::from_str("[[keybindings]]\nkey='ctrl+alt+left'\naction='move_tab_left'").unwrap();
        assert_eq!(move_tab.keybindings[0].action, Action::MoveTabLeft);
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

/// Limits apply to both deserialized and programmatically constructed configs.
pub const MAX_SEQUENCE_LEN: usize = 8;
pub const MAX_ADVANCED_BINDINGS: usize = 256;
pub const MAX_KEY_TABLES: usize = 16;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum BindingTarget {
    Action(Action),
    Table(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SequenceBinding {
    pub sequence: Vec<Chord>,
    pub target: BindingTarget,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyTable {
    pub name: String,
    pub bindings: Vec<SequenceBinding>,
}

/// Separate from legacy keybindings; the parent config opts into this type.
/// Deserialization validates automatically; compilation must validate again
/// because these public fields permit programmatic construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "AdvancedBindingsWire")]
pub struct AdvancedBindingsConfig {
    pub timeout_ms: u64,
    pub bindings: Vec<SequenceBinding>,
    pub tables: Vec<KeyTable>,
}
impl Default for AdvancedBindingsConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 1000,
            bindings: Vec::new(),
            tables: Vec::new(),
        }
    }
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct AdvancedBindingsWire {
    timeout_ms: u64,
    bindings: Vec<SequenceBinding>,
    tables: Vec<KeyTable>,
}
impl Default for AdvancedBindingsWire {
    fn default() -> Self {
        Self {
            timeout_ms: 1000,
            bindings: Vec::new(),
            tables: Vec::new(),
        }
    }
}
impl TryFrom<AdvancedBindingsWire> for AdvancedBindingsConfig {
    type Error = String;
    fn try_from(w: AdvancedBindingsWire) -> Result<Self, String> {
        let config = Self {
            timeout_ms: w.timeout_ms,
            bindings: w.bindings,
            tables: w.tables,
        };
        config.validate()?;
        Ok(config)
    }
}
impl AdvancedBindingsConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=60_000).contains(&self.timeout_ms) {
            return Err("advanced timeout_ms must be 1..=60000".into());
        }
        if self.tables.len() > MAX_KEY_TABLES {
            return Err("at most 16 key tables are supported".into());
        }
        let mut names = std::collections::HashSet::new();
        let mut count = self.bindings.len();
        for table in &self.tables {
            if table.name.is_empty()
                || table.name.len() > 32
                || !table
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            {
                return Err("table names must be 1..=32 ASCII letters, digits, _ or -".into());
            }
            if !names.insert(table.name.as_str()) {
                return Err(format!("duplicate key table: {}", table.name));
            }
            if table.bindings.is_empty() {
                return Err(format!("empty key table: {}", table.name));
            }
            count = count.saturating_add(table.bindings.len());
        }
        if count > MAX_ADVANCED_BINDINGS {
            return Err("at most 256 total advanced bindings are supported".into());
        }
        for bindings in
            std::iter::once(&self.bindings).chain(self.tables.iter().map(|t| &t.bindings))
        {
            for (i, binding) in bindings.iter().enumerate() {
                if !(1..=MAX_SEQUENCE_LEN).contains(&binding.sequence.len()) {
                    return Err("sequences must contain 1..=8 chords".into());
                }
                for chord in &binding.sequence {
                    // Validate public Chord fields, including noncanonical spellings.
                    let parsed = Chord::try_from(String::from(chord.clone()))?;
                    let function_alias = chord
                        .key
                        .strip_prefix('f')
                        .and_then(|s| s.parse::<u8>().ok())
                        .is_some_and(|n| chord.key != format!("f{n}"));
                    if parsed != *chord || chord.modifiers > 15 || function_alias {
                        return Err("noncanonical or invalid chord".into());
                    }
                    if chord.key == "escape" {
                        return Err("Escape is reserved for cancellation".into());
                    }
                }
                if let BindingTarget::Table(name) = &binding.target {
                    if !names.contains(name.as_str()) {
                        return Err(format!("unknown key table: {name}"));
                    }
                }
                for other in &bindings[..i] {
                    if binding.sequence.starts_with(&other.sequence)
                        || other.sequence.starts_with(&binding.sequence)
                    {
                        return Err("duplicate or ambiguous prefix sequence in key table".into());
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    fn binding(keys: &[&str]) -> SequenceBinding {
        SequenceBinding {
            sequence: keys
                .iter()
                .map(|s| s.to_string().try_into().unwrap())
                .collect(),
            target: BindingTarget::Action(Action::Copy),
        }
    }
    #[test]
    fn validated_toml_and_round_trip() {
        let text = "timeout_ms=500\n[[bindings]]\nsequence=['ctrl+k','c']\ntarget={kind='table',value='pane'}\n[[tables]]\nname='pane'\n[[tables.bindings]]\nsequence=['h']\ntarget={kind='action',value='focus_left'}";
        let config: AdvancedBindingsConfig = toml::from_str(text).unwrap();
        assert_eq!(config.timeout_ms, 500);
        let round: AdvancedBindingsConfig =
            toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        assert_eq!(round.bindings[0].sequence, config.bindings[0].sequence);
        for text in [
            "timeout_ms=0",
            "timeout_ms=60001",
            "unknown=true",
            "[[bindings]]\nsequence=[]\ntarget={kind='action',value='copy'}",
            "[[bindings]]\nsequence=['escape']\ntarget={kind='action',value='copy'}",
            "[[bindings]]\nsequence=['a']\ntarget={kind='action',value='run_shell'}",
            "[[bindings]]\nsequence=['a']\ntarget={kind='table',value='missing'}",
        ] {
            assert!(
                toml::from_str::<AdvancedBindingsConfig>(text).is_err(),
                "{text}"
            );
        }
    }
    #[test]
    fn bounds_and_prefix_ambiguity_are_rejected() {
        for bindings in [
            vec![binding(&[])],
            vec![binding(&["a"; 9])],
            vec![binding(&["a"]); 257],
            vec![binding(&["a"]), binding(&["a"])],
            vec![binding(&["a"]), binding(&["a", "b"])],
            vec![binding(&["a", "b"]), binding(&["a"])],
        ] {
            assert!(AdvancedBindingsConfig {
                bindings,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
        let mut c = AdvancedBindingsConfig {
            tables: (0..17)
                .map(|i| KeyTable {
                    name: format!("t{i}"),
                    bindings: vec![binding(&["a"])],
                })
                .collect(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
        c.tables.truncate(1);
        c.tables.push(c.tables[0].clone());
        assert!(c.validate().is_err());
        c.tables.truncate(1);
        c.tables[0].name = "bad name".into();
        assert!(c.validate().is_err());
        c.tables[0].name = "ok".into();
        c.tables[0].bindings.clear();
        assert!(c.validate().is_err());
    }
    #[test]
    fn public_fields_cannot_bypass_chord_validation() {
        for chord in [
            Chord {
                key: "f01".into(),
                modifiers: 0,
            },
            Chord {
                key: "A".into(),
                modifiers: 0,
            },
            Chord {
                key: "a".into(),
                modifiers: 16,
            },
            Chord {
                key: "invalid".into(),
                modifiers: 0,
            },
        ] {
            let c = AdvancedBindingsConfig {
                bindings: vec![SequenceBinding {
                    sequence: vec![chord],
                    target: BindingTarget::Action(Action::Copy),
                }],
                ..Default::default()
            };
            assert!(c.validate().is_err());
        }
    }
}
