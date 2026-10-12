//! Explicit optional platform integrations. Capability flags mean adapter availability,
//! not authorization, hardware support, or completed platform acceptance.
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SystemCapabilities {
    pub native_menu: bool,
    pub tray_icon: bool,
    pub notifications: bool,
    pub embedded_web: bool,
    pub maps: bool,
    pub on_device_ai: bool,
    pub app_intents: bool,
}
pub fn capabilities() -> SystemCapabilities {
    let desktop = cfg!(all(feature = "native-desktop", target_os = "macos"));
    SystemCapabilities {
        native_menu: desktop,
        tray_icon: desktop,
        notifications: super::notifications::capabilities().local_requests,
        embedded_web: false,
        maps: false,
        on_device_ai: false,
        app_intents: false,
    }
}
#[derive(Clone, Debug)]
pub struct DesktopCommand {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}
impl DesktopCommand {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }
    pub fn enabled(mut self, value: bool) -> Self {
        self.enabled = value;
        self
    }
}
#[derive(Clone, Debug)]
pub struct DesktopMenu {
    pub label: String,
    pub commands: Vec<DesktopCommand>,
}
impl DesktopMenu {
    pub fn new(
        label: impl Into<String>,
        commands: impl IntoIterator<Item = DesktopCommand>,
    ) -> Self {
        Self {
            label: label.into(),
            commands: commands.into_iter().collect(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct TrayConfig {
    pub tooltip: String,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub template: bool,
    pub commands: Vec<DesktopCommand>,
}
#[derive(Clone, Debug)]
pub struct DesktopConfig {
    pub app_name: String,
    pub menus: Vec<DesktopMenu>,
    pub tray: Option<TrayConfig>,
}
impl DesktopConfig {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
            menus: Vec::new(),
            tray: None,
        }
    }
    pub fn menu(mut self, menu: DesktopMenu) -> Self {
        self.menus.push(menu);
        self
    }
    pub fn tray(mut self, tray: TrayConfig) -> Self {
        self.tray = Some(tray);
        self
    }
    pub fn validate(&self) -> Result<(), String> {
        fn label(s: &str) -> bool {
            !s.trim().is_empty() && s.len() <= 256 && !s.chars().any(char::is_control)
        }
        if !label(&self.app_name)
            || self.menus.len() > 32
            || self.menus.iter().any(|m| !label(&m.label))
        {
            return Err("invalid desktop app/menu label or count".into());
        }
        let total = self
            .menus
            .iter()
            .try_fold(0usize, |n, m| n.checked_add(m.commands.len()))
            .and_then(|n| n.checked_add(self.tray.as_ref().map_or(0, |t| t.commands.len())));
        if total.is_none_or(|n| n > 512) {
            return Err("desktop command limit exceeded".into());
        }
        let mut ids = BTreeMap::new();
        for item in self
            .menus
            .iter()
            .flat_map(|m| &m.commands)
            .chain(self.tray.iter().flat_map(|t| &t.commands))
        {
            if item.id.is_empty()
                || item.id.len() > 64
                || !item
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || !label(&item.label)
            {
                return Err("invalid desktop command id/label".into());
            }
            if let Some((old_label, old_enabled)) =
                ids.insert(&item.id, (&item.label, item.enabled))
            {
                if old_label != &item.label || old_enabled != item.enabled {
                    return Err("inconsistent desktop command definitions".into());
                }
            }
        }
        if ids.len() > 256 {
            return Err("desktop command limit exceeded".into());
        }
        if let Some(tray) = &self.tray {
            if !label(&tray.tooltip)
                || tray.width == 0
                || tray.height == 0
                || tray.width > 128
                || tray.height > 128
                || tray.rgba.len() != tray.width as usize * tray.height as usize * 4
            {
                return Err("tray icon must be bounded 1..128 RGBA pixels with a tooltip".into());
            }
        }
        Ok(())
    }
    pub(super) fn accepts(&self, id: &str) -> bool {
        self.command_enabled(id) == Some(true)
    }
    pub(super) fn command_enabled(&self, id: &str) -> Option<bool> {
        self.menus
            .iter()
            .flat_map(|m| &m.commands)
            .chain(self.tray.iter().flat_map(|t| &t.commands))
            .find(|c| c.id == id)
            .map(|c| c.enabled)
    }
}
#[cfg(all(feature = "native-desktop", target_os = "macos"))]
pub(super) mod macos {
    use super::*;
    use muda::{Menu, MenuItem, PredefinedMenuItem, Submenu};
    use std::cell::Cell;
    pub enum DesktopEvent {
        Command(String),
        Quit,
    }
    pub struct DesktopHost {
        menu: Menu,
        _tray: Option<tray_icon::TrayIcon>,
        command_items: Vec<CommandItem>,
    }
    struct CommandItem {
        id: String,
        item: MenuItem,
        declared_enabled: bool,
        current_enabled: Cell<bool>,
    }
    impl Drop for DesktopHost {
        fn drop(&mut self) {
            self.menu.remove_for_nsapp();
        }
    }
    impl DesktopHost {
        pub fn install(
            config: &DesktopConfig,
            send: impl Fn(DesktopEvent) + Send + Sync + 'static,
            guarded_quit: bool,
            commands: &[crate::native::CommandInfo<'_>],
        ) -> Result<Self, String> {
            config.validate()?;
            let menu = Menu::new();
            let mut command_items = vec![];
            let apple = Submenu::new(&config.app_name, true);
            apple
                .append_items(&[
                    &PredefinedMenuItem::about(None, None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::services(None),
                    &PredefinedMenuItem::separator(),
                    &PredefinedMenuItem::hide(None),
                    &PredefinedMenuItem::hide_others(None),
                    &PredefinedMenuItem::show_all(None),
                    &PredefinedMenuItem::separator(),
                ])
                .map_err(|e| e.to_string())?;
            if guarded_quit {
                let quit = MenuItem::with_id(
                    "volt-host-quit",
                    format!("Quit {}", config.app_name),
                    true,
                    None,
                );
                quit.set_key_accelerator(Some(muda::accelerator::KeyAccelerator::new(
                    Some(muda::accelerator::Modifiers::SUPER),
                    muda::accelerator::Key::Character("q".into()),
                )))
                .map_err(|e| e.to_string())?;
                apple.append(&quit).map_err(|e| e.to_string())?;
            } else {
                apple
                    .append(&PredefinedMenuItem::quit(None))
                    .map_err(|e| e.to_string())?;
            }
            menu.append(&apple).map_err(|e| e.to_string())?;
            for group in &config.menus {
                let sub = Submenu::new(&group.label, true);
                for command in &group.commands {
                    let item = MenuItem::with_id(
                        format!("volt-command-{}", command.id),
                        &command.label,
                        command.enabled,
                        None,
                    );
                    if let Some(shortcut) = commands
                        .iter()
                        .find(|c| c.id == command.id)
                        .and_then(|c| c.shortcut)
                    {
                        item.set_key_accelerator(Some(shortcut.menu_accelerator()))
                            .map_err(|e| e.to_string())?;
                    }
                    sub.append(&item).map_err(|e| e.to_string())?;
                    command_items.push(CommandItem {
                        id: command.id.clone(),
                        item,
                        declared_enabled: command.enabled,
                        current_enabled: Cell::new(command.enabled),
                    });
                }
                menu.append(&sub).map_err(|e| e.to_string())?;
            }
            let tray = if let Some(tray) = &config.tray {
                let tray_menu = Menu::new();
                for command in &tray.commands {
                    let item = MenuItem::with_id(
                        format!("volt-command-{}", command.id),
                        &command.label,
                        command.enabled,
                        None,
                    );
                    // Tray menus are another source of the same IDs, not a
                    // second registration of application keyboard accelerators.
                    tray_menu.append(&item).map_err(|e| e.to_string())?;
                    command_items.push(CommandItem {
                        id: command.id.clone(),
                        item,
                        declared_enabled: command.enabled,
                        current_enabled: Cell::new(command.enabled),
                    });
                }
                let icon = tray_icon::Icon::from_rgba(tray.rgba.clone(), tray.width, tray.height)
                    .map_err(|e| e.to_string())?;
                Some(
                    tray_icon::TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu))
                        .with_icon(icon)
                        .with_icon_as_template(tray.template)
                        .with_tooltip(&tray.tooltip)
                        .build()
                        .map_err(|e| e.to_string())?,
                )
            } else {
                None
            };
            // muda's handler is process-global and one-shot: standalone single-host only.
            muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
                if guarded_quit && event.id.0 == "volt-host-quit" {
                    send(DesktopEvent::Quit);
                } else if let Some(id) = event.id.0.strip_prefix("volt-command-") {
                    send(DesktopEvent::Command(id.to_owned()));
                }
            }));
            menu.init_for_nsapp();
            let host = Self {
                menu,
                _tray: tray,
                command_items,
            };
            host.sync_commands(commands, true);
            Ok(host)
        }
        pub fn sync_commands(&self, commands: &[crate::native::CommandInfo<'_>], allowed: bool) {
            for item in &self.command_items {
                let enabled = allowed
                    && item.declared_enabled
                    && commands
                        .iter()
                        .find(|c| c.id == item.id)
                        .is_none_or(|c| c.enabled);
                // No repeated AppKit writes for ordinary unchanged input/scroll.
                if item.current_enabled.replace(enabled) != enabled {
                    item.item.set_enabled(enabled);
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_definitions_are_bounded_consistent_and_disabled_gated() {
        let config = DesktopConfig::new("App").menu(DesktopMenu::new(
            "File",
            [
                DesktopCommand::new("save", "Save"),
                DesktopCommand::new("disabled", "Disabled").enabled(false),
            ],
        ));
        assert!(config.validate().is_ok());
        assert!(config.accepts("save"));
        assert!(!config.accepts("disabled"));
        assert!(!config.accepts("unknown"));
        assert!(DesktopConfig::new("App")
            .menu(DesktopMenu::new(
                "File",
                [DesktopCommand::new("bad/id", "Save")]
            ))
            .validate()
            .is_err());
    }
    #[test]
    fn unimplemented_services_never_claim_support() {
        let c = capabilities();
        assert_eq!(
            c.notifications,
            super::super::notifications::capabilities().local_requests
        );
        assert!(!c.embedded_web && !c.maps && !c.app_intents && !c.on_device_ai);
    }
}
