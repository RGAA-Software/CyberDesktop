//! Windows Search index queries (AQS) via Shell COM.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use windows::Win32::Globalization::GetUserDefaultUILanguage;
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Search::{
    ICondition, IQueryParser, IQueryParserManager, QueryParserManager,
};
use windows::Win32::UI::Shell::{
    IEnumShellItems, ISearchFolderItemFactory, IShellItem, IShellItemArray,
    SearchFolderItemFactory, SHCreateItemFromParsingName, SHCreateShellItemArrayFromShellItem,
    BHID_EnumItems, SIGDN_DESKTOPABSOLUTEPARSING,
};

use crate::com::ensure_com_apartment;

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn string_to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn path_in_scope(path: &Path, scope: &Path) -> bool {
    let path_key = path.to_string_lossy().to_ascii_lowercase();
    let scope_key = scope.to_string_lossy().to_ascii_lowercase();
    path_key.starts_with(&scope_key)
}

unsafe fn shell_filesystem_path(item: &IShellItem) -> anyhow::Result<PathBuf> {
    let name = item.GetDisplayName(SIGDN_DESKTOPABSOLUTEPARSING)?;
    let result = name.to_string()?;
    windows::Win32::System::Com::CoTaskMemFree(Some(name.0 as *mut _));
    Ok(PathBuf::from(result))
}

/// Query the Windows Search index with AQS, scoped to `scope_root`.
pub fn search_indexed_aqs(
    scope_root: &Path,
    aqs_query: &str,
    cancel: &AtomicBool,
    max_results: usize,
) -> anyhow::Result<Vec<PathBuf>> {
    if aqs_query.trim().is_empty() {
        return Ok(Vec::new());
    }
    if !scope_root.is_dir() {
        anyhow::bail!("search scope is not a directory");
    }
    ensure_com_apartment()?;
    search_indexed_aqs_inner(scope_root, aqs_query, cancel, max_results)
}

fn search_indexed_aqs_inner(
    scope_root: &Path,
    aqs_query: &str,
    cancel: &AtomicBool,
    max_results: usize,
) -> anyhow::Result<Vec<PathBuf>> {
    unsafe { search_indexed_aqs_com(scope_root, aqs_query, cancel, max_results) }
}

unsafe fn search_indexed_aqs_com(
    scope_root: &Path,
    aqs_query: &str,
    cancel: &AtomicBool,
    max_results: usize,
) -> anyhow::Result<Vec<PathBuf>> {
    let parser_manager: IQueryParserManager =
        CoCreateInstance(&QueryParserManager, None, CLSCTX_INPROC_SERVER)?;
    let lang = GetUserDefaultUILanguage();
    let parser: IQueryParser = parser_manager.CreateLoadedParser(
        windows::core::w!("SystemIndex"),
        lang,
    )?;

    let query_wide = string_to_wide(aqs_query);
    let solution = parser.Parse(
        windows::core::PCWSTR(query_wide.as_ptr()),
        None::<&windows::Win32::System::Com::IEnumUnknown>,
    )?;

    let mut condition: Option<ICondition> = None;
    solution.GetQuery(Some(&mut condition), None)?;
    let condition = condition.ok_or_else(|| anyhow::anyhow!("AQS query produced no condition"))?;

    let factory: ISearchFolderItemFactory =
        CoCreateInstance(&SearchFolderItemFactory, None, CLSCTX_INPROC_SERVER)?;

    let scope_wide = path_to_wide(scope_root);
    let scope_item: IShellItem =
        SHCreateItemFromParsingName(windows::core::PCWSTR(scope_wide.as_ptr()), None)?;
    let scope_array: IShellItemArray =
        SHCreateShellItemArrayFromShellItem(&scope_item)?;

    factory.SetScope(&scope_array)?;
    factory.SetCondition(&condition)?;
    let title_wide = string_to_wide(aqs_query);
    factory.SetDisplayName(windows::core::PCWSTR(title_wide.as_ptr()))?;

    let search_folder: IShellItem = factory.GetShellItem()?;
    let enumerator: IEnumShellItems = search_folder.BindToHandler(None, &BHID_EnumItems)?;

    let mut paths = Vec::new();
    loop {
        if cancel.load(Ordering::Relaxed) || paths.len() >= max_results {
            break;
        }
        let mut fetched = 0u32;
        let mut batch = [None];
        enumerator.Next(&mut batch, Some(&mut fetched))?;
        if fetched == 0 {
            break;
        }
        let Some(item) = batch[0].clone() else {
            break;
        };
        let Ok(path) = shell_filesystem_path(&item) else {
            continue;
        };
        if path_in_scope(&path, scope_root) {
            paths.push(path);
        }
    }

    Ok(paths)
}
