use std::path::PathBuf;

use gpui::SharedString;

/// Where a tab's main content is focused (Files: path string, "Home", settings, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationTarget {
    Home,
    Path(PathBuf),
    RecycleBin,
    Settings,
    /// Files sidebar file tag: flat list of paths tagged with this name.
    FileTag(String),
}

impl NavigationTarget {
    pub fn tab_title(&self) -> SharedString {
        match self {
            NavigationTarget::Home => SharedString::from("Home"),
            NavigationTarget::RecycleBin => SharedString::from("Recycle Bin"),
            NavigationTarget::Settings => SharedString::from("Settings"),
            NavigationTarget::FileTag(name) => SharedString::from(format!("Tag: {name}")),
            NavigationTarget::Path(path) => SharedString::from(
                path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
            ),
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
            return NavigationTarget::Settings;
        }
        if let Some(name) = tab_key.strip_prefix("tag:") {
            return NavigationTarget::FileTag(name.to_string());
        }
        let path = PathBuf::from(tab_key);
        if path.is_dir() {
            NavigationTarget::Path(path)
        } else if path.parent().is_some_and(|p| p.is_dir()) {
            NavigationTarget::Path(path.parent().unwrap().to_path_buf())
        } else {
            NavigationTarget::Home
        }
    }

    /// Title for a closed-tab menu row from its persisted `tab` key.
    pub fn label_for_session_tab(tab_key: &str) -> SharedString {
        let target = Self::decode_session_tab(tab_key);
        match &target {
            NavigationTarget::Path(_) => {
                let path = PathBuf::from(tab_key);
                SharedString::from(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                )
            }
            _ => target.tab_title(),
        }
    }

    pub fn toolbar_path_label(&self) -> String {
        match self {
            NavigationTarget::Home => "Home".to_string(),
            NavigationTarget::RecycleBin => "Recycle Bin".to_string(),
            NavigationTarget::Settings => "Settings".to_string(),
            NavigationTarget::FileTag(name) => name.clone(),
            NavigationTarget::Path(path) => path.to_string_lossy().to_string(),
        }
    }
}
