//! Eject removable volumes and disconnect network drives (Shell `InvokeVerb`).

use std::path::Path;
use std::process::Command;

/// Ejects a removable drive root (`E:\`) or disconnects a network location.
pub fn eject_volume(root: &Path, disconnect_network: bool) -> anyhow::Result<()> {
    let verb = if disconnect_network {
        "Disconnect"
    } else {
        "Eject"
    };
    let parse_name = shell_parse_name(root, disconnect_network)?;
    let script = format!(
        "$ErrorActionPreference='Stop'; \
         $shell = New-Object -ComObject Shell.Application; \
         $folder = $shell.Namespace(17); \
         if ($null -eq $folder) {{ throw 'shell folder unavailable' }}; \
         $item = $folder.ParseName('{parse_name}'); \
         if ($null -eq $item) {{ throw 'target not found' }}; \
         $item.InvokeVerb('{verb}')"
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

fn shell_parse_name(root: &Path, disconnect_network: bool) -> anyhow::Result<String> {
    if disconnect_network {
        let name = root
            .to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_string();
        if name.is_empty() {
            anyhow::bail!("invalid network path");
        }
        return Ok(name.replace('\'', "''"));
    }
    let root_str = root.to_string_lossy();
    let letter = root_str
        .chars()
        .next()
        .filter(|c| c.is_ascii_alphabetic())
        .ok_or_else(|| anyhow::anyhow!("not a drive root: {}", root.display()))?;
    Ok(format!("{letter}:"))
}
