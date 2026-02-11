// TUI Widgets - Reusable interactive components

pub mod dialog;
pub mod input;
pub mod progress;
pub mod notification;
pub mod menu;
pub mod stats_bar;
pub mod footer;
pub mod quick_jump;

pub use dialog::{Dialog, DialogType};
pub use input::{InputDialog, InputField};
pub use progress::ProgressBar;
pub use notification::{Notification, NotificationManager};
pub use menu::{Menu, MenuItem};
pub use stats_bar::StatsBar;
pub use footer::{Footer, ViewContext};
pub use quick_jump::QuickJumpMenu;
