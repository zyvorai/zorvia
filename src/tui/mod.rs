// TUI module for interactive terminal interface

pub mod theme;
pub mod config;
pub mod colors;

pub use theme::Theme;
pub use config::{TuiConfig, UiConfig, BehaviorConfig, KeybindingsConfig};
