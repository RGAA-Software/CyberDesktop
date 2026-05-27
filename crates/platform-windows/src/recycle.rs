use std::path::PathBuf;
use std::time::SystemTime;

use windows::Win32::Foundation::{HWND, S_OK};
use windows::Win32::UI::Shell::Common::{ITEMIDLIST, STRRET};
use windows::Win32::UI::Shell::{
    IEnumIDList, ILFree, IShellFolder, SHGetDesktopFolder, StrRetToStrW, SHCONTF_FOLDERS,
    SHCONTF_INCLUDEHIDDEN, SHCONTF_NONFOLDERS, SHGDNF, SHGDN_FORPARSING, SHGDN_INFOLDER,
};

use crate::com::ensure_com_apartment;
use crate::paths::recycle_bin_pidl;

/// `SFGAO_FOLDER` — child is a subfolder (e.g. per-drive bucket under recycle bin).
const SFGAO_FOLDER: u32 = 0x2000_0000;

/// One item in the virtual recycle bin (not a direct filesystem path).
#[derive(Debug, Clone)]
pub struct RecycleBinEntry {
    pub display_name: String,
    /// Parsing path for Shell verbs (properties, context menu).
    pub shell_path: PathBuf,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
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

unsafe fn enum_child_pidls(folder: &IShellFolder) -> anyhow::Result<Vec<*mut ITEMIDLIST>> {
    let mut enum_id: Option<IEnumIDList> = None;
    let flags = (SHCONTF_FOLDERS.0 | SHCONTF_NONFOLDERS.0 | SHCONTF_INCLUDEHIDDEN.0) as u32;
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

unsafe fn entry_from_pidl(folder: &IShellFolder, pidl: *const ITEMIDLIST) -> RecycleBinEntry {
    let display_name = display_name_of(folder, pidl, SHGDN_INFOLDER)
        .unwrap_or_else(|_| display_name_of(folder, pidl, SHGDN_FORPARSING).unwrap_or_default());
    let shell_path = display_name_of(folder, pidl, SHGDN_FORPARSING)
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(&display_name));

    RecycleBinEntry {
        display_name,
        shell_path,
        size: None,
        modified: None,
    }
}

unsafe fn collect_from_folder(
    folder: &IShellFolder,
    out: &mut Vec<RecycleBinEntry>,
) -> anyhow::Result<()> {
    let child_pidls = enum_child_pidls(folder)?;
    for pidl in child_pidls {
        if is_folder_item(folder, pidl) {
            let sub: IShellFolder = folder.BindToObject(pidl, None)?;
            collect_from_folder(&sub, out)?;
        } else {
            out.push(entry_from_pidl(folder, pidl));
        }
        ILFree(Some(pidl));
    }
    Ok(())
}

/// Enumerates deleted items via the Shell recycle-bin namespace (`Shell:RecycleBinFolder`).
pub fn list_recycle_bin_entries() -> anyhow::Result<Vec<RecycleBinEntry>> {
    ensure_com_apartment()?;
    unsafe { list_recycle_bin_entries_inner() }
}

unsafe fn list_recycle_bin_entries_inner() -> anyhow::Result<Vec<RecycleBinEntry>> {
    let desktop: IShellFolder = SHGetDesktopFolder()?;
    let recycle_pidl = recycle_bin_pidl()?;

    let recycle_folder: IShellFolder = desktop.BindToObject(recycle_pidl, None)?;

    let mut items = Vec::new();
    let result = collect_from_folder(&recycle_folder, &mut items);
    ILFree(Some(recycle_pidl));
    result?;
    Ok(items)
}
