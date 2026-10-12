//! Bounded, state-aware application commands shared by shortcuts and custom UI.
//! This is an in-process UI dispatcher, not a shell/agent permission boundary.
use std::{collections::BTreeSet, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseReason {
    /// The standalone window's close button or an explicit close command.
    Window,
    /// An explicit application quit request (not forced OS termination).
    Quit,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloseDecision {
    Close,
    /// Keep the host and worker queues alive; the callback may open a dialog.
    KeepOpen,
}
/// Send through `Application::requests`. Requests are rechecked on the UI thread.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApplicationRequest {
    Command(String),
    Close(CloseReason),
}

/// Logical ASCII letter/digit shortcut with Cmd on macOS, Ctrl elsewhere.
/// Matching requires exactly the requested modifiers. Clipboard/undo/select-all
/// chords remain owned by the text editor and are rejected at validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Shortcut {
    key: char,
    shift: bool,
    alt: bool,
}
impl Shortcut {
    pub fn primary(key: char) -> Self {
        Self {
            key: key.to_ascii_lowercase(),
            shift: false,
            alt: false,
        }
    }
    pub fn shift(mut self, value: bool) -> Self {
        self.shift = value;
        self
    }
    pub fn alt(mut self, value: bool) -> Self {
        self.alt = value;
        self
    }
    pub fn label(self) -> String {
        format!(
            "{}{}{}{}",
            if cfg!(target_os = "macos") {
                "Cmd+"
            } else {
                "Ctrl+"
            },
            if self.shift { "Shift+" } else { "" },
            if self.alt { "Alt+" } else { "" },
            self.key.to_ascii_uppercase()
        )
    }
    fn validate(self) -> Result<(), String> {
        if !self.key.is_ascii_alphanumeric() || "acvqxyz".contains(self.key) {
            return Err(
                "shortcut requires an ASCII letter/digit and cannot override editor or system Quit chords".into(),
            );
        }
        Ok(())
    }
    #[cfg(all(feature = "native-desktop", target_os = "macos"))]
    pub(super) fn menu_accelerator(self) -> muda::accelerator::KeyAccelerator {
        use muda::accelerator::{Key, KeyAccelerator, Modifiers};
        let mut modifiers = Modifiers::SUPER;
        if self.shift {
            modifiers |= Modifiers::SHIFT;
        }
        if self.alt {
            modifiers |= Modifiers::ALT;
        }
        KeyAccelerator::new(Some(modifiers), Key::Character(self.key.to_string()))
    }
    pub(super) fn matches(
        self,
        key: char,
        mods: winit::keyboard::ModifiersState,
        mac: bool,
    ) -> bool {
        let primary = if mac {
            mods.super_key()
        } else {
            mods.control_key()
        };
        let other = if mac {
            mods.control_key()
        } else {
            mods.super_key()
        };
        primary
            && !other
            && mods.shift_key() == self.shift
            && mods.alt_key() == self.alt
            && key.to_ascii_lowercase() == self.key
    }
}

type Callback<C> = Box<dyn FnMut(&mut C)>;
type Enabled<C> = Box<dyn Fn(&C) -> bool>;
pub struct Command<C> {
    id: String,
    label: String,
    shortcut: Option<Shortcut>,
    enabled: Enabled<C>,
    callback: Callback<C>,
    close: Option<CloseReason>,
}
impl<C> Command<C> {
    pub fn new(
        id: impl Into<String>,
        label: impl Into<String>,
        callback: impl FnMut(&mut C) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            enabled: Box::new(|_| true),
            callback: Box::new(callback),
            close: None,
        }
    }
    pub fn shortcut(mut self, shortcut: Shortcut) -> Self {
        self.shortcut = Some(shortcut);
        self
    }
    /// Evaluated again immediately before dispatch, not just when a menu was drawn.
    pub fn enabled_when(mut self, enabled: impl Fn(&C) -> bool + 'static) -> Self {
        self.enabled = Box::new(enabled);
        self
    }
    /// Run the callback, then request closing through the application's close hook.
    pub fn request_close(mut self, reason: CloseReason) -> Self {
        self.close = Some(reason);
        self
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandInfo<'a> {
    pub id: &'a str,
    pub label: &'a str,
    pub shortcut: Option<Shortcut>,
    pub enabled: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandDispatch {
    /// Unknown id, or an invalid registry. No callback ran.
    Unknown,
    Disabled,
    /// A callback ran. The caller must apply any returned close request.
    Handled(Option<CloseReason>),
}
/// At most 256 commands, unique IDs and shortcuts. Custom palettes/buttons can
/// enumerate `entries` and send `ApplicationRequest::Command` to the native host.
/// Direct `dispatch` only runs Rust callbacks: it does not close an OS window.
pub struct CommandRegistry<C> {
    commands: Vec<Command<C>>,
    validation: OnceLock<Result<(), String>>,
}
impl<C> Default for CommandRegistry<C> {
    fn default() -> Self {
        Self {
            commands: vec![],
            validation: OnceLock::new(),
        }
    }
}
impl<C> CommandRegistry<C> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn command(mut self, command: Command<C>) -> Self {
        self.commands.push(command);
        self.validation = OnceLock::new();
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        self.validation
            .get_or_init(|| self.validate_definitions())
            .clone()
    }
    fn validate_definitions(&self) -> Result<(), String> {
        if self.commands.len() > 256 {
            return Err("application command limit exceeded".into());
        }
        let mut ids = BTreeSet::new();
        let mut shortcuts = BTreeSet::new();
        for c in &self.commands {
            if c.id.is_empty()
                || c.id.len() > 64
                || !c
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || c.label.trim().is_empty()
                || c.label.len() > 256
                || c.label.chars().any(char::is_control)
            {
                return Err("invalid application command id/label".into());
            }
            if !ids.insert(&c.id) {
                return Err("duplicate application command id".into());
            }
            if let Some(shortcut) = c.shortcut {
                shortcut.validate()?;
                if !shortcuts.insert(shortcut) {
                    return Err("duplicate application shortcut".into());
                }
            }
        }
        Ok(())
    }
    pub fn entries<'a>(&'a self, context: &C) -> Vec<CommandInfo<'a>> {
        if self.validate().is_err() {
            return vec![];
        }
        self.commands
            .iter()
            .map(|c| CommandInfo {
                id: &c.id,
                label: &c.label,
                shortcut: c.shortcut,
                enabled: (c.enabled)(context),
            })
            .collect()
    }
    pub fn dispatch(&mut self, id: &str, context: &mut C) -> CommandDispatch {
        if self.validate().is_err() {
            return CommandDispatch::Unknown;
        }
        let Some(c) = self.commands.iter_mut().find(|c| c.id == id) else {
            return CommandDispatch::Unknown;
        };
        if !(c.enabled)(context) {
            return CommandDispatch::Disabled;
        }
        (c.callback)(context);
        CommandDispatch::Handled(c.close)
    }
    pub(super) fn shortcut_id(
        &self,
        key: char,
        mods: winit::keyboard::ModifiersState,
    ) -> Option<&str> {
        self.commands
            .iter()
            .find(|c| {
                c.shortcut
                    .is_some_and(|s| s.matches(key, mods, cfg!(target_os = "macos")))
            })
            .map(|c| c.id.as_str())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::ModifiersState as M;
    #[test]
    fn logical_shortcuts_require_exact_primary_shift_and_alt() {
        let s = Shortcut::primary('s');
        assert!(s.matches('S', M::SUPER, true));
        assert!(s.matches('s', M::CONTROL, false));
        for mods in [
            M::empty(),
            M::CONTROL,
            M::SUPER | M::CONTROL,
            M::SUPER | M::SHIFT,
            M::SUPER | M::ALT,
        ] {
            assert!(!s.matches('s', mods, true));
        }
        assert!(!s.matches('t', M::SUPER, true));
        assert!(s
            .shift(true)
            .alt(true)
            .matches('S', M::SUPER | M::SHIFT | M::ALT, true));
        assert!(!s.matches('s', M::SUPER, false));
    }
    #[test]
    fn appending_after_validation_invalidates_the_cached_definition_result() {
        let r = CommandRegistry::new().command(Command::new("save", "Save", |_: &mut ()| {}));
        r.validate().unwrap();
        let r = r.command(Command::new("save", "Duplicate", |_| {}));
        assert!(r.validate().is_err());
    }
    #[test]
    fn invalid_direct_dispatch_is_inert_and_validation_never_calls_enablement() {
        let mut r = CommandRegistry::new().command(
            Command::new("bad/id", "Bad", |_: &mut ()| panic!("callback"))
                .enabled_when(|_| panic!("enabled")),
        );
        assert!(r.validate().is_err());
        assert!(r.entries(&()).is_empty());
        assert_eq!(r.dispatch("bad/id", &mut ()), CommandDispatch::Unknown);
    }
}
