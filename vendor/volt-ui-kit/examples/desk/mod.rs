mod state;
mod views;
pub use state::{parse_args, theme, App};
pub use views::view;
#[cfg(test)]
mod tests;
