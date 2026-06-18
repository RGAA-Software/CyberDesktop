use std::path::PathBuf;

use gpui::SharedString;
use rust_i18n::t;

/// Where a tab's main content is focused (Files: path string, "Home", settings, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Home,
    Path(PathBuf),
    RecycleBin,
    Settings,
    /// Files sidebar file tag: flat list of paths tagged with this name.
    FileTag(String),
    /// Global folder search results for `query`.
    SearchResults {
        query: String,
    },
}

impl NavigationTarget {
    pub fn tab_title(&self) -> SharedString {
        match self {
            NavigationTarget::Home => SharedString::from("Home"),
            NavigationTarget::RecycleBin => SharedString::from("Recycle Bin"),
            NavigationTarget::Settings => SharedString::from("Settings"),
            NavigationTarget::FileTag(name) => SharedString::from(format!("Tag: {name}")),
            NavigationTarget::SearchResults { query } => {
                SharedString::from(format!("Search: {query}"))
            }
            NavigationTarget::Path(path) => {
                if path.to_string_lossy() == r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}" {
                    SharedString::from(t!("sidebar.network_places").to_string())
                } else {
                    SharedString::from(
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string()),
                    )
                }
            }
        }
    }

    /// Decode a value persisted in `session_tabs` / `session_closed_tabs`.
    pub fn decode_session_tab(tab_key: &str) -> Self {
        if tab_key == "home" {
            return NavigationTarget::Home;
        }
        if tab_key == "recycle" {
            return NavigationTarget::RecycleBin;
        }
        if tab_key == "settings" {
            return NavigationTarget::Home;
        }
        if let Some(name) = tab_key.strip_prefix("tag:") {
            return NavigationTarget::FileTag(name.to_string());
        }
        if let Some(query) = tab_key.strip_prefix("search:") {
            return NavigationTarget::SearchResults {
                query: query.to_string(),
            };
        }
        let path = PathBuf::from(tab_key);
        if path.is_dir() {
            NavigationTarget::Path(path)
        } else {
            NavigationTarget::Home
        }
    }

    /// Title for a closed-tab menu row from its persisted `tab` key.
    pub fn label_for_session_tab(tab_key: &str) -> SharedString {
        Self::decode_session_tab(tab_key).tab_title()
    }

    pub fn toolbar_path_label(&self) -> String {
        match self {
            NavigationTarget::Home => "Home".to_string(),
            NavigationTarget::RecycleBin => "Recycle Bin".to_string(),
            NavigationTarget::Settings => "Settings".to_string(),
            NavigationTarget::FileTag(name) => name.clone(),
            NavigationTarget::SearchResults { query } => query.clone(),
            NavigationTarget::Path(path) => {
                if path.to_string_lossy() == r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}" {
                    t!("sidebar.network_places").to_string()
                } else {
                    path.to_string_lossy().to_string()
                }
            }
        }
    }
}
