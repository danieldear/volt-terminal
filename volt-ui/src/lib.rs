pub mod app;
pub mod chat_panel;
pub mod display_link;
pub mod editor;
pub mod menu;
pub mod pane_tree;
pub mod prompt;
pub mod tab;
pub mod tab_layout;
pub mod task_form;
pub mod tasks;
pub mod theme_editor;
pub use app::App;

pub mod workspace_panel;

pub mod workspace_git;
pub mod workspace_project;

pub mod workspace_search;

#[cfg(target_os = "macos")]
mod native_text;
#[cfg(target_os = "macos")]
mod secure_input;

mod keybindings;

mod links;

#[cfg(any(target_os = "macos", test))]
mod shell_integration;

pub mod settings;
mod shell_startup;
