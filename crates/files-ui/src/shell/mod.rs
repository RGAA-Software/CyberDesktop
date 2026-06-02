#[cfg(feature = "full-app")]
mod actions;

#[cfg(feature = "full-app")]
pub use actions::ReopenClosedTabAt;
#[cfg(feature = "full-app")]
pub(crate) mod app_menus;
#[cfg(feature = "full-app")]
mod app_shell;
#[cfg(feature = "full-app")]
pub mod navigation;
#[cfg(feature = "full-app")]
mod pane_shell;
#[cfg(feature = "full-app")]
pub mod preferences;
#[cfg(feature = "full-app")]
mod shell_panes;
mod window;

#[cfg(feature = "full-app")]
pub use pane_shell::PaneShell;
#[cfg(feature = "full-app")]
pub use shell_panes::{PaneSide, ShellPanes};
#[cfg(feature = "full-app")]
pub use window::open_main_window;

#[cfg(feature = "full-app")]
pub(crate) use actions::*;
