//! Open Windows Storage / Storage Sense settings (Home drive cards).

use std::process::Command;

/// Opens the Storage Sense page in Windows Settings.
pub fn open_storage_sense_settings() -> anyhow::Result<()> {
    let status = Command::new("cmd")
        .args(["/c", "start", "", "ms-settings:storagesense"])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to open Storage Sense settings ({status})");
    }
    Ok(())
}
