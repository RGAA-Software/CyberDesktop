use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DriveInfo {
    pub path: PathBuf,
    /// Primary line (volume label or drive root).
    pub label: String,
    pub volume_label: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub is_removable: bool,
    pub is_network: bool,
}

impl DriveInfo {
    pub fn space_text(&self) -> Option<String> {
        let total = self.total_bytes?;
        let free = self.free_bytes?;
        let used = total.saturating_sub(free);
        Some(format!("{} / {}", format_bytes(used), format_bytes(total)))
    }

    pub fn used_fraction(&self) -> Option<f32> {
        let total = self.total_bytes?;
        let free = self.free_bytes?;
        if total == 0 {
            return None;
        }
        Some((total.saturating_sub(free) as f32) / total as f32)
    }
}

/// Windows system drive root (e.g. `C:\`), from `SystemDrive`.
pub fn system_drive_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        Some(PathBuf::from(format!("{}\\", drive.trim_end_matches('\\'))))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// True when `path` is the system drive root (e.g. `C:\`).
pub fn is_system_drive(path: &Path) -> bool {
    system_drive_root().is_some_and(|root| path == root)
}

/// Lists ready local drive roots (e.g. `C:\`, `D:\`).
pub fn list_drives() -> Vec<DriveInfo> {
    #[cfg(windows)]
    {
        list_windows_drives()
    }

    #[cfg(not(windows))]
    {
        vec![DriveInfo {
            path: PathBuf::from("/"),
            label: "Root".to_string(),
            volume_label: None,
            total_bytes: None,
            free_bytes: None,
            is_removable: false,
            is_network: false,
        }]
    }
}

#[cfg(windows)]
fn list_windows_drives() -> Vec<DriveInfo> {
    use app_platform_windows::{volume_details, DriveKind};

    let mut drives = Vec::new();

    for letter in b'A'..=b'Z' {
        let root = format!("{}:\\", letter as char);
        let path = PathBuf::from(&root);
        if !path.exists() {
            continue;
        }
        let details = volume_details(&path);
        if details.kind == DriveKind::Removable {
            let empty = details
                .total_bytes
                .zip(details.free_bytes)
                .is_some_and(|(t, f)| t > 0 && f == t);
            if empty {
                continue;
            }
        }
        let volume_label = details.volume_label.clone();
        let label = volume_label
            .as_ref()
            .filter(|name| !name.is_empty())
            .map(|name| format!("{root} ({name})"))
            .unwrap_or_else(|| root.clone());
        drives.push(DriveInfo {
            path,
            label,
            volume_label,
            total_bytes: details.total_bytes,
            free_bytes: details.free_bytes,
            is_removable: details.kind == DriveKind::Removable,
            is_network: details.kind == DriveKind::Remote,
        });
    }

    drives
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{value:.1} {UN}", UN = UNITS[unit])
    }
}

pub fn default_user_profile() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|p| p.exists())
}

pub fn home_navigation_path() -> PathBuf {
    default_user_profile()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// User Desktop folder (Send to desktop shortcut).
pub fn user_desktop_directory() -> Option<PathBuf> {
    let profile = default_user_profile()?;
    let desktop = profile.join("Desktop");
    if desktop.is_dir() {
        return Some(desktop);
    }
    let onedrive_desktop = profile.join("OneDrive").join("Desktop");
    if onedrive_desktop.is_dir() {
        return Some(onedrive_desktop);
    }
    None
}
