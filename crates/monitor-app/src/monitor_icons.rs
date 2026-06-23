//! [Tabler Icons](https://tabler.io/icons) paths for CyberMonitor (outline, 24×24).

use gpui::px;
use gpui_component::{Icon, IconName, Sizable, Size};

macro_rules! tabler_path {
    ($name:literal) => {
        concat!("icons/tabler/", $name, ".svg")
    };
}

pub const OVERVIEW: &str = tabler_path!("chart-pie");
pub const CPU: &str = tabler_path!("cpu");
pub const MEMORY: &str = tabler_path!("device-sd-card");
pub const GPU: &str = tabler_path!("device-gpu");
pub const STORAGE: &str = tabler_path!("database");
pub const NETWORK: &str = tabler_path!("network");
pub const PROCESS: &str = tabler_path!("list");
pub const SERVICE: &str = tabler_path!("star");
pub const STARTUP: &str = tabler_path!("arrow-right");
pub const USERS: &str = tabler_path!("users");

pub const REFRESH: &str = tabler_path!("refresh");
pub const SUN: &str = tabler_path!("sun");
pub const MOON: &str = tabler_path!("moon");
pub const SETTINGS: &str = tabler_path!("settings");

/// Full-color sidebar / brand logo (`app-assets/assets/app/logo/ic_cyber_monitor.svg`).
pub const APP_LOGO_PATH: &str = "app/logo/ic_cyber_monitor.svg";

const NAV_ICON: Size = Size::Size(px(18.));
const TOPBAR_ICON: Size = Size::Size(px(16.));

pub fn nav_icon(path: &'static str) -> Icon {
    Icon::new(IconName::File).path(path).with_size(NAV_ICON)
}

pub fn topbar_icon(path: &'static str) -> Icon {
    Icon::new(IconName::File).path(path).with_size(TOPBAR_ICON)
}

pub fn theme_toggle_icon(is_dark: bool) -> Icon {
    topbar_icon(if is_dark { SUN } else { MOON })
}
