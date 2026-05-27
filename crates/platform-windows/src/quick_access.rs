//! Shell Frequent-places folder — Explorer Quick Access pins.

use std::path::Path;
use std::process::Command;

use crate::shell_folder::{list_known_folder_folders, ShellFolderEntry, FOLDERID_FREQUENT};

/// One pinned folder from the Shell Quick Access / Frequent list.
pub type ShellQuickAccessEntry = ShellFolderEntry;

/// Lists folders pinned to Windows Quick Access (Frequent places known folder).
pub fn list_shell_quick_access_folders() -> anyhow::Result<Vec<ShellQuickAccessEntry>> {
    list_known_folder_folders(&FOLDERID_FREQUENT)
}

/// Pin `path` to Explorer Quick Access (`pintohome` verb).
pub fn shell_pin_to_quick_access(path: &Path) -> anyhow::Result<()> {
    shell_invoke_folder_verb(path, "pintohome")
}

/// Remove `path` from Explorer Quick Access (`unpinfromhome` verb).
pub fn shell_unpin_from_quick_access(path: &Path) -> anyhow::Result<()> {
    shell_invoke_folder_verb(path, "unpinfromhome")
}

fn shell_invoke_folder_verb(path: &Path, verb: &str) -> anyhow::Result<()> {
    if !path.exists() {
        anyhow::bail!("path not found: {}", path.display());
    }
    let path_lit = path
        .to_string_lossy()
        .replace('\'', "''")
        .trim_end_matches(['\\', '/'])
        .to_string();
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $shell = New-Object -ComObject Shell.Application; \
         $dir = $shell.Namespace('{path_lit}'); \
         if ($null -eq $dir) {{ throw 'namespace not found' }}; \
         $dir.Self.InvokeVerb('{verb}')"
    );
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .status()?;
    if !status.success() {
        anyhow::bail!("{verb} failed ({status})");
    }
    Ok(())
}
