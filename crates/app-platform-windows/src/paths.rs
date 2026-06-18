use std::path::PathBuf;

use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

/// `{645FF040-5081-101B-9F08-00AA002F954E}`
const FOLDERID_RECYCLE_BIN: GUID = GUID::from_u128(0x645FF040_5081_101B_9F08_00AA002F954E);

const FOLDERID_DESKTOP: GUID = GUID::from_u128(0xB4B3BA95_50F9_4AF9_8F86_ED3A474518D9);
const FOLDERID_DOCUMENTS: GUID = GUID::from_u128(0xFDD39AD0_238F_46AF_ADB4_6C85480369C7);
const FOLDERID_DOWNLOADS: GUID = GUID::from_u128(0x374DE290_123F_4565_9164_39C4925E467B);
const FOLDERID_MUSIC: GUID = GUID::from_u128(0x4DDC7D2C_9018_4302_978F_EBF0F72821CB);
const FOLDERID_VIDEOS: GUID = GUID::from_u128(0x18989B1D_99B5_455B_841C_AB7E844679E9);
const FOLDERID_PICTURES: GUID = GUID::from_u128(0x33E28130_4E1E_4676_835A_9835C3BC3BB4);

/// Built-in user folder shown on the Home quick-access widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultUserFolderKind {
    Desktop,
    Documents,
    Downloads,
    Music,
    Videos,
    Pictures,
}

/// One of the user-profile folders always shown on the Home quick-access widget.
#[derive(Debug, Clone)]
pub struct DefaultUserFolder {
    pub kind: DefaultUserFolderKind,
    pub display_name: String,
    pub path: PathBuf,
}

/// Returns the six default user folders in Explorer order (Desktop, Documents,
/// Downloads, Music, Videos, Pictures). Every entry is always returned; paths
/// fall back to the user profile when Shell lookup fails and are not filtered
/// by whether the directory exists yet (OneDrive redirects, etc.).
pub fn list_default_user_folders() -> Vec<DefaultUserFolder> {
    const SPECS: &[(DefaultUserFolderKind, GUID, &str)] = &[
        (DefaultUserFolderKind::Desktop, FOLDERID_DESKTOP, "Desktop"),
        (
            DefaultUserFolderKind::Documents,
            FOLDERID_DOCUMENTS,
            "Documents",
        ),
        (
            DefaultUserFolderKind::Downloads,
            FOLDERID_DOWNLOADS,
            "Downloads",
        ),
        (DefaultUserFolderKind::Music, FOLDERID_MUSIC, "Music"),
        (DefaultUserFolderKind::Videos, FOLDERID_VIDEOS, "Videos"),
        (
            DefaultUserFolderKind::Pictures,
            FOLDERID_PICTURES,
            "Pictures",
        ),
    ];
    SPECS
        .iter()
        .map(|(kind, id, profile_subdir)| {
            let path = resolve_user_folder_path(id, profile_subdir);
            let display_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| (*profile_subdir).to_string());
            DefaultUserFolder {
                kind: *kind,
                display_name,
                path,
            }
        })
        .collect()
}

fn resolve_user_folder_path(folder_id: &GUID, profile_subdir: &str) -> PathBuf {
    if let Some(path) = known_folder_path(folder_id) {
        return path;
    }
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return PathBuf::from(profile_subdir);
    };
    if profile_subdir == "Desktop" {
        let desktop = profile.join("Desktop");
        if desktop.is_dir() {
            return desktop;
        }
        let onedrive_desktop = profile.join("OneDrive").join("Desktop");
        if onedrive_desktop.is_dir() {
            return onedrive_desktop;
        }
        return desktop;
    }
    profile.join(profile_subdir)
}

fn known_folder_path(folder_id: &GUID) -> Option<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(folder_id, KF_FLAG_DEFAULT, None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        let _ = CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}

/// Same namespace string as Files (`Constants.UserEnvironmentPaths.RecycleBinPath`).
pub const SHELL_RECYCLE_BIN_PATH: &str = "Shell:RecycleBinFolder";

/// True when `path` is the shell recycle-bin folder (or inside it).
pub fn is_recycle_bin_path(path: &std::path::Path) -> bool {
    recycle_bin_folder()
        .map(|root| path == root || path.starts_with(&root))
        .unwrap_or(false)
}

/// Returns the shell recycle-bin folder path when available.
pub fn recycle_bin_folder() -> Option<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_RECYCLE_BIN, KF_FLAG_DEFAULT, None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        let _ = CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}
