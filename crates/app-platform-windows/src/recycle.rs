use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::core::Interface;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::EnhancedStorage::PKEY_Size;
use windows::Win32::System::Com::StructuredStorage::PropVariantToBSTR;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};
use windows::Win32::System::SystemServices::SFGAO_FOLDER;
use windows::Win32::UI::Shell::PropertiesSystem::PROPERTYKEY;
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_RecycleBinFolder, FileOperation, IEnumShellItems, IFileOperation,
    IShellItem, IShellItem2, SHCreateItemFromParsingName, SHEmptyRecycleBinW, SHGetKnownFolderItem,
    FOFX_EARLYFAILURE, FOF_NO_UI, KF_FLAG_DEFAULT, PID_DISPLACED_DATE, PID_DISPLACED_FROM,
    PSGUID_DISPLACED, SHERB_NOCONFIRMATION, SHERB_NOPROGRESSUI, SHERB_NOSOUND, SIGDN,
    SIGDN_DESKTOPABSOLUTEPARSING, SIGDN_PARENTRELATIVE,
};

use crate::com::ensure_com_apartment;

const SCID_ORIGINAL_LOCATION: PROPERTYKEY = PROPERTYKEY {
    fmtid: PSGUID_DISPLACED,
    pid: PID_DISPLACED_FROM,
};
const SCID_DATE_DELETED: PROPERTYKEY = PROPERTYKEY {
    fmtid: PSGUID_DISPLACED,
    pid: PID_DISPLACED_DATE,
};

/// One item in the virtual recycle bin (not a direct filesystem path).
#[derive(Debug, Clone)]
pub struct RecycleBinEntry {
    pub display_name: String,
    /// Parsing path for Shell verbs (properties, context menu, restore).
    pub shell_path: PathBuf,
    pub kind: RecycleBinItemKind,
    pub size: Option<u64>,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecycleBinItemKind {
    File,
    Folder,
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

unsafe fn shell_display_name(item: &IShellItem, sigdn: SIGDN) -> anyhow::Result<String> {
    let name = item.GetDisplayName(sigdn)?;
    let result = name.to_string()?;
    windows::Win32::System::Com::CoTaskMemFree(Some(name.0 as *mut _));
    Ok(result)
}

unsafe fn deleted_time(item: &IShellItem2) -> Option<SystemTime> {
    let time = item.GetFileTime(&SCID_DATE_DELETED).ok()?;
    let time_u64 = ((time.dwHighDateTime as u64) << 32) | (time.dwLowDateTime as u64);
    const EPOCH_AS_FILETIME: u64 = 116444736000000000;
    const HUNDREDS_OF_NANOSECONDS: u64 = 10_000_000;
    if time_u64 < EPOCH_AS_FILETIME {
        return None;
    }
    let seconds = (time_u64 - EPOCH_AS_FILETIME) / HUNDREDS_OF_NANOSECONDS;
    Some(UNIX_EPOCH + Duration::from_secs(seconds))
}

unsafe fn item_size(item: &IShellItem) -> Option<u64> {
    let item2: IShellItem2 = item.cast().ok()?;
    let is_dir = item2.GetAttributes(SFGAO_FOLDER).ok()? == SFGAO_FOLDER;
    if is_dir {
        return None;
    }
    item2.GetUInt64(&PKEY_Size).ok()
}

/// Enumerates deleted items via `IEnumShellItems` on the recycle-bin known folder.
pub fn list_recycle_bin_entries() -> anyhow::Result<Vec<RecycleBinEntry>> {
    ensure_com_apartment()?;
    unsafe { list_recycle_bin_entries_inner() }
}

unsafe fn list_recycle_bin_entries_inner() -> anyhow::Result<Vec<RecycleBinEntry>> {
    let recycle_bin: IShellItem = SHGetKnownFolderItem(
        &FOLDERID_RecycleBinFolder,
        KF_FLAG_DEFAULT,
        HANDLE::default(),
    )?;
    let pesi: IEnumShellItems = recycle_bin.BindToHandler(None, &BHID_EnumItems)?;

    let mut items = Vec::new();
    loop {
        let mut fetched = 0u32;
        let mut batch = [None];
        pesi.Next(&mut batch, Some(&mut fetched))?;
        if fetched == 0 {
            break;
        }
        let Some(item) = batch[0].clone() else {
            break;
        };

        let shell_path = PathBuf::from(shell_display_name(&item, SIGDN_DESKTOPABSOLUTEPARSING)?);
        let display_name = shell_display_name(&item, SIGDN_PARENTRELATIVE)?;
        let item2: IShellItem2 = item.cast()?;
        let is_dir = item2.GetAttributes(SFGAO_FOLDER)? == SFGAO_FOLDER;

        items.push(RecycleBinEntry {
            display_name,
            shell_path,
            kind: if is_dir {
                RecycleBinItemKind::Folder
            } else {
                RecycleBinItemKind::File
            },
            size: item_size(&item),
            modified: deleted_time(&item2),
        });
    }

    Ok(items)
}

/// Permanently removes all items from the Recycle Bin (no system confirmation UI).
pub fn empty_recycle_bin() -> anyhow::Result<()> {
    ensure_com_apartment()?;
    unsafe {
        SHEmptyRecycleBinW(
            None,
            None,
            SHERB_NOCONFIRMATION | SHERB_NOPROGRESSUI | SHERB_NOSOUND,
        )?;
    }
    Ok(())
}

unsafe fn original_path_for_item(item: &IShellItem2) -> anyhow::Result<PathBuf> {
    let original_location = PropVariantToBSTR(&item.GetProperty(&SCID_ORIGINAL_LOCATION)?)?;
    let parent = PathBuf::from(original_location.to_string());
    let shell: IShellItem = item.cast()?;
    let name = shell_display_name(&shell, SIGDN_PARENTRELATIVE)?;
    Ok(parent.join(name))
}

/// Resolve recycle-bin shell paths for files that were deleted from `original_paths`.
pub fn recycle_shell_paths_for_originals(
    original_paths: &[PathBuf],
) -> anyhow::Result<Vec<PathBuf>> {
    if original_paths.is_empty() {
        return Ok(Vec::new());
    }
    ensure_com_apartment()?;
    unsafe { recycle_shell_paths_for_originals_inner(original_paths) }
}

unsafe fn recycle_shell_paths_for_originals_inner(
    original_paths: &[PathBuf],
) -> anyhow::Result<Vec<PathBuf>> {
    let recycle_bin: IShellItem = SHGetKnownFolderItem(
        &FOLDERID_RecycleBinFolder,
        KF_FLAG_DEFAULT,
        HANDLE::default(),
    )?;
    let pesi: IEnumShellItems = recycle_bin.BindToHandler(None, &BHID_EnumItems)?;

    let mut requested: std::collections::HashSet<PathBuf> = original_paths
        .iter()
        .map(|path| normalize_path_for_match(path))
        .collect();
    let mut shell_paths = Vec::new();

    loop {
        let mut fetched = 0u32;
        let mut batch = [None];
        pesi.Next(&mut batch, Some(&mut fetched))?;
        if fetched == 0 {
            break;
        }
        let Some(item) = batch[0].clone() else {
            break;
        };

        let item2: IShellItem2 = item.cast()?;
        let original = normalize_path_for_match(&original_path_for_item(&item2)?);
        if requested.remove(&original) {
            shell_paths.push(PathBuf::from(shell_display_name(
                &item,
                SIGDN_DESKTOPABSOLUTEPARSING,
            )?));
        }
        if requested.is_empty() {
            break;
        }
    }

    Ok(shell_paths)
}

fn normalize_path_for_match(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Restore recycle-bin items via `IFileOperation::MoveItem` back to their original folders.
pub fn restore_recycle_bin_items(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }
    ensure_com_apartment()?;
    unsafe { restore_recycle_bin_items_inner(paths) }
}

unsafe fn restore_recycle_bin_items_inner(paths: &[PathBuf]) -> anyhow::Result<()> {
    let pfo: IFileOperation = CoCreateInstance(&FileOperation, None, CLSCTX_ALL)?;
    pfo.SetOperationFlags(FOF_NO_UI | FOFX_EARLYFAILURE)?;

    let mut restored = 0usize;
    let mut errors = Vec::new();

    for path in paths {
        match restore_one_item(&pfo, path) {
            Ok(()) => restored += 1,
            Err(error) => errors.push(format!("{}: {error:#}", path.display())),
        }
    }

    if restored > 0 {
        pfo.PerformOperations()?;
    }

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.join("\n"))
    }
}

unsafe fn restore_one_item(pfo: &IFileOperation, path: &Path) -> anyhow::Result<()> {
    let wide = path_to_wide(path);
    let trash_item: IShellItem =
        SHCreateItemFromParsingName(windows::core::PCWSTR(wide.as_ptr()), None)?;
    let item2: IShellItem2 = trash_item.cast()?;

    let original_location = PropVariantToBSTR(&item2.GetProperty(&SCID_ORIGINAL_LOCATION)?)?;
    let original_parent = PathBuf::from(original_location.to_string());
    let name = shell_display_name(&trash_item, SIGDN_PARENTRELATIVE)?;
    let parent_wide = path_to_wide(&original_parent);
    let orig_folder: IShellItem =
        SHCreateItemFromParsingName(windows::core::PCWSTR(parent_wide.as_ptr()), None)?;
    let name_wide = path_to_wide(Path::new(&name));

    pfo.MoveItem(
        &trash_item,
        &orig_folder,
        windows::core::PCWSTR(name_wide.as_ptr()),
        None,
    )?;
    Ok(())
}
