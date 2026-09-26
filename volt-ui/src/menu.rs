//! Native macOS application menu bar and right-click context menu, built
//! with `muda`. Both post `MenuEvent`s to the same global channel; `MenuAction`
//! is the shared vocabulary `App::user_event` resolves them into so keyboard
//! shortcuts and menu clicks drive the exact same `MainState`/`App` methods.
#![cfg(target_os = "macos")]

use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

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
    ToggleChatPanel,
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
/// nothing keeps the *returned* `Menu` itself alive. If the caller lets the
/// return value drop, every `Rc` in the whole tree hits zero, the Rust-side
/// backing data is freed, and AppKit's native menu bar — which is retained
/// separately and keeps working visually — is left holding dangling
/// pointers. The first click on any item then reads freed memory: this
/// reproducibly aborted the app (SIGABRT inside `CFStringCreate`, confirmed
/// via crash report) the first time it happened here, when this function
/// returned `()` and the caller had nowhere to keep the tree alive. The
/// caller must store the returned `Menu` for the app's full lifetime (e.g.
/// as an `App` field) — dropping it after this call reintroduces the bug.
#[must_use = "dropping this frees the menu bar's Rust-side backing data \
              while AppKit's native menu keeps running — see doc comment"]
pub fn install_app_menu() -> Menu {
    let menu = Menu::new();

    let app_menu = Submenu::new("Volt", true);
    let _ = app_menu.append_items(&[
        &PredefinedMenuItem::about(Some("About Volt"), None),
        &PredefinedMenuItem::separator(),
        &MenuAction::OpenSettings.item("Settings…"),
        &MenuAction::ReloadSettings.item("Reload Settings"),
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
        &MenuAction::ToggleChatPanel.item("Toggle AI Chat Panel (Prototype)"),
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
    menu
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
pub fn build_context_menu(read_only: bool, has_selection: bool) -> Menu {
    let menu = Menu::new();
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
        &MenuAction::ChangeTerminalTitle.item("Change Terminal Title…"),
        &PredefinedMenuItem::separator(),
        &MenuAction::SearchGoogle.item_enabled("Search With Google", has_selection),
    ]);
    menu
}
