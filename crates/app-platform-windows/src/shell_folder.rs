//! Enumerate children of Shell known folders (Files sidebar sections).

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{PCWSTR, GUID};
use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, HKEY_LOCAL_MACHINE, KEY_READ};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::{
    IEnumIDList, ILFree, IShellFolder, SHGetDesktopFolder, SHGetKnownFolderIDList,
    SHParseDisplayName, StrRetToStrW, KF_FLAG_DEFAULT, SHCONTF_ENABLE_ASYNC, SHCONTF_FASTITEMS,
    SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHGDNF, SHGDN_FORPARSING, SHGDN_INFOLDER,
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

unsafe fn enum_folder_pidls(folder: &IShellFolder, enable_async: bool) -> anyhow::Result<Vec<*mut ITEMIDLIST>> {
    let mut enum_id: Option<IEnumIDList> = None;
    let mut flags = (SHCONTF_FOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0 | SHCONTF_FASTITEMS.0) as u32;
    if enable_async {
        flags |= SHCONTF_ENABLE_ASYNC.0 as u32;
    }
    let hr = folder.EnumObjects(HWND::default(), flags, &mut enum_id);
    if hr != S_OK {
        anyhow::bail!("EnumObjects failed: {hr:?}");
    }
    let Some(enum_id) = enum_id else {
        return Ok(Vec::new());
    };

    let mut pidls = Vec::new();
    let start = std::time::Instant::now();
    loop {
        let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        let mut fetched = 0u32;
        let mut batch = [pidl];
        let hr = enum_id.Next(&mut batch, Some(&mut fetched));
        if hr != S_OK || fetched == 0 {
            if enable_async && start.elapsed() < std::time::Duration::from_secs(5) {
                pump_messages(std::time::Duration::from_millis(50));
                continue;
            }
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

/// Pump Windows messages briefly so that async Shell callbacks can complete.
unsafe fn pump_messages(duration: std::time::Duration) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };
    let start = std::time::Instant::now();
    let mut msg = std::mem::zeroed::<MSG>();
    while start.elapsed() < duration {
        if PeekMessageW(&mut msg, HWND::default(), 0, 0, PM_REMOVE).into() {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

unsafe fn list_known_folder_folders_inner(
    folder_id: &GUID,
) -> anyhow::Result<Vec<ShellFolderEntry>> {
    let desktop: IShellFolder = SHGetDesktopFolder()?;
    let folder_pidl = SHGetKnownFolderIDList(folder_id, KF_FLAG_DEFAULT.0 as u32, None)?;
    let shell_folder: IShellFolder = desktop.BindToObject(folder_pidl, None)?;

    let mut entries = Vec::new();
    for pidl in enum_folder_pidls(&shell_folder, false)? {
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
    // Do not gate on wsl_installed() registry check — it is unreliable across
    // WSL install methods (Store vs MSI).
    tracing::info!(target: "wsl", "enumerating WSL distros");

    // std::fs::read_dir does not reliably access the WSL Plan9 UNC paths,
    // so use the wsl.exe CLI which is the most robust method.
    let output = std::process::Command::new("wsl.exe")
        .args(["-l", "--quiet"])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            // wsl.exe outputs UTF-16LE on some Windows versions and UTF-8 on others.
            let text = if output.stdout.windows(2).any(|w| w == [0, 0]) || output.stdout.iter().filter(|&&b| b == 0).count() > 2 {
                // Contains many NUL bytes → treat as UTF-16LE.
                let u16_len = output.stdout.len() / 2;
                let u16_slice: &[u16] = unsafe {
                    std::slice::from_raw_parts(output.stdout.as_ptr() as *const u16, u16_len)
                };
                String::from_utf16_lossy(u16_slice)
            } else {
                String::from_utf8_lossy(&output.stdout).into_owned()
            };
            let mut entries = Vec::new();
            for name in text.lines() {
                let name = name.trim().trim_matches('\0');
                if name.is_empty() || name.contains('\0') {
                    continue;
                }
                // Use \\wsl.localhost\ when available (Win11), else \\wsl$\.
                let path = if std::path::Path::new(r"\\wsl.localhost\").exists() {
                    PathBuf::from(format!(r"\\wsl.localhost\{name}"))
                } else {
                    PathBuf::from(format!(r"\\wsl$\{name}"))
                };
                entries.push(ShellFolderEntry {
                    display_name: name.to_string(),
                    path,
                });
            }
            if !entries.is_empty() {
                entries.sort_by(|a, b| a.display_name.cmp(&b.display_name));
                tracing::info!(target: "wsl", count = entries.len(), "found WSL distros");
                return entries;
            }
        }
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr);
            tracing::warn!(target: "wsl", error = %err, "wsl -l failed");
        }
        Err(e) => {
            tracing::warn!(target: "wsl", error = %e, "wsl.exe not found");
        }
    }

    // Fallback: try UNC paths directly (works on some systems).
    for root in [r"\\wsl.localhost\", r"\\wsl$\"] {
        let path = PathBuf::from(root);
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
            tracing::info!(target: "wsl", count = entries.len(), "found WSL distros via UNC");
            return entries;
        }
    }

    tracing::info!(target: "wsl", "no WSL distros found");
    Vec::new()
}

/// Enumerate network computers from the Shell Network virtual folder.
#[cfg(windows)]
pub fn list_network_computers() -> Vec<ShellFolderEntry> {
    let entries = unsafe { list_network_computers_inner(false) };
    tracing::info!(target: "network", count = entries.len(), "found network computers");
    entries
}

#[cfg(windows)]
unsafe fn list_network_computers_inner(enable_async: bool) -> Vec<ShellFolderEntry> {
    let guid_wide: Vec<u16> = r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"
        .encode_utf16()
        .chain([0])
        .collect();
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(guid_wide.as_ptr()), None, &mut pidl, 0, None).is_err() {
        return Vec::new();
    }
    if pidl.is_null() {
        return Vec::new();
    }

    let desktop: IShellFolder = match SHGetDesktopFolder() {
        Ok(d) => d,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    let shell_folder: IShellFolder = match desktop.BindToObject(pidl, None) {
        Ok(f) => f,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };

    let pidls = match enum_folder_pidls(&shell_folder, enable_async) {
        Ok(p) => p,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    for child_pidl in pidls {
        let display_name = display_name_of(&shell_folder, child_pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing = display_name_of(&shell_folder, child_pidl, SHGDN_FORPARSING).unwrap_or_default();
        if parsing.is_empty() {
            ILFree(Some(child_pidl));
            continue;
        }
        entries.push(ShellFolderEntry {
            display_name,
            path: PathBuf::from(&parsing),
        });
        ILFree(Some(child_pidl));
    }

    ILFree(Some(pidl));
    entries
}

/// Enumerate shares inside a network computer (e.g. \\COMPUTERNAME).
#[cfg(windows)]
pub fn list_network_shares(path: &std::path::Path) -> Vec<ShellFolderEntry> {
    let entries = unsafe { list_network_shares_inner(path) };
    tracing::info!(target: "network", path = %path.display(), count = entries.len(), "found network shares");
    entries
}

#[cfg(windows)]
unsafe fn list_network_shares_inner(path: &std::path::Path) -> Vec<ShellFolderEntry> {
    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(path_wide.as_ptr()), None, &mut pidl, 0, None).is_err() {
        return Vec::new();
    }
    if pidl.is_null() {
        return Vec::new();
    }

    let desktop: IShellFolder = match SHGetDesktopFolder() {
        Ok(d) => d,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    let shell_folder: IShellFolder = match desktop.BindToObject(pidl, None) {
        Ok(f) => f,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };

    let pidls = match enum_folder_pidls(&shell_folder, true) {
        Ok(p) => p,
        Err(_) => {
            ILFree(Some(pidl));
            return Vec::new();
        }
    };

    let mut entries = Vec::new();
    for child_pidl in pidls {
        let display_name = display_name_of(&shell_folder, child_pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing = display_name_of(&shell_folder, child_pidl, SHGDN_FORPARSING).unwrap_or_default();
        if parsing.is_empty() {
            ILFree(Some(child_pidl));
            continue;
        }
        entries.push(ShellFolderEntry {
            display_name,
            path: std::path::PathBuf::from(&parsing),
        });
        ILFree(Some(child_pidl));
    }

    ILFree(Some(pidl));
    entries
}

#[cfg(not(windows))]
pub fn list_network_computers() -> Vec<ShellFolderEntry> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn list_network_shares(_path: &std::path::Path) -> Vec<ShellFolderEntry> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn list_wsl_distro_roots() -> Vec<ShellFolderEntry> {
    Vec::new()
}
