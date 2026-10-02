//! Native macOS application menu bar and right-click context menu, built
//! with `muda`. Both post `MenuEvent`s to the same global channel; `MenuAction`
//! is the shared vocabulary `App::user_event` resolves them into so keyboard
//! shortcuts and menu clicks drive the exact same `MainState`/`App` methods.
#![cfg(target_os = "macos")]

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use volt_renderer::tab_color::TabColor;

/// Every action a menu-bar item or context-menu item can trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    NewWindow,
    NewTab,
    CloseWindow,
    CloseTab,
    Copy,
    Paste,
    Find,
    SearchWorkspace,
    ToggleChatPanel,
    ToggleWorkspaceLayout,
    IncreaseFontSize,
    DecreaseFontSize,
    ToggleFullScreen,
    Zoom,
    OpenSettings,
    ReloadSettings,
    SplitRight,
    SplitLeft,
    SplitDown,
    SplitUp,
    ResetTerminal,
    ToggleInspector,
    ToggleReadOnly,
    ChangeTabTitle,
    ChangeTerminalTitle,
    SearchGoogle,
    CustomizeTheme,
    OpenThemesFolder,
    ToggleSecureInput,
}

/// Menu ids of theme items are this prefix plus the theme id.
const THEME_ID_PREFIX: &str = "volt.theme:";
const TAB_COLOR_ID_PREFIX: &str = "volt.tab_color:";

pub fn tab_color_menu_id(color: Option<TabColor>) -> String {
    format!(
        "{TAB_COLOR_ID_PREFIX}{}",
        color.map_or("clear", TabColor::id)
    )
}

/// `Some(None)` clears the accent; plain `None` means unrelated/invalid ID.
pub fn tab_color_from_id(id: &str) -> Option<Option<TabColor>> {
    let value = id.strip_prefix(TAB_COLOR_ID_PREFIX)?;
    if value == "clear" {
        Some(None)
    } else {
        TabColor::from_id(value).map(Some)
    }
}

fn tab_color_submenu(selected: Option<TabColor>) -> Submenu {
    let menu = Submenu::new("Tab Color", true);
    let _ = menu.append(&CheckMenuItem::with_id(
        tab_color_menu_id(None),
        "No Color",
        true,
        selected.is_none(),
        None,
    ));
    let _ = menu.append(&PredefinedMenuItem::separator());
    for color in TabColor::ALL {
        let _ = menu.append(&CheckMenuItem::with_id(
            tab_color_menu_id(Some(color)),
            color.label(),
            true,
            selected == Some(color),
            None,
        ));
    }
    menu
}

/// The theme id a Volt ▸ Theme item stands for, if `id` is one.
pub fn theme_from_id(id: &str) -> Option<&str> {
    id.strip_prefix(THEME_ID_PREFIX)
}

impl MenuAction {
    fn id(self) -> &'static str {
        use MenuAction::*;
        match self {
            NewWindow => "volt.new_window",
            NewTab => "volt.new_tab",
            CloseWindow => "volt.close_window",
            CloseTab => "volt.close_tab",
            Copy => "volt.copy",
            Paste => "volt.paste",
            Find => "volt.find",
            SearchWorkspace => "volt.search_workspace",
            ToggleWorkspaceLayout => "volt.toggle_workspace_layout",
            ToggleChatPanel => "volt.toggle_chat_panel",
            IncreaseFontSize => "volt.increase_font_size",
            DecreaseFontSize => "volt.decrease_font_size",
            ToggleFullScreen => "volt.toggle_fullscreen",
            Zoom => "volt.zoom",
            OpenSettings => "volt.open_settings",
            ReloadSettings => "volt.reload_settings",
            SplitRight => "volt.split_right",
            SplitLeft => "volt.split_left",
            SplitDown => "volt.split_down",
            SplitUp => "volt.split_up",
            ResetTerminal => "volt.reset_terminal",
            ToggleInspector => "volt.toggle_inspector",
            ToggleReadOnly => "volt.toggle_read_only",
            ChangeTabTitle => "volt.change_tab_title",
            ChangeTerminalTitle => "volt.change_terminal_title",
            SearchGoogle => "volt.search_google",
            CustomizeTheme => "volt.customize_theme",
            OpenThemesFolder => "volt.open_themes_folder",
            ToggleSecureInput => "volt.toggle_secure_input",
        }
    }

    /// Resolve a `muda::MenuEvent`'s id back into an action. Returns `None`
    /// for ids muda owns itself (predefined items with no id we assigned).
    pub fn from_id(id: &str) -> Option<Self> {
        use MenuAction::*;
        Some(match id {
            "volt.new_window" => NewWindow,
            "volt.new_tab" => NewTab,
            "volt.close_window" => CloseWindow,
            "volt.close_tab" => CloseTab,
            "volt.copy" => Copy,
            "volt.paste" => Paste,
            "volt.find" => Find,
            "volt.search_workspace" => SearchWorkspace,
            "volt.toggle_workspace_layout" => ToggleWorkspaceLayout,
            "volt.toggle_chat_panel" => ToggleChatPanel,
            "volt.increase_font_size" => IncreaseFontSize,
            "volt.decrease_font_size" => DecreaseFontSize,
            "volt.toggle_fullscreen" => ToggleFullScreen,
            "volt.zoom" => Zoom,
            "volt.open_settings" => OpenSettings,
            "volt.reload_settings" => ReloadSettings,
            "volt.split_right" => SplitRight,
            "volt.split_left" => SplitLeft,
            "volt.split_down" => SplitDown,
            "volt.split_up" => SplitUp,
            "volt.reset_terminal" => ResetTerminal,
            "volt.toggle_inspector" => ToggleInspector,
            "volt.toggle_read_only" => ToggleReadOnly,
            "volt.change_tab_title" => ChangeTabTitle,
            "volt.change_terminal_title" => ChangeTerminalTitle,
            "volt.search_google" => SearchGoogle,
            "volt.customize_theme" => CustomizeTheme,
            "volt.open_themes_folder" => OpenThemesFolder,
            "volt.toggle_secure_input" => ToggleSecureInput,
            _ => return None,
        })
    }

    fn item(self, text: &str) -> MenuItem {
        MenuItem::with_id(self.id(), text, true, None)
    }

    fn item_enabled(self, text: &str, enabled: bool) -> MenuItem {
        MenuItem::with_id(self.id(), text, enabled, None)
    }
}

/// Build and install the native application menu bar. `NSApp.mainMenu` is
/// process-global — call this once at startup, not per-window.
///
/// None of our own items carry a keyboard `Accelerator`. Giving custom
/// `MenuItem`s a Cmd-key accelerator here reproducibly crashed the app
/// (SIGABRT, an uncaught Objective-C exception thrown inside AppKit's
/// `-[NSApplication _handleEvent:]` during key-equivalent matching — before
/// any of our Rust code ever runs) on the first press of an accelerated key
/// (e.g. Cmd+T). Every shortcut in this menu is already implemented,
/// unaffected, in `app.rs`'s own `KeyboardInput` handler — the accelerator
/// was purely a cosmetic hint in the menu text, so dropping it costs no
/// functionality, only that hint. `PredefinedMenuItem`s (Quit, Minimize,
/// Hide, …) keep their standard OS-defined accelerators; those go through
/// AppKit's own target/action wiring, not ours, and aren't implicated.
///
/// **Must be kept alive by the caller.** Every native `NSMenuItem` muda
/// creates stores a raw pointer back into its Rust-side `MenuChild`; a
/// parent's `append_items` clones an `Rc` to keep children alive, but
/// nothing keeps the *returned* root `Menu` itself alive. If the caller lets the
/// return value drop, every `Rc` in the whole tree hits zero, the Rust-side
/// backing data is freed, and AppKit's native menu bar — which is retained
/// separately and keeps working visually — is left holding dangling
/// pointers. The first click on any item then reads freed memory: this
/// reproducibly aborted the app (SIGABRT inside `CFStringCreate`, confirmed
/// via crash report) the first time it happened here, when this function
/// returned `()` and the caller had nowhere to keep the tree alive. The
/// caller must store the returned `AppMenu` for the app's full lifetime (e.g.
/// as an `App` field) — dropping it after this call reintroduces the bug.
#[must_use = "dropping this frees the menu bar's Rust-side backing data \
              while AppKit's native menu keeps running — see doc comment"]
pub fn install_app_menu() -> AppMenu {
    let menu = Menu::new();
    let theme_menu = Submenu::new("Theme", true);
    let secure_input_item = CheckMenuItem::with_id(
        MenuAction::ToggleSecureInput.id(),
        "Secure Keyboard Entry (manual)",
        true,
        false,
        None,
    );

    let app_menu = Submenu::new("Volt", true);
    let _ = app_menu.append_items(&[
        &PredefinedMenuItem::about(Some("About Volt"), None),
        &PredefinedMenuItem::separator(),
        &MenuAction::OpenSettings.item("Settings…"),
        &MenuAction::ReloadSettings.item("Reload Settings"),
        &theme_menu,
        &secure_input_item,
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::services(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::hide(None),
        &PredefinedMenuItem::hide_others(None),
        &PredefinedMenuItem::show_all(None),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::quit(None),
    ]);

    let file_menu = Submenu::new("File", true);
    let _ = file_menu.append_items(&[
        &MenuAction::NewWindow.item("New Window"),
        &MenuAction::NewTab.item("New Tab"),
        &PredefinedMenuItem::separator(),
        &MenuAction::CloseWindow.item("Close Window"),
        &MenuAction::CloseTab.item("Close Tab"),
    ]);

    let edit_menu = Submenu::new("Edit", true);
    let _ = edit_menu.append_items(&[
        &MenuAction::Copy.item("Copy"),
        &MenuAction::Paste.item("Paste"),
        &PredefinedMenuItem::separator(),
        &MenuAction::Find.item("Find…"),
    ]);

    let view_menu = Submenu::new("View", true);
    let _ = view_menu.append_items(&[
        &MenuAction::ToggleChatPanel.item("Toggle Workspace Panel"),
        &MenuAction::SearchWorkspace.item("Search Workspace…  ⌘⇧P"),
        &MenuAction::ToggleWorkspaceLayout.item("Switch Workspace: Docked / Floating"),
        &PredefinedMenuItem::separator(),
        &MenuAction::IncreaseFontSize.item("Increase Font Size"),
        &MenuAction::DecreaseFontSize.item("Decrease Font Size"),
        &PredefinedMenuItem::separator(),
        &MenuAction::ToggleFullScreen.item("Toggle Full Screen"),
    ]);

    let window_menu = Submenu::new("Window", true);
    let _ = window_menu.append_items(&[
        &PredefinedMenuItem::minimize(None),
        &MenuAction::Zoom.item("Zoom"),
        &PredefinedMenuItem::separator(),
        &PredefinedMenuItem::bring_all_to_front(None),
    ]);
    // Marks this as the real macOS "Window" menu — AppKit then auto-appends
    // the live window list and handles Minimize/Zoom natively.
    window_menu.set_as_windows_menu_for_nsapp();

    let _ = menu.append_items(&[&app_menu, &file_menu, &edit_menu, &view_menu, &window_menu]);

    menu.init_for_nsapp();
    AppMenu {
        _menu: menu,
        theme_menu,
        theme_items: Vec::new(),
        secure_input_item,
    }
}

/// The installed menu bar plus the handles needed to refresh Volt ▸ Theme.
pub struct AppMenu {
    /// Never read, only kept alive — see `install_app_menu`.
    _menu: Menu,
    theme_menu: Submenu,
    theme_items: Vec<(String, CheckMenuItem)>,
    secure_input_item: CheckMenuItem,
}

impl AppMenu {
    pub fn set_manual_secure_input(&self, enabled: bool) {
        self.secure_input_item.set_checked(enabled);
    }
    /// List `themes` (id, display name, built-in?) with `current` checked:
    /// built-ins, then user themes, then the editor and folder items. Only
    /// rebuilds when the list changed; otherwise just moves the check mark
    /// (AppKit also toggles a clicked item itself, so always resync it).
    pub fn set_themes(&mut self, themes: &[(String, String, bool)], current: &str) {
        let same = self.theme_items.len() == themes.len()
            && self
                .theme_items
                .iter()
                .zip(themes)
                .all(|((id, item), (tid, name, _))| id == tid && item.text() == *name);
        if !same {
            while self.theme_menu.remove_at(0).is_some() {}
            self.theme_items = themes
                .iter()
                .map(|(id, name, _)| {
                    let item = CheckMenuItem::with_id(
                        format!("{THEME_ID_PREFIX}{id}"),
                        name,
                        true,
                        false,
                        None,
                    );
                    (id.clone(), item)
                })
                .collect();
            let builtins = themes.iter().filter(|(_, _, builtin)| *builtin).count();
            for (i, (_, item)) in self.theme_items.iter().enumerate() {
                if i == builtins && i > 0 {
                    let _ = self.theme_menu.append(&PredefinedMenuItem::separator());
                }
                let _ = self.theme_menu.append(item);
            }
            let _ = self.theme_menu.append_items(&[
                &PredefinedMenuItem::separator(),
                &MenuAction::CustomizeTheme.item("Customize Theme…"),
                &MenuAction::OpenThemesFolder.item("Open Themes Folder"),
            ]);
        }
        for (id, item) in &self.theme_items {
            item.set_checked(id == current);
        }
    }
}

/// Build a fresh right-click context menu for the terminal view. Built fresh
/// per click (not cached) so `read_only`/`has_selection` reflect the target
/// pane's actual current state.
///
/// Deliberately not shown, and why:
/// - **Ask Siri**, live spell-check: AppKit appends these only to real
///   `NSTextView` context menus. Volt's terminal grid is custom
///   GPU-rendered, not an `NSTextView`, so the OS never offers them here —
///   no app can add them to a non-text-view menu.
/// - **AutoFill**: macOS's password-manager text-*field* integration; has
///   no meaning against a raw PTY.
/// - **Services submenu**: genuine and dynamic (it lists whatever apps on
///   the machine have registered a service, e.g. another AI assistant
///   offering "Ask <it>") — but a context menu on a terminal is an unusual
///   place for it by macOS convention (Services normally lives on the Edit
///   menu), and it can only ever be shown or hidden wholesale, never
///   filtered to specific apps — the OS decides what's registered, not
///   Volt. It's kept on the main app menu (Volt ▸ Services) instead, where
///   it's discoverable without cluttering a menu about *this pane*.
pub fn build_context_menu(
    read_only: bool,
    has_selection: bool,
    tab_color: Option<TabColor>,
    custom_tab_bar: bool,
) -> Menu {
    let menu = Menu::new();
    let color_menu = tab_color_submenu(tab_color);
    let _ = menu.append_items(&[
        &MenuAction::Copy.item_enabled("Copy", has_selection),
        &MenuAction::Paste.item("Paste"),
        &PredefinedMenuItem::separator(),
        &MenuAction::SplitRight.item("Split Right"),
        &MenuAction::SplitLeft.item("Split Left"),
        &MenuAction::SplitDown.item("Split Down"),
        &MenuAction::SplitUp.item("Split Up"),
        &PredefinedMenuItem::separator(),
        &MenuAction::ResetTerminal.item("Reset Terminal"),
        &MenuAction::ToggleInspector.item("Toggle Terminal Inspector"),
        &CheckMenuItem::with_id(
            MenuAction::ToggleReadOnly.id(),
            "Terminal Read-only",
            true,
            read_only,
            None,
        ),
        &PredefinedMenuItem::separator(),
        &MenuAction::ChangeTabTitle.item("Change Tab Title…"),
    ]);
    if custom_tab_bar {
        let _ = menu.append(&color_menu);
    }
    let _ = menu.append_items(&[
        &MenuAction::ChangeTerminalTitle.item("Change Terminal Title…"),
        &PredefinedMenuItem::separator(),
        &MenuAction::SearchGoogle.item_enabled("Search With Google", has_selection),
    ]);
    menu
}

/// Right-clicking a tab edits that tab, including a background tab; it does
/// not have to switch the active PTY first.
pub fn build_tab_context_menu(tab_color: Option<TabColor>) -> Menu {
    let menu = Menu::new();
    let color_menu = tab_color_submenu(tab_color);
    let _ = menu.append_items(&[
        &MenuAction::ChangeTabTitle.item("Change Tab Title…"),
        &color_menu,
    ]);
    menu
}

#[cfg(test)]
mod tab_color_tests {
    use super::*;

    #[test]
    fn every_tab_color_menu_id_round_trips_and_clear_is_distinct() {
        for color in volt_renderer::tab_color::TabColor::ALL {
            let id = tab_color_menu_id(Some(color));
            assert_eq!(tab_color_from_id(&id), Some(Some(color)));
        }
        assert_eq!(tab_color_from_id(&tab_color_menu_id(None)), Some(None));
        assert_eq!(tab_color_from_id("volt.theme:amber"), None);
        assert_eq!(tab_color_from_id("volt.tab_color:not-a-color"), None);
    }
}
