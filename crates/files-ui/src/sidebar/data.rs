use std::collections::HashSet;
use std::path::{Path, PathBuf};

use files_core::{AppConfig, FileTagConfig};
use files_fs::list_drives;
#[cfg(windows)]
use app_platform_windows::{
    list_cloud_drive_roots, list_known_folder_folders, list_wsl_distro_roots, FOLDERID_LIBRARIES,
    FOLDERID_NETWORK,
};

use crate::shell::navigation::NavigationTarget;

use super::model::{SidebarEntry, SidebarSection, SidebarSectionKind};

pub fn build_sidebar_sections(config: &AppConfig) -> Vec<SidebarSection> {
    files_core::log_startup_step("sidebar_build_sections_begin");
    let mut sections = Vec::new();

    sections.push(SidebarSection {
        kind: SidebarSectionKind::Home,
        title: rust_i18n::t!("sidebar.section.quick_access").to_string(),
        entries: vec![
            SidebarEntry {
                label: rust_i18n::t!("nav.home").to_string(),
                target: NavigationTarget::Home,
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            },
            SidebarEntry {
                label: rust_i18n::t!("nav.recycle_bin").to_string(),
                target: NavigationTarget::RecycleBin,
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            },
        ],
    });

    if config.show_sidebar_section_pinned {
        let entries = files_core::time_startup_step("sidebar_section_pinned", || {
            load_pinned_entries(config)
        });
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Pinned,
                title: rust_i18n::t!("sidebar.section.pinned").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_library {
        let entries = files_core::time_startup_step("sidebar_section_library", load_library_entries);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Library,
                title: rust_i18n::t!("sidebar.section.library").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_drives {
        let entries = files_core::time_startup_step("sidebar_section_drives", load_drive_entries);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Drives,
                title: rust_i18n::t!("sidebar.section.drives").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_cloud {
        let entries = files_core::time_startup_step("sidebar_section_cloud", load_cloud_entries);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Cloud,
                title: rust_i18n::t!("sidebar.section.cloud").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_network {
        let entries = files_core::time_startup_step("sidebar_section_network", load_network_entries);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Network,
                title: rust_i18n::t!("sidebar.section.network").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_wsl {
        let entries = files_core::time_startup_step("sidebar_section_wsl", load_wsl_entries);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::Wsl,
                title: rust_i18n::t!("sidebar.section.wsl").to_string(),
                entries,
            });
        }
    }

    if config.show_sidebar_section_file_tags {
        let entries = load_file_tag_entries(&config.file_tags);
        if !entries.is_empty() {
            sections.push(SidebarSection {
                kind: SidebarSectionKind::FileTags,
                title: rust_i18n::t!("sidebar.section.file_tags").to_string(),
                entries,
            });
        }
    }

    files_core::log_startup_step("sidebar_build_sections_done");
    sections
}

fn load_pinned_entries(config: &AppConfig) -> Vec<SidebarEntry> {
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    for path_str in &config.pinned_folders {
        let path = PathBuf::from(path_str);
        if !path.exists() || !seen.insert(path_key(&path)) {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        entries.push(SidebarEntry {
            label,
            target: NavigationTarget::Path(path),
            pinned_in_settings: true,
            color: None,
            usage_fraction: None,
        });
    }

    entries
}

fn load_library_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_LIBRARIES)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| e.path.exists())
            .map(|e| SidebarEntry {
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_drive_entries() -> Vec<SidebarEntry> {
    list_drives()
        .into_iter()
        .map(|drive| {
            let usage_fraction = drive.used_fraction();
            SidebarEntry {
                label: drive.label,
                target: NavigationTarget::Path(drive.path),
                pinned_in_settings: false,
                color: None,
                usage_fraction,
            }
        })
        .collect()
}

fn load_cloud_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_cloud_drive_roots()
            .into_iter()
            .filter(|e| e.path.exists())
            .map(|e| SidebarEntry {
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_network_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_NETWORK)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.path.as_os_str().is_empty())
            .map(|e| SidebarEntry {
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_wsl_entries() -> Vec<SidebarEntry> {
    #[cfg(windows)]
    {
        list_wsl_distro_roots()
            .into_iter()
            .map(|e| SidebarEntry {
                label: e.display_name,
                target: NavigationTarget::Path(e.path),
                pinned_in_settings: false,
                color: None,
                usage_fraction: None,
            })
            .collect()
    }
    #[cfg(not(windows))]
    Vec::new()
}

fn load_file_tag_entries(tags: &[FileTagConfig]) -> Vec<SidebarEntry> {
    tags.iter()
        .filter(|t| !t.name.is_empty())
        .map(|tag| SidebarEntry {
            label: tag.name.clone(),
            target: NavigationTarget::FileTag(tag.name.clone()),
            pinned_in_settings: false,
            color: tag.color.clone(),
            usage_fraction: None,
        })
        .collect()
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}
