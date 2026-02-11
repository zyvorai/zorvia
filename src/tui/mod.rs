// TUI module for interactive terminal interface

pub mod theme;
pub mod config;
pub mod colors;
pub mod app;
pub mod state;
pub mod ui;
pub mod widgets;
pub mod interactive_app;
pub mod splash;

pub use theme::Theme;
pub use config::{TuiConfig, UiConfig, BehaviorConfig, KeybindingsConfig};
pub use app::App;
pub use state::AppState;
pub use interactive_app::InteractiveApp;
pub use splash::SplashScreen;
