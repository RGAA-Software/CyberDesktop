//! Enumerate children of Shell known folders (Files sidebar sections).

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::System::Registry::{RegCloseKey, RegOpenKeyExW, HKEY_LOCAL_MACHINE, KEY_READ};
use windows::Win32::System::Variant::VariantToStringAlloc;
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;
use windows::Win32::UI::Shell::{
    IEnumIDList, ILFree, IShellFolder, IShellFolder2, SHGetDesktopFolder, SHGetKnownFolderIDList,
    SHParseDisplayName, StrRetToStrW, KF_FLAG_DEFAULT, SHCONTF_ENABLE_ASYNC, SHCONTF_FASTITEMS,
    SHCONTF_FOLDERS, SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS, SHGDNF, SHGDN_FORPARSING,
    SHGDN_INFOLDER,
};

use crate::com::{ensure_com_apartment, run_sta_task};

const SFGAO_FOLDER: u32 = 0x2000_0000;
const SFGAO_FILESYSTEM: u32 = 0x4000_0000;

/// Category of a network item in the Shell Network folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkItemCategory {
    Computer,
    MediaDevice,
    Printer,
    OtherDevice,
    Infrastructure,
    Unknown,
}

impl NetworkItemCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Computer => "network.category.computer",
            Self::MediaDevice => "network.category.media_device",
            Self::Printer => "network.category.printer",
            Self::OtherDevice => "network.category.other_device",
            Self::Infrastructure => "network.category.infrastructure",
            Self::Unknown => "network.category.unknown",
        }
    }
    pub fn sort_index(&self) -> i64 {
        match self {
            Self::Infrastructure => 0,
            Self::Computer => 1,
            Self::MediaDevice => 2,
            Self::Printer => 3,
            Self::OtherDevice => 4,
            Self::Unknown => 5,
        }
    }
}

fn detect_network_category(
    attrs: u32,
    parsing: &str,
    display_name: &str,
    item_type_text: Option<&str>,
) -> NetworkItemCategory {
    let is_folder = attrs & SFGAO_FOLDER != 0;
    let parsing_lower = parsing.to_ascii_lowercase();
    let name_lower = display_name.to_ascii_lowercase();

    // Primary signal: System.ItemTypeText is the most reliable source for the Explorer-visible category.
    if let Some(itype) = item_type_text {
        let itype_lower = itype.to_ascii_lowercase();
        if itype_lower.contains("computer") {
            return NetworkItemCategory::Computer;
        }
        if itype_lower.contains("media") {
            return NetworkItemCategory::MediaDevice;
        }
        if itype_lower.contains("printer") {
            return NetworkItemCategory::Printer;
        }
        if itype_lower.contains("infrastructure") {
            return NetworkItemCategory::Infrastructure;
        }
        if itype_lower.contains("device") {
            return NetworkItemCategory::OtherDevice;
        }
    }

    // Fallback: use parsing path patterns + SFGAO attributes.
    // Network namespace providers are the most reliable language-independent signal.
    if parsing_lower.contains("microsoft windows network") {
        return NetworkItemCategory::Computer;
    }
    if parsing_lower.contains("media servers") || parsing_lower.contains("dlna") {
        return NetworkItemCategory::MediaDevice;
    }
    if parsing_lower.contains("print providers") || parsing_lower.contains("\\printers") {
        return NetworkItemCategory::Printer;
    }
    if parsing_lower.contains("network infrastructure") {
        return NetworkItemCategory::Infrastructure;
    }
    if parsing_lower.contains("other devices") {
        return NetworkItemCategory::OtherDevice;
    }
    // WSD (Web Services for Devices) is almost always printers/scanners.
    if parsing_lower.contains("microsoft.networking.wsd") {
        return NetworkItemCategory::Printer;
    }
    if is_folder && (parsing.starts_with(r"\\") || parsing.starts_with("\\\\")) {
        return NetworkItemCategory::Computer;
    }
    if parsing_lower.contains("media") || name_lower.contains("media server") {
        return NetworkItemCategory::MediaDevice;
    }
    if parsing_lower.contains("print") || name_lower.contains("printer") {
        return NetworkItemCategory::Printer;
    }
    if parsing_lower.contains("infrastructure") || name_lower.contains("infrastructure") {
        return NetworkItemCategory::Infrastructure;
    }

    // Non-folder items that aren't categorized above = other devices
    if !is_folder {
        return NetworkItemCategory::OtherDevice;
    }

    // Fallback for anything else
    NetworkItemCategory::Unknown
}

unsafe fn item_attributes(folder: &IShellFolder, pidl: *const ITEMIDLIST) -> u32 {
    let apidl = [pidl];
    let mut attrs = SFGAO_FOLDER | SFGAO_FILESYSTEM;
    let _ = folder.GetAttributesOf(&apidl, &mut attrs);
    attrs
}

const PKEY_ITEMTYPETEXT: PROPERTYKEY = PROPERTYKEY {
    fmtid: GUID::from_u128(0xB725F130_47EF_101A_A5F1_02608C9EEBAC),
    pid: 4,
};

/// Query the Explorer-visible category/type text for a Shell namespace child item.
///
/// For the Network folder, the correct category is in `IShellFolder2::GetDetailsOf(pidl, 1)`
/// (the "Category" column), NOT in `System.ItemTypeText` or any property store.
unsafe fn item_type_text_of(
    folder: &IShellFolder,
    _parent_pidl: *const ITEMIDLIST,
    child_pidl: *const ITEMIDLIST,
) -> Option<String> {
    // Primary: IShellFolder2::GetDetailsOf column 1 = "Category".
    // This is exactly what Windows Explorer uses to group items in the Network folder.
    if let Ok(folder2) = folder.cast::<IShellFolder2>() {
        let mut sd = std::mem::zeroed::<windows::Win32::UI::Shell::Common::SHELLDETAILS>();
        if folder2.GetDetailsOf(child_pidl, 1, &mut sd).is_ok() {
            let mut strret = sd.str;
            let mut psz: windows::core::PWSTR = windows::core::PWSTR::null();
            if StrRetToStrW(&mut strret, Some(child_pidl), &mut psz).is_ok() {
                let text = psz.to_string().ok().filter(|s| !s.is_empty());
                CoTaskMemFree(Some(psz.0 as *mut _));
                if text.is_some() {
                    return text;
                }
            }
        }
    }

    // Fallback: IShellFolder2::GetDetailsEx for PKEY_ItemTypeText.
    if let Ok(folder2) = folder.cast::<IShellFolder2>() {
        if let Ok(var) = folder2.GetDetailsEx(child_pidl, &PKEY_ITEMTYPETEXT) {
            if let Ok(pwsz) = VariantToStringAlloc(&var) {
                let text = pwsz.to_string().ok().filter(|s| !s.is_empty());
                CoTaskMemFree(Some(pwsz.0 as *mut _));
                if text.is_some() {
                    return text;
                }
            }
        }
    }

    None
}

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
    pub category: NetworkItemCategory,
    /// Localized type text from `System.ItemTypeText` (e.g. "Computer", "Media Device", "Printer").
    pub item_type_text: Option<String>,
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
    let mut attrs = SFGAO_FOLDER;
    folder.GetAttributesOf(&apidl, &mut attrs).is_ok() && attrs & SFGAO_FOLDER != 0
}

unsafe fn enum_folder_pidls(
    folder: &IShellFolder,
    enable_async: bool,
    extra_flags: u32,
) -> anyhow::Result<Vec<*mut ITEMIDLIST>> {
    let mut enum_id: Option<IEnumIDList> = None;
    let mut flags =
        (SHCONTF_FOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0 | SHCONTF_FASTITEMS.0) as u32 | extra_flags;
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
            let _ = TranslateMessage(&msg);
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
    for pidl in enum_folder_pidls(&shell_folder, false, 0)? {
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
            entries.push(ShellFolderEntry {
                display_name,
                path,
                category: NetworkItemCategory::Unknown,
                item_type_text: None,
            });
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
                category: NetworkItemCategory::Unknown,
                item_type_text: None,
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
            let text = if output.stdout.windows(2).any(|w| w == [0, 0])
                || output.stdout.iter().filter(|&&b| b == 0).count() > 2
            {
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
                    category: NetworkItemCategory::Unknown,
                    item_type_text: None,
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
                category: NetworkItemCategory::Unknown,
                item_type_text: None,
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
///
/// Runs on a dedicated STA thread (like Files `STATask`) because Shell COM
/// enumeration requires an apartment-threaded COM context.
#[cfg(windows)]
pub fn list_network_computers() -> Vec<ShellFolderEntry> {
    // Use synchronous enumeration — SHCONTF_ENABLE_ASYNC produced unreliable
    // partial results (sometimes 1-2 items, sometimes many). run_sta_task runs
    // on its own STA thread so the UI is never blocked.
    let entries = run_sta_task(|| unsafe { list_network_computers_inner(false) });
    tracing::info!(target: "network", count = entries.len(), "found network computers");
    entries
}

#[cfg(windows)]
unsafe fn list_network_computers_inner(enable_async: bool) -> Vec<ShellFolderEntry> {
    tracing::info!(target: "network", "list_network_computers_inner: parsing network folder GUID");
    let guid_wide: Vec<u16> = r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"
        .encode_utf16()
        .chain([0])
        .collect();
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(guid_wide.as_ptr()), None, &mut pidl, 0, None).is_err() {
        tracing::warn!(target: "network", "SHParseDisplayName failed for network folder");
        return Vec::new();
    }
    if pidl.is_null() {
        tracing::warn!(target: "network", "SHParseDisplayName returned null PIDL");
        return Vec::new();
    }
    tracing::info!(target: "network", "SHParseDisplayName succeeded");

    let desktop: IShellFolder = match SHGetDesktopFolder() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: "network", error = ?e, "SHGetDesktopFolder failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    let shell_folder: IShellFolder = match desktop.BindToObject(pidl, None) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(target: "network", error = ?e, "BindToObject failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    tracing::info!(target: "network", "BindToObject succeeded");

    let pidls = match enum_folder_pidls(&shell_folder, enable_async, SHCONTF_NONFOLDERS.0 as u32) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(target: "network", error = ?e, "EnumObjects failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    tracing::info!(target: "network", count = pidls.len(), "EnumObjects returned PIDLs");

    let mut entries = Vec::new();
    for child_pidl in pidls {
        let display_name =
            display_name_of(&shell_folder, child_pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing =
            display_name_of(&shell_folder, child_pidl, SHGDN_FORPARSING).unwrap_or_default();
        let attrs = item_attributes(&shell_folder, child_pidl);
        let folder = attrs & SFGAO_FOLDER != 0;
        let filesystem = attrs & SFGAO_FILESYSTEM != 0;
        let item_type_text = item_type_text_of(&shell_folder, pidl, child_pidl);
        let category =
            detect_network_category(attrs, &parsing, &display_name, item_type_text.as_deref());
        tracing::info!(target: "network", display_name, parsing, folder, filesystem, item_type_text, ?category, "network folder child");
        if parsing.is_empty() {
            ILFree(Some(child_pidl));
            continue;
        }
        entries.push(ShellFolderEntry {
            display_name,
            path: PathBuf::from(&parsing),
            category,
            item_type_text,
        });
        ILFree(Some(child_pidl));
    }

    ILFree(Some(pidl));
    entries
}

/// Enumerate shares inside a network computer (e.g. \\COMPUTERNAME).
///
/// Runs on a dedicated STA thread so that Shell COM has an initialized apartment.
#[cfg(windows)]
pub fn list_network_shares(path: &std::path::Path) -> Vec<ShellFolderEntry> {
    let path_owned = path.to_path_buf();
    let entries = run_sta_task(move || unsafe { list_network_shares_inner(&path_owned) });
    tracing::info!(target: "network", path = %path.display(), count = entries.len(), "found network shares");
    entries
}

#[cfg(windows)]
unsafe fn list_network_shares_inner(path: &std::path::Path) -> Vec<ShellFolderEntry> {
    tracing::info!(target: "network", path = %path.display(), "list_network_shares_inner: parsing computer path");
    let path_wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(path_wide.as_ptr()), None, &mut pidl, 0, None).is_err() {
        tracing::warn!(target: "network", path = %path.display(), "SHParseDisplayName failed for computer");
        return Vec::new();
    }
    if pidl.is_null() {
        tracing::warn!(target: "network", path = %path.display(), "SHParseDisplayName returned null PIDL");
        return Vec::new();
    }
    tracing::info!(target: "network", path = %path.display(), "SHParseDisplayName succeeded");

    let desktop: IShellFolder = match SHGetDesktopFolder() {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!(target: "network", path = %path.display(), error = ?e, "SHGetDesktopFolder failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    let shell_folder: IShellFolder = match desktop.BindToObject(pidl, None) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(target: "network", path = %path.display(), error = ?e, "BindToObject failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    tracing::info!(target: "network", path = %path.display(), "BindToObject succeeded");

    let pidls = match enum_folder_pidls(&shell_folder, false, 0) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(target: "network", path = %path.display(), error = ?e, "EnumObjects failed");
            ILFree(Some(pidl));
            return Vec::new();
        }
    };
    tracing::info!(target: "network", path = %path.display(), count = pidls.len(), "EnumObjects returned PIDLs");

    let mut entries = Vec::new();
    for child_pidl in pidls {
        let display_name =
            display_name_of(&shell_folder, child_pidl, SHGDN_INFOLDER).unwrap_or_default();
        let parsing =
            display_name_of(&shell_folder, child_pidl, SHGDN_FORPARSING).unwrap_or_default();
        let attrs = item_attributes(&shell_folder, child_pidl);
        let folder = attrs & SFGAO_FOLDER != 0;
        let filesystem = attrs & SFGAO_FILESYSTEM != 0;
        let item_type_text = item_type_text_of(&shell_folder, pidl, child_pidl);
        let category =
            detect_network_category(attrs, &parsing, &display_name, item_type_text.as_deref());
        tracing::info!(target: "network", display_name, parsing, folder, filesystem, item_type_text, ?category, "network share child");
        if parsing.is_empty() {
            ILFree(Some(child_pidl));
            continue;
        }
        entries.push(ShellFolderEntry {
            display_name,
            path: std::path::PathBuf::from(&parsing),
            category,
            item_type_text,
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

/// Diagnostic test: enumerate the Shell Network folder and print raw classification data
/// for every item so we can see what the media player actually reports.
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use windows::Win32::UI::Shell::Common::SHELLDETAILS;

    #[test]
    fn print_network_devices() {
        ensure_com_apartment().unwrap();
        let entries = unsafe { list_network_computers_inner(false) };
        println!("=== Network Devices ({}) ===\n", entries.len());
        for (i, e) in entries.iter().enumerate() {
            println!("[{}]", i);
            println!("  display_name : {}", e.display_name);
            println!("  parsing_path : {}", e.path.display());
            println!("  item_type_text: {:?}", e.item_type_text);
            println!("  category     : {:?}", e.category);
            println!();
        }
    }

    #[test]
    fn print_network_devices_diag() {
        ensure_com_apartment().unwrap();

        let guid_wide: Vec<u16> = r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"
            .encode_utf16()
            .chain([0])
            .collect();
        let mut parent_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        unsafe {
            SHParseDisplayName(PCWSTR(guid_wide.as_ptr()), None, &mut parent_pidl, 0, None)
                .unwrap();
        }
        let desktop = unsafe { SHGetDesktopFolder().unwrap() };
        let shell_folder: IShellFolder =
            unsafe { desktop.BindToObject(parent_pidl, None).unwrap() };
        let folder2: IShellFolder2 = shell_folder
            .cast()
            .ok()
            .expect("IShellFolder2 not supported");

        let pidls = unsafe {
            enum_folder_pidls(&shell_folder, false, SHCONTF_NONFOLDERS.0 as u32).unwrap()
        };
        println!("=== Network Devices Diag ({}) ===\n", pidls.len());

        for (i, child_pidl) in pidls.iter().enumerate() {
            let display_name = unsafe {
                display_name_of(&shell_folder, *child_pidl, SHGDN_INFOLDER).unwrap_or_default()
            };
            let parsing = unsafe {
                display_name_of(&shell_folder, *child_pidl, SHGDN_FORPARSING).unwrap_or_default()
            };

            // Get Category column (col 1)
            let mut sd: SHELLDETAILS = unsafe { std::mem::zeroed() };
            let category_text = if unsafe { folder2.GetDetailsOf(*child_pidl, 1, &mut sd).is_ok() }
            {
                let mut strret = sd.str;
                let mut psz: windows::core::PWSTR = windows::core::PWSTR::null();
                unsafe {
                    let _ = StrRetToStrW(&mut strret, Some(*child_pidl), &mut psz);
                    let text = psz.to_string().ok().filter(|s| !s.is_empty());
                    windows::Win32::System::Com::CoTaskMemFree(Some(psz.0 as *mut _));
                    text
                }
            } else {
                None
            };

            println!("[{}] {}", i, display_name);
            println!("  parsing       : {}", parsing);
            println!("  category_col  : {:?}", category_text);
            println!();
        }

        for pidl in pidls {
            unsafe {
                ILFree(Some(pidl));
            }
        }
        unsafe {
            ILFree(Some(parent_pidl));
        }
    }
}
