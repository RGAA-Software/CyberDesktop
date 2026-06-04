use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, OnceLock, RwLock};
use std::thread;
use std::time::Duration;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::{FILES_CONFIG_APP_ID, WINDOW_HEIGHT, WINDOW_WIDTH};

/// Default size of the standalone CyberFiles settings window. These dimensions
/// must not be persisted as the main file browser window size.
const SETTINGS_WINDOW_WIDTH: f32 = 1120.0;
const SETTINGS_WINDOW_HEIGHT: f32 = 720.0;

const CONFIG_SAVE_DEBOUNCE_MS: u64 = 300;

static CONFIG_APP_ID: OnceLock<&'static str> = OnceLock::new();
static CONFIG_CACHE: OnceLock<RwLock<AppConfig>> = OnceLock::new();
static CONFIG_CACHE_INITIALIZED: AtomicBool = AtomicBool::new(false);
static CONFIG_FLUSH_TX: OnceLock<mpsc::Sender<()>> = OnceLock::new();

/// Persisted file browser view: `details`, `grid`, or `columns`.
pub const VIEW_DETAILS: &str = "details";
pub const VIEW_LIST: &str = "list";
pub const VIEW_GRID: &str = "grid";
pub const VIEW_CARDS: &str = "cards";
pub const VIEW_COLUMNS: &str = "columns";

pub const GROUP_NONE: &str = "none";
pub const GROUP_NAME: &str = "name";
pub const GROUP_MODIFIED: &str = "modified";
pub const GROUP_CREATED: &str = "created";
pub const GROUP_SIZE: &str = "size";
pub const GROUP_TYPE: &str = "type";
pub const GROUP_TAG: &str = "tag";
pub const GROUP_DATE_YEAR: &str = "year";
pub const GROUP_DATE_MONTH: &str = "month";
pub const GROUP_DATE_DAY: &str = "day";

const CONFIG_FILE: &str = "settings.json";

/// Persisted user preferences (written on save, applied on next launch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub locale: String,
    pub dark_mode: bool,
    pub theme_name: String,
    pub font_size: f32,
    pub border_radius: f32,
    pub scrollbar_show: String,
    pub list_active_highlight: bool,
    pub window_width: f32,
    pub window_height: f32,
    #[serde(default)]
    pub pinned_folders: Vec<String>,
    #[serde(default = "default_show_info_pane")]
    pub show_info_pane: bool,
    #[serde(default = "default_file_view_mode")]
    pub file_view_mode: String,
    #[serde(default)]
    pub file_sort_option: Option<String>,
    #[serde(default)]
    pub file_sort_direction: Option<String>,
    #[serde(default)]
    pub file_group_option: Option<String>,
    #[serde(default)]
    pub file_group_date_unit: Option<String>,
    #[serde(default)]
    pub file_show_hidden: Option<bool>,
    #[serde(default = "default_show_file_extensions")]
    pub show_file_extensions: bool,
    /// Sidebar width mode: `expanded`, `compact` (icon), `minimal` (offcanvas).
    #[serde(default = "default_sidebar_display_mode")]
    pub sidebar_display_mode: String,
    #[serde(default)]
    pub sidebar_collapsed: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_pinned: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_library: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_drives: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_cloud: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_network: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_wsl: bool,
    #[serde(default = "default_true")]
    pub show_sidebar_section_file_tags: bool,
    /// User-defined file tags for sidebar (full tag system is future work).
    #[serde(default)]
    pub file_tags: Vec<FileTagConfig>,
    /// Home page widget visibility (Files `Show*Widget`).
    #[serde(default = "default_true")]
    pub show_home_quick_access: bool,
    #[serde(default = "default_true")]
    pub show_home_drives: bool,
    #[serde(default = "default_true")]
    pub show_home_network: bool,
    #[serde(default = "default_true")]
    pub show_home_file_tags: bool,
    #[serde(default = "default_true")]
    pub show_home_recent: bool,
    /// Home widget expander state (Files `*WidgetExpanded`).
    #[serde(default = "default_true")]
    pub home_quick_access_expanded: bool,
    #[serde(default = "default_true")]
    pub home_drives_expanded: bool,
    #[serde(default = "default_true")]
    pub home_network_expanded: bool,
    #[serde(default = "default_true")]
    pub home_file_tags_expanded: bool,
    #[serde(default = "default_true")]
    pub home_recent_expanded: bool,
    /// Display order of Home widgets (`quick_access`, `drives`, `network`, `file_tags`, `recent`).
    #[serde(default = "default_home_widget_order")]
    pub home_widget_order: Vec<String>,
    /// When true, extra Shell verbs beyond the first few appear in a «More» submenu (Files default).
    #[serde(default = "default_true")]
    pub context_menu_shell_extensions_submenu: bool,
    /// Built-in item visibility in the file list context menu (Files Settings → Context menu).
    #[serde(default = "default_true")]
    pub context_menu_show_compress: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_extract: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_send_to: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_pin: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_open_in_terminal: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_file_tags: bool,
    #[serde(default = "default_true")]
    pub context_menu_show_create_shortcut: bool,
    /// Show «Open in new pane» in the file list context menu (Files `ShowOpenInNewPane`).
    #[serde(default = "default_true")]
    pub show_open_in_new_pane: bool,
    /// When true, new tabs start in dual-pane mode (Files `AlwaysOpenDualPaneInNewTab`).
    #[serde(default)]
    pub always_open_dual_pane_in_new_tab: bool,
    /// Default split direction for new dual-pane layouts: `vertical` or `horizontal`.
    #[serde(default = "default_shell_pane_arrangement")]
    pub shell_pane_arrangement: String,
    /// When true, open supported text/code files with CyberEditor on double-click.
    #[serde(default = "default_true")]
    pub open_text_with_cybereditor: bool,
    /// When true, open supported audio/video files with CyberMediaPlayer on double-click.
    #[serde(default = "default_true")]
    pub open_media_with_cybermediaplayer: bool,
    /// When true, GPUI disables Windows DirectComposition for the main Files window.
    #[serde(default = "default_true")]
    pub disable_direct_composition: bool,
    /// When true, reopen the last saved set of tabs on application launch.
    #[serde(default = "default_true")]
    pub auto_restore_tabs: bool,
    /// Per-tab dual-pane layout (same order as session tabs stored in SQLite).
    #[serde(default)]
    pub session_pane_layouts: Vec<SessionPaneLayout>,
    /// Recently closed tabs (most recent first) for reopen.
    #[serde(default)]
    pub session_closed_tabs: Vec<ClosedTabSession>,
    /// User overrides for command key bindings (`action_id` → gpui keystroke string).
    #[serde(default)]
    pub keybindings: BTreeMap<String, String>,
}

/// Snapshot of a closed tab (target + dual-pane layout).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosedTabSession {
    pub tab: String,
    pub pane_layout: SessionPaneLayout,
}

/// Dual-pane state for one tab (`ShellPanes`).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionPaneLayout {
    #[serde(default)]
    pub dual_pane: bool,
    /// `primary` or `secondary`.
    #[serde(default = "default_session_pane_side")]
    pub active_side: String,
    /// Encoded navigation target for the primary pane (same format as `session_tabs`).
    #[serde(default)]
    pub primary_tab: String,
    /// Encoded navigation target for the secondary pane.
    #[serde(default)]
    pub secondary_tab: String,
    /// `vertical` (side-by-side) or `horizontal` (stacked). D2 layout uses this.
    #[serde(default = "default_shell_pane_arrangement")]
    pub arrangement: String,
    /// Primary pane share along the split axis (0.0–1.0). D2 splitter uses this.
    #[serde(default = "default_split_ratio")]
    pub split_ratio: f32,
}

fn default_session_pane_side() -> String {
    "primary".into()
}

fn default_shell_pane_arrangement() -> String {
    "vertical".into()
}

fn default_split_ratio() -> f32 {
    0.5
}

/// Sidebar file tag entry (Files `FileTagsManager` subset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileTagConfig {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub paths: Vec<String>,
}

fn default_sidebar_display_mode() -> String {
    "expanded".into()
}

fn default_true() -> bool {
    true
}

fn default_show_info_pane() -> bool {
    true
}

fn default_file_view_mode() -> String {
    VIEW_DETAILS.into()
}

fn default_show_file_extensions() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            locale: "en".into(),
            dark_mode: false,
            theme_name: "Ant".into(),
            font_size: 16.,
            border_radius: 6.,
            scrollbar_show: "scrolling".into(),
            list_active_highlight: false,
            window_width: WINDOW_WIDTH,
            window_height: WINDOW_HEIGHT,
            pinned_folders: Vec::new(),
            show_info_pane: true,
            file_view_mode: default_file_view_mode(),
            file_sort_option: None,
            file_sort_direction: None,
            file_group_option: None,
            file_group_date_unit: None,
            file_show_hidden: None,
            show_file_extensions: default_show_file_extensions(),
            sidebar_display_mode: default_sidebar_display_mode(),
            sidebar_collapsed: false,
            show_sidebar_section_pinned: true,
            show_sidebar_section_library: true,
            show_sidebar_section_drives: true,
            show_sidebar_section_cloud: true,
            show_sidebar_section_network: true,
            show_sidebar_section_wsl: true,
            show_sidebar_section_file_tags: true,
            file_tags: Vec::new(),
            show_home_quick_access: true,
            show_home_drives: true,
            show_home_network: true,
            show_home_file_tags: true,
            show_home_recent: true,
            home_quick_access_expanded: true,
            home_drives_expanded: true,
            home_network_expanded: true,
            home_file_tags_expanded: true,
            home_recent_expanded: true,
            home_widget_order: default_home_widget_order(),
            context_menu_shell_extensions_submenu: true,
            context_menu_show_compress: true,
            context_menu_show_extract: true,
            context_menu_show_send_to: true,
            context_menu_show_pin: true,
            context_menu_show_open_in_terminal: true,
            context_menu_show_file_tags: true,
            context_menu_show_create_shortcut: true,
            show_open_in_new_pane: true,
            always_open_dual_pane_in_new_tab: false,
            shell_pane_arrangement: default_shell_pane_arrangement(),
            open_text_with_cybereditor: true,
            open_media_with_cybermediaplayer: true,
            disable_direct_composition: true,
            auto_restore_tabs: true,
            session_pane_layouts: Vec::new(),
            session_closed_tabs: Vec::new(),
            keybindings: BTreeMap::new(),
        }
    }
}

pub fn default_home_widget_order() -> Vec<String> {
    vec![
        "quick_access".into(),
        "drives".into(),
        "network".into(),
        "file_tags".into(),
        "recent".into(),
    ]
}

/// Ensures `order` contains every known widget id once, in a stable order.
pub fn normalize_home_widget_order(order: &[String]) -> Vec<String> {
    let known = default_home_widget_order();
    let mut normalized: Vec<String> = order
        .iter()
        .filter(|id| known.iter().any(|k| k == *id))
        .cloned()
        .collect();
    for id in known {
        if !normalized.iter().any(|existing| existing == &id) {
            normalized.push(id);
        }
    }
    normalized
}

/// Home widget visibility from settings.
pub fn home_widget_prefs() -> HomeWidgetPrefs {
    load_config().map(HomeWidgetPrefs::from).unwrap_or_default()
}

/// Persisted Home widget show/expand flags.
#[derive(Debug, Clone)]
pub struct HomeWidgetPrefs {
    pub show_quick_access: bool,
    pub show_drives: bool,
    pub show_network: bool,
    pub show_file_tags: bool,
    pub show_recent: bool,
    pub quick_access_expanded: bool,
    pub drives_expanded: bool,
    pub network_expanded: bool,
    pub file_tags_expanded: bool,
    pub recent_expanded: bool,
    pub widget_order: Vec<String>,
}

impl Default for HomeWidgetPrefs {
    fn default() -> Self {
        Self {
            show_quick_access: true,
            show_drives: true,
            show_network: true,
            show_file_tags: true,
            show_recent: true,
            quick_access_expanded: true,
            drives_expanded: true,
            network_expanded: true,
            file_tags_expanded: true,
            recent_expanded: true,
            widget_order: default_home_widget_order(),
        }
    }
}

impl HomeWidgetPrefs {
    pub fn widget_order_normalized(&self) -> Vec<String> {
        normalize_home_widget_order(&self.widget_order)
    }

    pub fn is_widget_visible(&self, id: &str) -> bool {
        match id {
            "quick_access" => self.show_quick_access,
            "drives" => self.show_drives,
            "network" => self.show_network,
            "file_tags" => self.show_file_tags,
            "recent" => self.show_recent,
            _ => false,
        }
    }

    pub fn move_widget(&mut self, id: &str, up: bool) {
        let mut order = self.widget_order_normalized();
        let Some(pos) = order.iter().position(|entry| entry == id) else {
            return;
        };
        let target = if up {
            pos.saturating_sub(1)
        } else {
            (pos + 1).min(order.len().saturating_sub(1))
        };
        if target == pos {
            return;
        }
        let entry = order.remove(pos);
        order.insert(target, entry);
        self.widget_order = order;
    }
}

impl From<AppConfig> for HomeWidgetPrefs {
    fn from(c: AppConfig) -> Self {
        Self {
            show_quick_access: c.show_home_quick_access,
            show_drives: c.show_home_drives,
            show_network: c.show_home_network,
            show_file_tags: c.show_home_file_tags,
            show_recent: c.show_home_recent,
            quick_access_expanded: c.home_quick_access_expanded,
            drives_expanded: c.home_drives_expanded,
            network_expanded: c.home_network_expanded,
            file_tags_expanded: c.home_file_tags_expanded,
            recent_expanded: c.home_recent_expanded,
            widget_order: normalize_home_widget_order(&c.home_widget_order),
        }
    }
}

/// Built-in CyberFiles context menu entries (not Shell verbs).
#[derive(Debug, Clone, Copy)]
pub struct ContextMenuItemPrefs {
    pub compress: bool,
    pub extract: bool,
    pub send_to: bool,
    pub pin: bool,
    pub open_in_terminal: bool,
    pub file_tags: bool,
    pub create_shortcut: bool,
    pub open_in_new_pane: bool,
}

impl Default for ContextMenuItemPrefs {
    fn default() -> Self {
        Self {
            compress: true,
            extract: true,
            send_to: true,
            pin: true,
            open_in_terminal: true,
            file_tags: true,
            create_shortcut: true,
            open_in_new_pane: true,
        }
    }
}

impl From<&AppConfig> for ContextMenuItemPrefs {
    fn from(c: &AppConfig) -> Self {
        Self {
            compress: c.context_menu_show_compress,
            extract: c.context_menu_show_extract,
            send_to: c.context_menu_show_send_to,
            pin: c.context_menu_show_pin,
            open_in_terminal: c.context_menu_show_open_in_terminal,
            file_tags: c.context_menu_show_file_tags,
            create_shortcut: c.context_menu_show_create_shortcut,
            open_in_new_pane: c.show_open_in_new_pane,
        }
    }
}

pub fn context_menu_item_prefs() -> ContextMenuItemPrefs {
    load_config()
        .map(|c| ContextMenuItemPrefs::from(&c))
        .unwrap_or_default()
}

pub fn open_text_with_cybereditor_enabled() -> bool {
    load_config()
        .map(|c| c.open_text_with_cybereditor)
        .unwrap_or(true)
}

pub fn open_media_with_cybermediaplayer_enabled() -> bool {
    load_config()
        .map(|c| c.open_media_with_cybermediaplayer)
        .unwrap_or(true)
}

pub fn disable_direct_composition_enabled() -> bool {
    load_config()
        .map(|c| c.disable_direct_composition)
        .unwrap_or(true)
}

pub fn save_home_widget_prefs(prefs: &HomeWidgetPrefs) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config.show_home_quick_access = prefs.show_quick_access;
    config.show_home_drives = prefs.show_drives;
    config.show_home_network = prefs.show_network;
    config.show_home_file_tags = prefs.show_file_tags;
    config.show_home_recent = prefs.show_recent;
    config.home_quick_access_expanded = prefs.quick_access_expanded;
    config.home_drives_expanded = prefs.drives_expanded;
    config.home_network_expanded = prefs.network_expanded;
    config.home_file_tags_expanded = prefs.file_tags_expanded;
    config.home_recent_expanded = prefs.recent_expanded;
    config.home_widget_order = prefs.widget_order_normalized();
    save_config(&config)
}

/// Sidebar is icon-only when `sidebar_display_mode == "compact"`.
pub fn sidebar_is_compact(config: &AppConfig) -> bool {
    config.sidebar_display_mode == "compact"
}

pub fn sidebar_is_offcanvas(config: &AppConfig) -> bool {
    config.sidebar_display_mode == "minimal"
}

/// Updates file-browser fields in settings and writes `settings.json`.
pub fn save_file_browser_prefs(
    view_mode: &str,
    sort_option: &str,
    sort_direction: &str,
    group_option: &str,
    group_date_unit: &str,
    show_hidden: bool,
    show_file_extensions: bool,
) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config.file_view_mode = view_mode.to_string();
    config.file_sort_option = Some(sort_option.to_string());
    config.file_sort_direction = Some(sort_direction.to_string());
    config.file_group_option = Some(group_option.to_string());
    config.file_group_date_unit = Some(group_date_unit.to_string());
    config.file_show_hidden = Some(show_hidden);
    config.show_file_extensions = show_file_extensions;
    save_config(&config)
}

pub fn file_group_date_unit_from_config() -> String {
    load_config()
        .and_then(|c| c.file_group_date_unit)
        .unwrap_or_else(|| GROUP_DATE_YEAR.to_string())
}

pub fn file_group_from_config() -> String {
    load_config()
        .and_then(|c| c.file_group_option)
        .unwrap_or_else(|| GROUP_NONE.to_string())
}

pub fn file_view_mode_from_config() -> String {
    load_config()
        .map(|c| c.file_view_mode)
        .unwrap_or_else(default_file_view_mode)
}

pub fn file_sort_prefs_from_config() -> (Option<String>, Option<String>, Option<bool>, bool) {
    load_config()
        .map(|c| {
            (
                c.file_sort_option,
                c.file_sort_direction,
                c.file_show_hidden,
                c.show_file_extensions,
            )
        })
        .unwrap_or((None, None, None, true))
}

pub fn pinned_folder_paths() -> Vec<PathBuf> {
    load_config()
        .map(|c| {
            c.pinned_folders
                .into_iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

/// Selects which on-disk settings file to use. Call once at process startup
/// (before any `load_config` / `save_config`) with the exe id, e.g. `cyber_files`.
pub fn set_config_app_id(id: &'static str) {
    let _ = CONFIG_APP_ID.set(id);
}

fn config_app_id() -> &'static str {
    CONFIG_APP_ID
        .get()
        .copied()
        .expect("set_config_app_id must be called before using config")
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join(CONFIG_FILE))
}

pub(crate) fn config_dir() -> Option<PathBuf> {
    ProjectDirs::from("com", "cyber_desktop", config_app_id()).map(|dirs| dirs.config_dir().into())
}

/// Loads settings from the in-memory cache (disk on first access).
///
/// Returns `None` when the config directory is unavailable, or when no settings
/// file exists yet and the cache has not been initialized by a save.
pub fn load_config() -> Option<AppConfig> {
    let path = config_path()?;
    if !CONFIG_CACHE_INITIALIZED.load(Ordering::Acquire) && !path.exists() {
        return None;
    }
    Some(
        config_cache()
            .read()
            .expect("config cache poisoned")
            .clone(),
    )
}

/// Updates the in-memory cache and schedules a debounced background write.
pub fn save_config(config: &AppConfig) -> anyhow::Result<()> {
    config_path().ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?;
    *config_cache().write().expect("config cache poisoned") = config.clone();
    ensure_config_flush_worker();
    schedule_config_flush();
    Ok(())
}

/// Writes the current cache to disk immediately (e.g. on application exit).
pub fn flush_config() {
    let Some(cache) = CONFIG_CACHE.get() else {
        return;
    };
    let Ok(config) = cache.read() else {
        return;
    };
    if let Err(err) = write_config_to_disk(&config) {
        tracing::error!(target: "config", error = %err, "failed to flush config");
    }
}

fn config_cache() -> &'static RwLock<AppConfig> {
    CONFIG_CACHE.get_or_init(|| {
        CONFIG_CACHE_INITIALIZED.store(true, Ordering::Release);
        RwLock::new(read_config_from_disk().unwrap_or_default())
    })
}

fn read_config_from_disk() -> Option<AppConfig> {
    let path = config_path()?;
    read_config_file(&path)
}

fn read_config_file(path: &PathBuf) -> Option<AppConfig> {
    let data = fs::read_to_string(path).ok()?;
    let mut config: AppConfig = serde_json::from_str(&data).ok()?;
    let (width, height) =
        normalize_main_window_size(config.window_width, config.window_height);
    config.window_width = width;
    config.window_height = height;
    Some(config)
}

fn normalize_main_window_size(width: f32, height: f32) -> (f32, f32) {
    if config_app_id() == FILES_CONFIG_APP_ID
        && (width - SETTINGS_WINDOW_WIDTH).abs() < 0.5
        && (height - SETTINGS_WINDOW_HEIGHT).abs() < 0.5
    {
        return (WINDOW_WIDTH, WINDOW_HEIGHT);
    }
    (width, height)
}

fn ensure_config_flush_worker() {
    CONFIG_FLUSH_TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("config-flush".into())
            .spawn(move || config_flush_worker(rx))
            .ok();
        tx
    });
}

fn config_flush_worker(rx: mpsc::Receiver<()>) {
    while rx.recv().is_ok() {
        while rx
            .recv_timeout(Duration::from_millis(CONFIG_SAVE_DEBOUNCE_MS))
            .is_ok()
        {}
        let Some(cache) = CONFIG_CACHE.get() else {
            continue;
        };
        let Ok(config) = cache.read() else {
            continue;
        };
        if let Err(err) = write_config_to_disk(&config) {
            tracing::error!(target: "config", error = %err, "failed to save config");
        }
    }
}

fn schedule_config_flush() {
    if let Some(tx) = CONFIG_FLUSH_TX.get() {
        let _ = tx.send(());
    }
}

fn write_config_to_disk(config: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("config directory unavailable"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string(config)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, json)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

pub fn window_size() -> (f32, f32) {
    load_config()
        .map(|c| normalize_main_window_size(c.window_width, c.window_height))
        .unwrap_or((WINDOW_WIDTH, WINDOW_HEIGHT))
}

pub fn keybinding_overrides() -> BTreeMap<String, String> {
    load_config()
        .map(|c| c.keybindings)
        .unwrap_or_default()
}

pub fn save_keybinding_override(action_id: &str, keystroke: &str) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config
        .keybindings
        .insert(action_id.to_string(), keystroke.to_string());
    save_config(&config)
}

pub fn reset_keybinding_override(action_id: &str) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config.keybindings.remove(action_id);
    save_config(&config)
}

pub fn reset_all_keybinding_overrides() -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    config.keybindings.clear();
    save_config(&config)
}
