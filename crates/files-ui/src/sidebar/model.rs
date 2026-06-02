use crate::shell::navigation::NavigationTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSectionKind {
    Home,
    Pinned,
    Library,
    Drives,
    Cloud,
    Network,
    Wsl,
    FileTags,
}

#[derive(Debug, Clone)]
pub struct SidebarSection {
    pub kind: SidebarSectionKind,
    pub title: String,
    pub entries: Vec<SidebarEntry>,
}

#[derive(Debug, Clone)]
pub struct SidebarEntry {
    pub label: String,
    pub target: NavigationTarget,
    /// Pinned section: path is in `settings.json` (reorderable).
    pub pinned_in_settings: bool,
    /// File-tag section: color saved in settings.
    pub color: Option<String>,
}
