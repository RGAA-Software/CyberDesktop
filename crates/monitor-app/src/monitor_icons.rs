use gpui::px;
use gpui_component::{Icon, Sizable, Size};

// 左侧导航图标（来自 design/cyber_monitor.html）
pub const OVERVIEW: &str = "M5 12a7 7 0 1 0 7-7v7z M12 5v7h7";
pub const CPU: &str =
    "M9 2v3 M15 2v3 M9 19v3 M15 19v3 M2 9h3 M2 15h3 M19 9h3 M19 15h3 M7 7h10v10H7z";
pub const MEMORY: &str = "M4 7h16v10H4z M8 11h8 M8 15h5";
pub const GPU: &str = "M4 7h16v10H4z M8 3v4 M16 3v4 M8 17v4 M16 17v4";
pub const STORAGE: &str = "M5 6h14l1 4v8H4v-8z M7 16h.01 M17 16h.01";
pub const NETWORK: &str = "M4 17h16 M6 17a6 6 0 0 1 12 0 M8 17a4 4 0 0 1 8 0 M12 17v4";
pub const PROCESS: &str = "M4 6h16 M4 12h16 M4 18h16 M8 6v12";
pub const SERVICE: &str = "M12 2l2.2 5.4L20 8l-4 3 1.2 5L12 22l-4.2-6L4 16l4-3-1.2-5z";
pub const STARTUP: &str = "M5 12h14 M13 5l7 7-7 7";
pub const USERS: &str =
    "M16 21v-2a4 4 0 0 0-8 0v2 M12 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8 M22 21v-2a4 4 0 0 0-5-3.9";

// 顶栏图标
pub const REFRESH: &str = "M20 12a8 8 0 1 1-2.34-5.66 M20 4v6h-6";
pub const THEME: &str = "M12 3v2 M12 19v2 M4.93 4.93l1.41 1.41 M17.66 17.66l1.41 1.41 M3 12h2 M19 12h2 M4.93 19.07l1.41-1.41 M17.66 6.34l1.41-1.41 M12 7a5 5 0 1 0 0 10 5 5 0 0 0 0-10z";
pub const SETTINGS: &str = "M12 3l1.6 2.7 3-.1.8 2.8 2.6 1.4-1 2.8 1 2.8-2.6 1.4-.8 2.8-3-.1L12 21l-1.6-2.7-3 .1-.8-2.8L4 14.2l1-2.8-1-2.8 2.6-1.4.8-2.8 3 .1z M12 12m-3 0a3 3 0 1 0 6 0a3 3 0 1 0-6 0";
pub const WINDOW_MINIMIZE: &str = "M6 18h12";
pub const WINDOW_MAXIMIZE: &str = "M6 6h12v12H6z";
pub const WINDOW_CLOSE: &str = "M6 6l12 12 M18 6L6 18";

pub fn icon(path: &'static str) -> Icon {
    Icon::empty().path(path).with_size(Size::Size(px(18.)))
}
