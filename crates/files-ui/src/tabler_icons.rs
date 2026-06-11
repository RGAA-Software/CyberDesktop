#![allow(dead_code)]
//! [Tabler Icons](https://tabler.io/icons) paths for CyberFiles UI (outline, 24×24).

use gpui::{px, Pixels};
use gpui_component::{Icon, IconName, Sizable as _, Size};

pub const APP_ICON_PX: Size = Size::Size(px(18.));

macro_rules! tabler_path {
    ($name:literal) => {
        concat!("icons/tabler/", $name, ".svg")
    };
}

pub const FILES: &str = tabler_path!("files");
pub const HOME: &str = tabler_path!("home");
pub const FOLDER: &str = tabler_path!("folder");
pub const FOLDER_FILLED: &str = tabler_path!("folder-filled");
pub const FOLDER_OFF: &str = tabler_path!("folder-off");
pub const FOLDER_PLUS: &str = tabler_path!("folder-plus");
pub const FOLDER_PIN: &str = tabler_path!("folder-pin");
pub const FOLDER_OPEN: &str = tabler_path!("folder-open");
pub const FILE: &str = tabler_path!("file");
pub const FILE_PLUS: &str = tabler_path!("file-plus");
pub const FILE_ZIP: &str = tabler_path!("file-zip");
pub const PLUS: &str = tabler_path!("plus");
pub const X: &str = tabler_path!("x");
pub const MOON: &str = tabler_path!("moon");
pub const SUN: &str = tabler_path!("sun");
pub const SETTINGS: &str = tabler_path!("settings");
pub const GITHUB: &str = tabler_path!("brand-github");
pub const BELL: &str = tabler_path!("bell");
pub const ARROW_LEFT: &str = tabler_path!("arrow-left");
pub const ARROW_RIGHT: &str = tabler_path!("arrow-right");
pub const ARROW_UP: &str = tabler_path!("arrow-up");
pub const ARROW_BACK_UP: &str = tabler_path!("arrow-back-up");
pub const REFRESH: &str = tabler_path!("refresh");
pub const CHEVRON_RIGHT: &str = tabler_path!("chevron-right");
pub const CHEVRON_DOWN: &str = tabler_path!("chevron-down");
pub const CHEVRON_UP: &str = tabler_path!("chevron-up");
pub const EXTERNAL_LINK: &str = tabler_path!("external-link");
pub const LAYOUT_COLUMNS: &str = tabler_path!("layout-columns");
pub const LAYOUT_SIDEBAR_RIGHT: &str = tabler_path!("layout-sidebar-right");
pub const LAYOUT_SIDEBAR_RIGHT_COLLAPSE: &str = tabler_path!("layout-sidebar-right-collapse");
pub const PIN: &str = tabler_path!("pin");
pub const PINNED: &str = tabler_path!("pinned");
pub const COPY: &str = tabler_path!("copy");
pub const CUT: &str = tabler_path!("cut");
pub const CLIPBOARD: &str = tabler_path!("clipboard");
pub const PENCIL: &str = tabler_path!("pencil");
pub const TRASH: &str = tabler_path!("trash");
pub const DOTS: &str = tabler_path!("dots");
pub const LIST_DETAILS: &str = tabler_path!("list-details");
pub const LIST: &str = tabler_path!("list");
pub const LAYOUT_GRID: &str = tabler_path!("layout-grid");
pub const LAYOUT_BOARD: &str = tabler_path!("layout-board");
pub const COLUMNS_3: &str = tabler_path!("columns-3");
pub const INFO_CIRCLE: &str = tabler_path!("info-circle");
pub const TAG: &str = tabler_path!("tag");
pub const STAR: &str = tabler_path!("star");
pub const STAR_OFF: &str = tabler_path!("star-off");
pub const SORT_ASC: &str = tabler_path!("sort-ascending");
pub const SORT_DESC: &str = tabler_path!("sort-descending");
pub const ARROWS_SORT: &str = tabler_path!("arrows-sort");
pub const EYE: &str = tabler_path!("eye");
pub const EYE_OFF: &str = tabler_path!("eye-off");
pub const DEVICE_DESKTOP: &str = tabler_path!("device-desktop");
pub const NETWORK: &str = tabler_path!("network");
pub const CLOUD: &str = tabler_path!("cloud");
pub const PRINTER: &str = tabler_path!("printer");
pub const CAST: &str = tabler_path!("cast");
pub const ROUTER: &str = tabler_path!("router");
pub const PLUG: &str = tabler_path!("plug");
pub const HELP: &str = tabler_path!("help");
pub const CLOCK: &str = tabler_path!("clock");
pub const HISTORY: &str = tabler_path!("history");
pub const CALENDAR: &str = tabler_path!("calendar");
pub const SEARCH: &str = tabler_path!("search");
pub const TERMINAL: &str = tabler_path!("terminal-2");
pub const INBOX: &str = tabler_path!("inbox");
pub const SORT_LETTERS: &str = tabler_path!("sort-ascending-letters");
pub const WIDGET: &str = tabler_path!("widget");
pub const LINK: &str = tabler_path!("link");
pub const SERVER: &str = tabler_path!("server");
pub const DOWNLOAD: &str = tabler_path!("download");
pub const MUSIC: &str = tabler_path!("music");
pub const DATABASE: &str = tabler_path!("database");
pub const BRAND_WINDOWS: &str = tabler_path!("brand-windows");
pub const MOVIE: &str = tabler_path!("movie");
pub const PHOTO: &str = tabler_path!("photo");
pub const BOOK: &str = tabler_path!("book");
pub const FILE_TEXT: &str = tabler_path!("file-text");
pub const FILE_CODE: &str = tabler_path!("file-code");
pub const FILE_TYPE_PDF: &str = tabler_path!("file-type-pdf");
pub const FILE_TYPE_HTML: &str = tabler_path!("file-type-html");
pub const FILE_TYPE_TS: &str = tabler_path!("file-type-ts");
pub const FILE_TYPE_JS: &str = tabler_path!("file-type-js");
pub const FILE_TYPE_CPP: &str = tabler_path!("file-type-cpp");

/// Render a Tabler SVG at the standard 18px UI size.
pub fn icon(path: &'static str) -> Icon {
    Icon::new(IconName::File).path(path).with_size(APP_ICON_PX)
}

/// Map legacy `IconName` usages to Tabler assets.
pub fn from_icon_name(name: IconName) -> Icon {
    icon(match name {
        IconName::ArrowLeft => ARROW_LEFT,
        IconName::ArrowRight => ARROW_RIGHT,
        IconName::ArrowUp => ARROW_UP,
        IconName::ArrowDown => CHEVRON_DOWN,
        IconName::Redo2 => REFRESH,
        IconName::Moon => MOON,
        IconName::Sun => SUN,
        IconName::Settings2 => SETTINGS,
        IconName::Github => GITHUB,
        IconName::Bell => BELL,
        IconName::Plus => PLUS,
        IconName::Close => X,
        IconName::PanelRightClose => LAYOUT_SIDEBAR_RIGHT_COLLAPSE,
        IconName::PanelRightOpen => LAYOUT_SIDEBAR_RIGHT,
        IconName::LayoutDashboard => LAYOUT_GRID,
        IconName::Replace => CUT,
        IconName::Copy => COPY,
        IconName::File => FILE,
        IconName::Folder => FOLDER,
        IconName::Info => INFO_CIRCLE,
        IconName::Delete => TRASH,
        IconName::GalleryVerticalEnd => LIST_DETAILS,
        IconName::PanelLeftOpen => LIST,
        IconName::PanelLeft => COLUMNS_3,
        IconName::Inbox => INBOX,
        IconName::ExternalLink => EXTERNAL_LINK,
        IconName::Ellipsis => DOTS,
        IconName::ChevronsUpDown => ARROWS_SORT,
        IconName::Star => STAR,
        IconName::StarOff => STAR_OFF,
        IconName::FolderOpen => FOLDER_OPEN,
        IconName::SquareTerminal => TERMINAL,
        IconName::ChevronRight => CHEVRON_RIGHT,
        IconName::ChevronDown => CHEVRON_DOWN,
        IconName::HardDrive => DEVICE_DESKTOP,
        IconName::Calendar => CALENDAR,
        IconName::SortAscending => SORT_ASC,
        IconName::SortDescending => SORT_DESC,
        IconName::ALargeSmall => SORT_LETTERS,
        IconName::Eye => EYE,
        IconName::EyeOff => EYE_OFF,
        IconName::Search => SEARCH,
        IconName::Globe => NETWORK,
        _ => FILE,
    })
}

pub fn logo_px() -> Pixels {
    px(20.)
}
