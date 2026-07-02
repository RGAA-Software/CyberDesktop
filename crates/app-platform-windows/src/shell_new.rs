//! Windows Explorer «New» submenu: enumerate `ShellNew` registry entries and create items.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};

use crate::shell_icon::{
    menu_icon_pixel_size, shell_icon_png_for_list_key, shell_icon_png_from_location,
};

use windows::core::PCWSTR;
use windows::Win32::System::Environment::ExpandEnvironmentStringsW;
use windows::Win32::UI::Shell::SHLoadIndirectString;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CLASSES_ROOT, HKEY_CURRENT_USER,
    KEY_READ, REG_BINARY, REG_MULTI_SZ, REG_SZ, REG_VALUE_TYPE,
};

const EXPLORER_SHELL_NEW_CLASSES: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Explorer\Discardable\PostSetup\ShellNew";
const CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Clone, PartialEq)]
pub struct ShellNewMenuItem {
    pub label: String,
    pub registry_key: String,
    pub icon_png: Option<Arc<Vec<u8>>>,
    kind: ShellNewKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ShellNewKind {
    Directory,
    NullFile {
        extension: String,
    },
    FileFromTemplate {
        template_path: PathBuf,
        extension: String,
    },
    BinaryData {
        extension: String,
        data: Vec<u8>,
    },
    Command {
        command: String,
        extension: Option<String>,
    },
}

struct CachedMenu {
    items: Vec<ShellNewMenuItem>,
    fetched_at: std::time::Instant,
}

static MENU_CACHE: OnceLock<RwLock<Option<CachedMenu>>> = OnceLock::new();
static WARM_RUNNING: AtomicBool = AtomicBool::new(false);

fn menu_cache() -> &'static RwLock<Option<CachedMenu>> {
    MENU_CACHE.get_or_init(|| RwLock::new(None))
}

/// Non-blocking read of the warmed menu list.
pub fn peek_shell_new_menu_items() -> Option<Vec<ShellNewMenuItem>> {
    menu_cache()
        .read()
        .ok()
        .and_then(|guard| guard.as_ref().map(|cache| cache.items.clone()))
}

/// Populate the cache on a background thread (safe to call from the UI thread).
pub fn warm_shell_new_menu_cache() {
    if peek_shell_new_menu_items().is_some() {
        return;
    }
    if WARM_RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    std::thread::spawn(|| {
        refresh_shell_new_menu_cache();
        WARM_RUNNING.store(false, Ordering::Release);
    });
}

/// Refreshes the cache; intended for tests and background warm-up only.
pub fn refresh_shell_new_menu_cache() {
    let icon_px = menu_icon_pixel_size(crate::shell_icon::system_scale_factor());
    let items = enumerate_shell_new_menu_items(icon_px);
    if let Ok(mut guard) = menu_cache().write() {
        *guard = Some(CachedMenu {
            items,
            fetched_at: std::time::Instant::now(),
        });
    }
}

/// Returns Explorer-style «New» menu rows (cached for a few minutes).
pub fn query_shell_new_menu_items() -> Vec<ShellNewMenuItem> {
    if let Some(items) = peek_shell_new_menu_items() {
        if let Ok(guard) = menu_cache().read() {
            if guard
                .as_ref()
                .is_some_and(|cache| cache.fetched_at.elapsed() < CACHE_TTL)
            {
                return items;
            }
        }
    }

    refresh_shell_new_menu_cache();
    peek_shell_new_menu_items().unwrap_or_default()
}

pub fn clear_shell_new_menu_cache() {
    if let Ok(mut guard) = menu_cache().write() {
        *guard = None;
    }
}

/// True for Explorer folder entries (`Folder` / `Directory` ShellNew).
pub fn shell_new_item_is_folder(item: &ShellNewMenuItem) -> bool {
    item.registry_key.eq_ignore_ascii_case("Folder")
        || item.registry_key.eq_ignore_ascii_case("Directory")
}

/// Creates a new file or folder in `parent` using the selected ShellNew definition.
///
/// Returns `None` when a `Command` handler launches an app without creating a
/// concrete path synchronously (e.g. WPS «New PDF»).
pub fn create_shell_new_item(
    parent: &Path,
    item: &ShellNewMenuItem,
) -> anyhow::Result<Option<PathBuf>> {
    match &item.kind {
        ShellNewKind::Directory => {
            let name = unique_shell_new_name(parent, &item.label, None);
            create_directory(parent, &name).map(Some)
        }
        ShellNewKind::NullFile { extension } => {
            let ext = effective_shell_new_extension(&item.registry_key, Some(extension));
            let name = unique_shell_new_name(parent, &item.label, ext.as_deref());
            create_file_with_contents(parent, &name, &[]).map(Some)
        }
        ShellNewKind::FileFromTemplate {
            template_path,
            extension,
        } => {
            let ext = effective_shell_new_extension(&item.registry_key, Some(extension));
            let name = unique_shell_new_name(parent, &item.label, ext.as_deref());
            let bytes = std::fs::read(expand_env_path(template_path))?;
            create_file_with_contents(parent, &name, &bytes).map(Some)
        }
        ShellNewKind::BinaryData { extension, data } => {
            let ext = effective_shell_new_extension(&item.registry_key, Some(extension));
            let name = unique_shell_new_name(parent, &item.label, ext.as_deref());
            create_file_with_contents(parent, &name, data).map(Some)
        }
        ShellNewKind::Command { command, extension } => {
            if shell_new_command_expects_path(command) {
                let ext = effective_shell_new_extension(&item.registry_key, extension.as_deref());
                let name = unique_shell_new_name(parent, &item.label, ext.as_deref());
                let path = parent.join(&name);
                invoke_shell_new_command(parent, command, Some(&path))?;
                Ok(Some(path))
            } else {
                invoke_shell_new_command(parent, command, None)?;
                Ok(None)
            }
        }
    }
}

fn enumerate_shell_new_menu_items(icon_px: u32) -> Vec<ShellNewMenuItem> {
    let mut items = Vec::new();
    let mut seen = HashSet::new();

    for class_name in ["Folder", "Directory"] {
        if let Some(item) = resolve_shell_new_entry(class_name, icon_px) {
            let key = item_dedup_key(&item);
            if seen.insert(key) {
                items.push(item);
            }
        }
    }

    let class_names = read_reg_multi_sz(HKEY_CURRENT_USER, EXPLORER_SHELL_NEW_CLASSES, "Classes");

    for class_name in class_names {
        if class_name.eq_ignore_ascii_case("Folder") || class_name.eq_ignore_ascii_case("Directory")
        {
            continue;
        }
        let Some(item) = resolve_shell_new_entry(&class_name, icon_px) else {
            continue;
        };
        let key = item_dedup_key(&item);
        if seen.insert(key) {
            items.push(item);
        }
    }

    items
}

fn item_dedup_key(item: &ShellNewMenuItem) -> String {
    format!(
        "{}|{}",
        item.label.to_ascii_lowercase(),
        item.registry_key.to_ascii_lowercase()
    )
}

fn resolve_shell_new_entry(class_name: &str, icon_px: u32) -> Option<ShellNewMenuItem> {
    let class_name = class_name.trim();
    if class_name.is_empty() {
        return None;
    }

    let shell_new_subkey = resolve_shell_new_subkey(class_name)?;
    if reg_value_exists(&shell_new_subkey, "DisableProcessing") {
        return None;
    }
    if shell_new_is_opt_in(&shell_new_subkey) {
        return None;
    }

    let label = read_shell_new_label(class_name, &shell_new_subkey)?;
    let extension = resolve_extension_for_class(class_name);
    let kind = read_shell_new_kind(&shell_new_subkey, extension)?;
    let icon_png = resolve_shell_new_icon(class_name, &shell_new_subkey, &kind, icon_px);

    Some(ShellNewMenuItem {
        label,
        registry_key: class_name.to_string(),
        icon_png,
        kind,
    })
}

fn resolve_shell_new_icon(
    class_name: &str,
    shell_new_subkey: &str,
    kind: &ShellNewKind,
    icon_px: u32,
) -> Option<Arc<Vec<u8>>> {
    if let Some(location) = read_shell_new_icon_location(class_name, shell_new_subkey) {
        if let Ok(png) = shell_icon_png_from_location(&location, icon_px) {
            if !png.is_empty() {
                return Some(Arc::new(png));
            }
        }
    }

    if class_name.starts_with('.') {
        if let Ok(png) = shell_icon_png_for_list_key(class_name, icon_px) {
            if !png.is_empty() {
                return Some(Arc::new(png));
            }
        }
    }

    if matches!(kind, ShellNewKind::Directory) {
        if let Ok(png) = shell_icon_png_for_list_key(":folder:", icon_px) {
            if !png.is_empty() {
                return Some(Arc::new(png));
            }
        }
    }

    None
}

fn read_shell_new_icon_location(class_name: &str, shell_new_subkey: &str) -> Option<String> {
    if let Some(location) = read_reg_expand_string(HKEY_CLASSES_ROOT, shell_new_subkey, "IconPath") {
        if !location.is_empty() {
            return Some(location);
        }
    }

    if let Some(progid) = progid_for_shell_new(class_name, shell_new_subkey) {
        if let Some(location) =
            read_reg_expand_string(HKEY_CLASSES_ROOT, &format!("{progid}\\DefaultIcon"), "")
        {
            if !location.is_empty() {
                return Some(location);
            }
        }
    }

    None
}

fn read_reg_expand_string(root: HKEY, subkey: &str, value_name: &str) -> Option<String> {
    read_reg_string(root, subkey, value_name).map(|value| expand_env_string(&value))
}

fn resolve_shell_new_subkey(class_name: &str) -> Option<String> {
    let direct = format!("{class_name}\\ShellNew");
    if reg_key_exists(&direct) {
        return Some(direct);
    }

    if class_name.starts_with('.') {
        let progid = read_reg_string(HKEY_CLASSES_ROOT, class_name, "")?;
        if !progid.is_empty() {
            let nested = format!("{class_name}\\{progid}\\ShellNew");
            if reg_key_exists(&nested) {
                return Some(nested);
            }
            let progid_shell_new = format!("{progid}\\ShellNew");
            if reg_key_exists(&progid_shell_new) {
                return Some(progid_shell_new);
            }
        }
    }

    None
}

fn shell_new_is_opt_in(shell_new_subkey: &str) -> bool {
    reg_value_exists(&format!("{shell_new_subkey}\\Config"), "IsOptIn")
}

fn read_shell_new_label(class_name: &str, shell_new_subkey: &str) -> Option<String> {
    for value_name in ["ItemName", "MenuText", "FriendlyTypeName"] {
        if let Some(name) = read_reg_string(HKEY_CLASSES_ROOT, shell_new_subkey, value_name) {
            if let Some(resolved) = resolve_display_string(&name) {
                return Some(resolved);
            }
        }
    }

    if let Some(progid) = progid_for_shell_new(class_name, shell_new_subkey) {
        if let Some(label) = read_class_display_name(&progid) {
            return Some(label);
        }
    }

    if class_name.eq_ignore_ascii_case("Directory") || class_name.eq_ignore_ascii_case("Folder") {
        return read_class_display_name("Folder").or(Some("Folder".to_string()));
    }

    if !class_name.starts_with('.') {
        return read_class_display_name(class_name);
    }

    None
}

fn progid_for_shell_new(class_name: &str, shell_new_subkey: &str) -> Option<String> {
    if let Some(rest) = shell_new_subkey.strip_prefix(&format!("{class_name}\\")) {
        if let Some(progid) = rest.strip_suffix("\\ShellNew") {
            if !progid.is_empty() {
                return Some(progid.to_string());
            }
        }
    }
    if !class_name.starts_with('.') {
        return Some(class_name.to_string());
    }
    read_reg_string(HKEY_CLASSES_ROOT, class_name, "")
}

fn read_class_display_name(class_name: &str) -> Option<String> {
    for value_name in ["FriendlyTypeName", ""] {
        if let Some(name) = read_reg_string(HKEY_CLASSES_ROOT, class_name, value_name) {
            if let Some(resolved) = resolve_display_string(&name) {
                return Some(resolved);
            }
        }
    }
    None
}

fn resolve_display_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_indirect_string(trimmed) {
        if let Some(resolved) = sh_load_indirect_string(trimmed) {
            return Some(resolved);
        }
    }
    Some(trimmed.to_string())
}

fn is_indirect_string(value: &str) -> bool {
    value.starts_with('@') || value.to_ascii_lowercase().contains(".dll,")
}

fn sh_load_indirect_string(source: &str) -> Option<String> {
    let source = source.trim();
    let indirect = if source.starts_with('@') {
        source.to_string()
    } else {
        format!("@{source}")
    };
    let wide = to_wide(&indirect);
    let mut buffer = vec![0u16; 512];
    unsafe {
        SHLoadIndirectString(PCWSTR(wide.as_ptr()), &mut buffer, None).ok()?;
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    let resolved = String::from_utf16_lossy(&buffer[..end]).trim().to_string();
    if resolved.is_empty() {
        None
    } else {
        Some(resolved)
    }
}

fn read_shell_new_kind(shell_new_subkey: &str, extension: Option<String>) -> Option<ShellNewKind> {
    if reg_value_exists(shell_new_subkey, "Directory") {
        return Some(ShellNewKind::Directory);
    }
    if let Some(template) = read_reg_string(HKEY_CLASSES_ROOT, shell_new_subkey, "FileName") {
        return Some(ShellNewKind::FileFromTemplate {
            template_path: expand_env_path(Path::new(template.trim())),
            extension: extension.unwrap_or_default(),
        });
    }
    if reg_value_exists(shell_new_subkey, "NullFile") {
        return Some(ShellNewKind::NullFile {
            extension: extension.unwrap_or_default(),
        });
    }
    if let Some(data) = read_reg_binary(HKEY_CLASSES_ROOT, shell_new_subkey, "Data") {
        return Some(ShellNewKind::BinaryData {
            extension: extension.unwrap_or_default(),
            data,
        });
    }
    if let Some(command) = read_reg_string(HKEY_CLASSES_ROOT, shell_new_subkey, "Command") {
        return Some(ShellNewKind::Command { command, extension });
    }
    None
}

fn resolve_extension_for_class(class_name: &str) -> Option<String> {
    if class_name.starts_with('.') {
        return Some(class_name.to_string());
    }
    None
}

fn shell_new_command_expects_path(command: &str) -> bool {
    command.contains("%1")
}

/// Maps fake ShellNew class extensions (`.pdfwpsshellnew`) to a real file suffix.
fn effective_shell_new_extension(
    class_name: &str,
    extension: Option<&str>,
) -> Option<String> {
    let ext = extension?.trim();
    if ext.is_empty() {
        return None;
    }
    if is_shell_new_pseudo_extension(ext) {
        if let Some(progid) = read_reg_string(HKEY_CLASSES_ROOT, class_name, "") {
            return extension_from_progid(&progid);
        }
        return None;
    }
    Some(ext.to_string())
}

fn is_shell_new_pseudo_extension(ext: &str) -> bool {
    let lower = ext.to_ascii_lowercase();
    lower.contains("shellnew") || !lower.starts_with('.') || lower.len() > 8
}

fn extension_from_progid(progid: &str) -> Option<String> {
    let lower = progid.to_ascii_lowercase();
    for suffix in [".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".rtf", ".txt"] {
        if lower.contains(suffix.trim_start_matches('.')) {
            return Some(suffix.to_string());
        }
    }
    None
}

fn unique_shell_new_name(parent: &Path, label: &str, extension: Option<&str>) -> String {
    let sanitized_label = label.trim();
    let ext = extension
        .filter(|ext| !ext.is_empty())
        .map(|ext| {
            if ext.starts_with('.') {
                ext.to_string()
            } else {
                format!(".{ext}")
            }
        })
        .unwrap_or_default();

    let mut base = if sanitized_label.is_empty() {
        format!("New{ext}")
    } else if ext.is_empty() {
        sanitized_label.to_string()
    } else if sanitized_label.ends_with(&ext) {
        sanitized_label.to_string()
    } else {
        format!("{sanitized_label}{ext}")
    };

    if !base.to_ascii_lowercase().starts_with("new ") && !ext.is_empty() {
        base = format!("New {base}");
    }

    let mut candidate = base.clone();
    let mut counter = 2;
    while parent.join(&candidate).exists() {
        if ext.is_empty() {
            candidate = format!("{base} ({counter})");
        } else {
            let stem = base.strip_suffix(&ext).unwrap_or(&base);
            candidate = format!("{stem} ({counter}){ext}");
        }
        counter += 1;
    }
    candidate
}

fn create_directory(parent: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let path = parent.join(name);
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    std::fs::create_dir(&path)?;
    Ok(path)
}

fn create_file_with_contents(
    parent: &Path,
    name: &str,
    contents: &[u8],
) -> anyhow::Result<PathBuf> {
    let path = parent.join(name);
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    if let Some(parent_dir) = path.parent() {
        std::fs::create_dir_all(parent_dir)?;
    }
    std::fs::write(&path, contents)?;
    Ok(path)
}

fn invoke_shell_new_command(
    parent: &Path,
    command: &str,
    output_path: Option<&Path>,
) -> anyhow::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_FLAG_NO_UI, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let expanded = expand_env_string(command);
    let (application, arguments) = if let Some(path) = output_path.filter(|_| expanded.contains("%1"))
    {
        split_command_line(&expanded.replace("%1", &path.to_string_lossy()))
    } else {
        split_command_line(&expanded)
    };
    let application_wide = to_wide(&application);
    let arguments_wide = arguments.as_ref().map(|args| to_wide(args));
    let directory_wide = to_wide(&parent.to_string_lossy());

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_FLAG_NO_UI,
        lpFile: PCWSTR(application_wide.as_ptr()),
        lpParameters: arguments_wide
            .as_ref()
            .map(|args| PCWSTR(args.as_ptr()))
            .unwrap_or(PCWSTR::null()),
        lpDirectory: PCWSTR(directory_wide.as_ptr()),
        nShow: SW_SHOW.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}

fn split_command_line(command: &str) -> (String, Option<String>) {
    let trimmed = command.trim();
    if let Some(rest) = trimmed.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let app = rest[..end].to_string();
            let args = rest[end + 1..].trim();
            return (app, (!args.is_empty()).then(|| args.to_string()));
        }
    }
    let mut parts = trimmed.splitn(2, ' ');
    let app = parts.next().unwrap_or_default().to_string();
    let args = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    (app, args)
}

fn expand_env_path(path: &Path) -> PathBuf {
    PathBuf::from(expand_env_string(&path.to_string_lossy()))
}

fn expand_env_string(value: &str) -> String {
    let wide = to_wide(value);
    let mut buffer = vec![0u16; 1024];
    let needed = unsafe { ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };
    if needed == 0 {
        return value.to_string();
    }
    if needed as usize > buffer.len() {
        buffer.resize(needed as usize, 0);
        let needed = unsafe { ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut buffer)) };
        if needed == 0 {
            return value.to_string();
        }
    }
    let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

fn reg_key_exists(subkey: &str) -> bool {
    open_hkey(HKEY_CLASSES_ROOT, subkey).is_ok()
}

fn reg_value_exists(subkey: &str, value_name: &str) -> bool {
    read_reg_value(HKEY_CLASSES_ROOT, subkey, value_name).is_some()
}

fn read_reg_string(root: HKEY, subkey: &str, value_name: &str) -> Option<String> {
    let bytes = read_reg_value(root, subkey, value_name)?;
    let utf16: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    let end = utf16.iter().position(|&c| c == 0).unwrap_or(utf16.len());
    let value = String::from_utf16_lossy(&utf16[..end]);
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn read_reg_binary(root: HKEY, subkey: &str, value_name: &str) -> Option<Vec<u8>> {
    let bytes = read_reg_value(root, subkey, value_name)?;
    Some(bytes)
}

fn read_reg_multi_sz(root: HKEY, subkey: &str, value_name: &str) -> Vec<String> {
    let Some(bytes) = read_reg_value(root, subkey, value_name) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    let mut start = 0usize;
    while start + 1 < bytes.len() {
        let end = bytes[start..]
            .chunks_exact(2)
            .position(|chunk| chunk == [0, 0])
            .map(|pos| start + pos * 2)
            .unwrap_or(bytes.len());
        if end == start {
            break;
        }
        let utf16: Vec<u16> = bytes[start..end]
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let value = String::from_utf16_lossy(&utf16);
        if !value.is_empty() {
            items.push(value);
        }
        start = end + 2;
    }
    items
}

fn read_reg_value(root: HKEY, subkey: &str, value_name: &str) -> Option<Vec<u8>> {
    unsafe {
        let subkey_wide = to_wide(subkey);
        let mut hkey = Default::default();
        if RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut hkey).is_err() {
            return None;
        }

        let value_wide = to_wide(value_name);
        let value_ptr = if value_name.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(value_wide.as_ptr())
        };

        let mut kind = REG_VALUE_TYPE::default();
        let mut len = 0u32;
        if RegQueryValueExW(hkey, value_ptr, None, Some(&mut kind), None, Some(&mut len)).is_err() {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let mut buf = vec![0u8; len as usize];
        if RegQueryValueExW(
            hkey,
            value_ptr,
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr()),
            Some(&mut len),
        )
        .is_err()
        {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let _ = RegCloseKey(hkey);
        buf.truncate(len as usize);
        if matches!(kind, REG_SZ | REG_MULTI_SZ) && buf.len() >= 2 {
            Some(buf)
        } else if kind == REG_BINARY {
            Some(buf)
        } else if buf.len() >= 2 {
            Some(buf)
        } else {
            None
        }
    }
}

fn open_hkey(root: HKEY, subkey: &str) -> anyhow::Result<HKEY> {
    unsafe {
        let subkey_wide = to_wide(subkey);
        let mut hkey = Default::default();
        RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut hkey).ok()?;
        Ok(hkey)
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_new_menu_lists_common_items() {
        refresh_shell_new_menu_cache();
        let items = peek_shell_new_menu_items().expect("cache");
        assert!(
            items.iter().any(|item| {
                item.kind == ShellNewKind::Directory
                    || item.registry_key.eq_ignore_ascii_case("Folder")
            }),
            "expected folder entry, got: {items:?}"
        );
        assert!(
            items
                .iter()
                .any(|item| matches!(item.kind, ShellNewKind::NullFile { .. })),
            "expected at least one file template, got: {items:?}"
        );
        for item in &items {
            assert!(
                !item.label.contains("shell32.dll"),
                "label should be resolved, got {:?}",
                item
            );
            assert!(
                !item.label.eq_ignore_ascii_case("LibraryFolder"),
                "opt-in library entries should be filtered, got {:?}",
                item
            );
            assert!(
                item.icon_png.as_ref().is_some_and(|png| !png.is_empty()),
                "expected shell icon for {:?}",
                item.registry_key
            );
        }
        let folder = items
            .iter()
            .find(|item| item.registry_key.eq_ignore_ascii_case("Folder"))
            .expect("folder item");
        assert!(
            !folder.label.contains(".dll"),
            "folder label should be resolved, got {:?}",
            folder
        );
    }

    #[test]
    fn shell_new_menu_matches_explorer_class_list() {
        refresh_shell_new_menu_cache();
        let items = peek_shell_new_menu_items().expect("cache");
        assert!(
            items.len() >= 10,
            "expected Explorer-style class list, got only {} items: {:?}",
            items.len(),
            items
                .iter()
                .map(|item| (&item.registry_key, &item.label))
                .collect::<Vec<_>>()
        );
        assert!(
            items
                .iter()
                .any(|item| item.registry_key.eq_ignore_ascii_case(".doc")),
            "missing .doc entry: {items:?}"
        );
        assert!(
            items
                .iter()
                .any(|item| item.registry_key.eq_ignore_ascii_case(".txt")
                    || item.registry_key.eq_ignore_ascii_case(".rtf")),
            "missing text/rtf entry: {items:?}"
        );
    }

    #[test]
    fn shell_new_create_folder_and_text_file() {
        let parent =
            std::env::temp_dir().join(format!("cyber_files_shell_new_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();

        refresh_shell_new_menu_cache();
        let items = peek_shell_new_menu_items().expect("cache");
        let folder = items
            .iter()
            .find(|item| matches!(item.kind, ShellNewKind::Directory))
            .expect("folder item");
        let folder_path = create_shell_new_item(&parent, folder)
            .expect("create folder")
            .expect("folder path");
        assert!(folder_path.is_dir());

        let file_item = items
            .iter()
            .find(|item| matches!(item.kind, ShellNewKind::NullFile { .. }))
            .expect("null file item");
        let file_path = create_shell_new_item(&parent, file_item)
            .expect("create file")
            .expect("file path");
        assert!(file_path.is_file());

        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn shell_new_command_without_placeholder_launches_only() {
        refresh_shell_new_menu_cache();
        let items = peek_shell_new_menu_items().expect("cache");
        let pdf_item = items
            .iter()
            .find(|item| item.registry_key.eq_ignore_ascii_case(".pdfwpsshellnew"));
        let Some(pdf_item) = pdf_item else {
            return;
        };
        let parent = std::env::temp_dir().join(format!(
            "cyber_files_shell_new_pdf_cmd_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();

        let result = create_shell_new_item(&parent, pdf_item).expect("launch command");
        assert!(
            result.is_none(),
            "WPS PDF ShellNew command should not fabricate a local path"
        );

        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn unique_shell_new_name_appends_counter() {
        let parent = std::env::temp_dir().join(format!(
            "cyber_files_shell_new_unique_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();

        let first = unique_shell_new_name(&parent, "Text Document", Some(".txt"));
        std::fs::write(parent.join(&first), []).unwrap();
        let second = unique_shell_new_name(&parent, "Text Document", Some(".txt"));
        assert_ne!(first, second);

        let _ = std::fs::remove_dir_all(&parent);
    }
}
