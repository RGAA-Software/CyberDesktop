//! Enumerate children of Shell known folders (Files sidebar sections).

use std::path::PathBuf;

use windows::core::{PCWSTR, GUID};
use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, HKEY_LOCAL_MACHINE, KEY_READ};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::{
    IEnumIDList, ILFree, IShellFolder, SHGetDesktopFolder, SHGetKnownFolderIDList, StrRetToStrW,
    KF_FLAG_DEFAULT, SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHGDNF, SHGDN_FORPARSING,
    SHGDN_INFOLDER,
};

use crate::com::ensure_com_apartment;

const SFGAO_FOLDER: u32 = 0x2000_0000;

/// `{3936e9e4-d92c-4eee-a85a-bc16d5ea0819}` — Frequent / Quick Access pins.
pub const FOLDERID_FREQUENT: GUID = GUID::from_u128(0x3936e9e4_d92c_4eee_a85a_bc16d5ea0819);
/// `{a992df1a-173b-439a-8746-4720baa52538}` — Libraries.
pub const FOLDERID_LIBRARIES: GUID = GUID::from_u128(0xa992df1a_173b_439a_8746_4720baa52538);
/// `{C5ABBF53-E17F-4121-8900-86626FC2C973}` — Network (NetHood).
pub const FOLDERID_NETWORK: GUID = GUID::from_u128(0xC5ABBF53_E17F_4121_8900_86626FC2C973);

/// One folder entry from a Shell namespace.
#[derive(Debug, Clone)]
pub struct ShellFolderEntry {
    pub display_name: String,
    pub path: PathBuf,
}

/// Lists folder children under a known folder id (folders only, parsing paths).
pub fn list_known_folder_folders(folder_id: &GUID) -> anyhow::Result<Vec<ShellFolderEntry>> {
    ensure_com_apartment()?;
    unsafe { list_known_folder_folders_inner(folder_id) }
}

unsafe fn display_name_of(
    folder: &IShellFolder,
    pidl: *const ITEMIDLIST,
    flags: SHGDNF,
) -> anyhow::Result<String> {
    let mut strret = STRRET::default();
    folder.GetDisplayNameOf(pidl, flags, &mut strret)?;
    let mut psz: windows::core::PWSTR = windows::core::PWSTR::null();
    StrRetToStrW(&mut strret, Some(pidl), &mut psz)?;
    let name = psz.to_string()?;
    windows::Win32::System::Com::CoTaskMemFree(Some(psz.0 as *mut _));
    Ok(name)
}

unsafe fn is_folder_item(folder: &IShellFolder, pidl: *const ITEMIDLIST) -> bool {
    let apidl = [pidl];
    let mut attrs = 0u32;
    folder.GetAttributesOf(&apidl, &mut attrs).is_ok() && attrs & SFGAO_FOLDER != 0
}

unsafe fn enum_folder_pidls(folder: &IShellFolder) -> anyhow::Result<Vec<*mut ITEMIDLIST>> {
    let mut enum_id: Option<IEnumIDList> = None;
    let flags = (SHCONTF_FOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32;
    let hr = folder.EnumObjects(HWND::default(), flags, &mut enum_id);
    if hr != S_OK {
        anyhow::bail!("EnumObjects failed: {hr:?}");
    }
    let Some(enum_id) = enum_id else {
        return Ok(Vec::new());
    };

    let mut pidls = Vec::new();
    loop {
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        let mut fetched = 0u32;
        let mut batch = [pidl];
        let hr = enum_id.Next(&mut batch, Some(&mut fetched));
        if hr != S_OK || fetched == 0 {
            break;
        }
        pidl = batch[0];
        if pidl.is_null() {
            break;
        }
        pidls.push(pidl);
    }
    Ok(pidls)
}

unsafe fn list_known_folder_folders_inner(
    folder_id: &GUID,
) -> anyhow::Result<Vec<ShellFolderEntry>> {
    let desktop: IShellFolder = SHGetDesktopFolder()?;
    let folder_pidl = SHGetKnownFolderIDList(folder_id, KF_FLAG_DEFAULT.0 as u32, None)?;
    let shell_folder: IShellFolder = desktop.BindToObject(folder_pidl, None)?;

    let mut entries = Vec::new();
    for pidl in enum_folder_pidls(&shell_folder)? {
        if !is_folder_item(&shell_folder, pidl) {
            ILFree(Some(pidl));
            continue;
        }
        let display_name = display_name_of(&shell_folder, pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing = display_name_of(&shell_folder, pidl, SHGDN_FORPARSING).unwrap_or_default();
        let path = PathBuf::from(&parsing);
        if path.as_os_str().is_empty() {
            ILFree(Some(pidl));
            continue;
        }
        if path.is_absolute() || parsing.starts_with(r"\\") {
            entries.push(ShellFolderEntry { display_name, path });
        }
        ILFree(Some(pidl));
    }

    ILFree(Some(folder_pidl));
    Ok(entries)
}

/// Cloud sync roots (OneDrive, etc.) under the user profile.
#[cfg(windows)]
pub fn list_cloud_drive_roots() -> Vec<ShellFolderEntry> {
    let mut entries = Vec::new();
    let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return entries;
    };
    let Ok(read) = std::fs::read_dir(&profile) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let is_cloud = name.eq_ignore_ascii_case("OneDrive")
            || name.starts_with("OneDrive-")
            || name.contains("Google Drive")
            || name.contains("Dropbox");
        if is_cloud {
            entries.push(ShellFolderEntry {
                display_name: name,
                path,
            });
        }
    }
    entries
}

#[cfg(not(windows))]
pub fn list_cloud_drive_roots() -> Vec<ShellFolderEntry> {
    Vec::new()
}

/// Check whether WSL is installed by looking at the registry.
pub fn wsl_installed() -> bool {
    unsafe {
        let subkey_wide: Vec<u16> = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Lxss\MSI"
            .encode_utf16()
            .chain([0])
            .collect();
        let mut key = windows::Win32::System::Registry::HKEY::default();
        let ok = RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        )
        .is_ok();
        if ok {
            let _ = RegCloseKey(key);
        }
        ok
    }
}

/// WSL distributions under `\wsl.localhost\` or `\wsl$\`.
#[cfg(windows)]
pub fn list_wsl_distro_roots() -> Vec<ShellFolderEntry> {
    if !wsl_installed() {
        return Vec::new();
    }
    for root in [r"\wsl.localhost\", r"\wsl$\"] {
        let path = PathBuf::from(root);
        // Skip the expensive .exists() call; just try to read_dir and catch errors.
        let Ok(read) = std::fs::read_dir(&path) else {
            continue;
        };
        let mut entries = Vec::new();
        for entry in read.flatten() {
            let distro_path = entry.path();
            if !distro_path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.is_empty() {
                continue;
            }
            entries.push(ShellFolderEntry {
                display_name: name,
                path: distro_path,
            });
        }
        if !entries.is_empty() {
            entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));
            return entries;
        }
    }
    Vec::new()
}

#[cfg(not(windows))]
pub fn list_wsl_distro_roots() -> Vec<ShellFolderEntry> {
    Vec::new()
}
