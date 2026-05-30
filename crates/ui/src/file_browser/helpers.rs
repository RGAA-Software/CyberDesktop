use super::*;

impl FileBrowser {
    pub(super) fn file_item_kind_icon(kind: FileItemKind) -> AnyElement {
        match kind {
            FileItemKind::Folder => folder_icon_element(),
            FileItemKind::Symlink => compact_icon(IconName::ExternalLink).into_any_element(),
            FileItemKind::File | FileItemKind::Other => {
                compact_icon(IconName::File).into_any_element()
            }
        }
    }

    /// List row icon: custom colored SVG -> Shell PNG -> GPUI fallback.
    pub(super) fn row_list_icon(
        item: &FileItem,
        logical_size: Pixels,
        window: &Window,
    ) -> impl IntoElement {
        if item.kind == FileItemKind::Folder {
            return div()
                .size(logical_size)
                .flex()
                .items_center()
                .justify_center()
                .child(folder_icon_element())
                .into_any_element();
        }
        if let Some(ext) = item.extension.as_deref().filter(|e| !e.is_empty()) {
            if let Some(path) = list_icon_cache::extension_svg_path(ext) {
                return color_icon::color_icon_box(path, logical_size);
            }
        }
        let px = platform::shell_icon_pixel_size(logical_size.as_f32(), window.scale_factor());
        let key = list_icon_cache::list_icon_key(item);
        if let Some(png) = list_icon_cache::list_icon_png_cached(&key, px) {
            if !png.is_empty() {
                return img(std::sync::Arc::new(Image::from_bytes(
                    ImageFormat::Png,
                    (*png).clone(),
                )))
                .size(logical_size)
                .object_fit(ObjectFit::Contain)
                .into_any_element();
            }
        }
        div()
            .size(logical_size)
            .flex()
            .items_center()
            .justify_center()
            .child(Self::file_item_kind_icon(item.kind))
            .into_any_element()
    }

    /// After directory refresh: load at most one Shell icon per category (folder, zip, exe, ...).
    pub(super) fn schedule_list_icon_warm(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.list_icon_warm_scheduled == self.list_icon_warm_token {
            return;
        }
        self.list_icon_warm_scheduled = self.list_icon_warm_token;
        let keys = list_icon_cache::list_icon_keys_for_items(&self.display_items);
        let px = platform::shell_icon_pixel_size(16., window.scale_factor());
        cx.spawn(async move |this, cx| {
            let _ = cx
                .background_spawn(async move {
                    list_icon_cache::warm_list_icons(keys, px);
                })
                .await;
            let _ = this.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    pub(super) fn set_sort_option(&mut self, option: SortOption) {
        self.sort_preferences.option = option;
        self.refresh();
        self.persist_prefs();
    }

    pub(super) fn sort_label(&self) -> String {
        let field = match self.sort_preferences.option {
            SortOption::Name => t!("files.sort.name"),
            SortOption::DateModified => t!("files.sort.modified"),
            SortOption::DateCreated => t!("files.sort.created"),
            SortOption::Size => t!("files.sort.size"),
            SortOption::FileType => t!("files.sort.type"),
            SortOption::Path => t!("files.sort.path"),
        };
        let arrow = match self.sort_preferences.direction {
            SortDirection::Ascending => "↑",
            SortDirection::Descending => "↓",
        };
        format!("{field} {arrow}")
    }
}

pub(super) fn paths_for_file_tag(tag_name: &str) -> Vec<PathBuf> {
    let config = load_config().unwrap_or_default();
    config
        .file_tags
        .iter()
        .find(|tag| tag.name == tag_name)
        .map(|tag| {
            tag.paths
                .iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn load_files_dir(
    path: &Path,
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> (Vec<FileItem>, Option<String>) {
    match read_directory(path, options, sort) {
        Ok(items) => (items, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

pub(super) fn item_sizes_for(count: usize, mode: ViewMode, size_level: u8) -> Rc<Vec<Size<Pixels>>> {
    let size = match mode {
        ViewMode::Details | ViewMode::List => match size_level {
            1 => FILE_ROW_SIZE_COMPACT,
            3 => FILE_ROW_SIZE_LARGE,
            _ => FILE_ROW_SIZE,
        },
        ViewMode::Grid => match size_level {
            1 => GRID_CELL_SIZE_SMALL,
            3 => GRID_CELL_SIZE_LARGE,
            _ => GRID_CELL_SIZE,
        },
        ViewMode::Cards => CARD_CELL_SIZE,
        ViewMode::Columns => COLUMN_ROW_SIZE,
    };
    Rc::new(vec![size; count.max(1)])
}

pub(super) fn column_listings_for(
    trail: &[PathBuf],
    read_options: &DirectoryReadOptions,
    sort: SortPreferences,
    query: &str,
) -> Vec<Vec<FileItem>> {
    trail
        .iter()
        .map(|path| {
            let (items, _) = load_files_dir(path, *read_options, sort);
            filter_items_by_query(&items, query)
        })
        .collect()
}

pub(super) fn drag_preview_label(paths: &[PathBuf]) -> String {
    if paths.len() == 1 {
        paths[0]
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| t!("files.type.file").to_string())
    } else {
        format!("{} {}", paths.len(), t!("files.status.items"))
    }
}

pub(super) fn drag_preview_hint(paths: &[PathBuf], copy: bool) -> String {
    if paths.is_empty() {
        return String::new();
    }
    let count = paths.len();
    if copy {
        t!("files.drag.preview_copying", count = count).to_string()
    } else {
        t!("files.drag.preview_moving", count = count).to_string()
    }
}

pub(super) fn sort_option_from_config(value: &str) -> SortOption {
    match value {
        "modified" => SortOption::DateModified,
        "created" => SortOption::DateCreated,
        "size" => SortOption::Size,
        "type" => SortOption::FileType,
        "path" => SortOption::Path,
        _ => SortOption::Name,
    }
}

pub(super) fn sort_direction_from_config(value: &str) -> SortDirection {
    match value {
        "desc" => SortDirection::Descending,
        _ => SortDirection::Ascending,
    }
}

pub(super) fn sort_option_config_value(option: SortOption) -> &'static str {
    match option {
        SortOption::Name => "name",
        SortOption::DateModified => "modified",
        SortOption::DateCreated => "created",
        SortOption::Size => "size",
        SortOption::FileType => "type",
        SortOption::Path => "path",
    }
}

#[cfg(windows)]
pub(super) fn open_paths_in_terminal(paths: &[PathBuf]) -> anyhow::Result<()> {
    use std::path::Path;
    use std::process::Command;

    let dirs = paths
        .iter()
        .map(|path| {
            if path.is_dir() {
                Ok(path.clone())
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .ok_or_else(|| anyhow::anyhow!("no parent directory"))
            }
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    if dirs.is_empty() {
        return Ok(());
    }

    let mut args = Vec::with_capacity(dirs.len() * 3);
    for (index, dir) in dirs.iter().enumerate() {
        let dir = dir.to_string_lossy().to_string();
        if index > 0 {
            args.push(";".to_string());
            args.push("nt".to_string());
        }
        args.push("-d".to_string());
        args.push(dir);
    }

    let wt = Command::new("wt.exe").args(&args).spawn();
    if wt.is_ok() {
        return Ok(());
    }

    let dir = dirs[0].to_string_lossy();
    Command::new("cmd")
        .args(["/C", "start", "", "wt.exe", "-d", &dir])
        .spawn()?;
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn open_paths_in_terminal(_paths: &[PathBuf]) -> anyhow::Result<()> {
    anyhow::bail!("terminal launch is only supported on Windows")
}

fn create_shortcut_for_path(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("no parent directory"))?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Shortcut".into());
    let link_path = parent.join(format!("Shortcut to {stem}.lnk"));
    let target = path.to_string_lossy().replace('\'', "''");
    let link = link_path.to_string_lossy().replace('\'', "''");
    let script = format!(
        "$s = (New-Object -ComObject WScript.Shell).CreateShortcut('{link}'); $s.TargetPath='{target}'; $s.Save()"
    );
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("powershell shortcut creation failed")
    }
}

pub(super) fn create_shortcuts_for_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        create_shortcut_for_path(path)?;
    }
    Ok(())
}

pub(super) fn sort_direction_config_value(direction: SortDirection) -> &'static str {
    match direction {
        SortDirection::Ascending => "asc",
        SortDirection::Descending => "desc",
    }
}

pub(super) fn item_type_label(item: &FileItem) -> String {
    match item.kind {
        FileItemKind::Folder => t!("files.type.folder").to_string(),
        FileItemKind::Symlink => t!("files.type.symlink").to_string(),
        FileItemKind::Other => t!("files.type.other").to_string(),
        FileItemKind::File => item
            .extension
            .as_ref()
            .map(|extension| format!("{} file", extension.to_uppercase()))
            .unwrap_or_else(|| t!("files.type.file").to_string()),
    }
}

pub(super) fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return String::new();
    };

    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = size as f64;
    let mut unit = 0;

    while value >= 1024. && unit < UNITS.len() - 1 {
        value /= 1024.;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", size, UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

pub(super) fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };

    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

pub(super) fn create_compress_partial_file(path: &Path) -> anyhow::Result<bool> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(super) fn open_with_system(path: &Path) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(Into::into)
    }
}

pub(super) fn open_with_cybereditor(path: &Path) -> anyhow::Result<()> {
    let Some(editor) = resolve_cybereditor_exe() else {
        return open_with_system(path);
    };
    std::process::Command::new(&editor)
        .arg(path)
        .spawn()?;
    Ok(())
}

fn resolve_cybereditor_exe() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    #[cfg(windows)]
    let name = "cyber_editor.exe";
    #[cfg(not(windows))]
    let name = "cyber_editor";
    let sibling = dir.join(name);
    sibling.is_file().then_some(sibling)
}

pub(super) fn is_executable_or_script_path(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        let ext = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        matches!(
            ext.as_deref(),
            Some("exe")
                | Some("com")
                | Some("bat")
                | Some("cmd")
                | Some("ps1")
                | Some("vbs")
                | Some("js")
                | Some("jse")
                | Some("wsf")
        )
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub(super) fn open_paths_with_target(
    target: &Path,
    source_paths: &[PathBuf],
) -> anyhow::Result<()> {
    if source_paths.is_empty() {
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let ext = target
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());

        if matches!(ext.as_deref(), Some("ps1")) {
            std::process::Command::new("powershell")
                .arg("-NoProfile")
                .arg("-ExecutionPolicy")
                .arg("Bypass")
                .arg("-File")
                .arg(target)
                .args(source_paths)
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        } else {
            std::process::Command::new(target)
                .args(source_paths)
                .spawn()
                .map(|_| ())
                .map_err(Into::into)
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        anyhow::bail!("open-with-target is only supported on Windows")
    }
}
