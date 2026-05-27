//! Volume label and free-space queries (Files drive / network cards).

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::core::PCWSTR;
use windows::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetDriveTypeW, GetVolumeInformationW,
};

const DRIVE_REMOVABLE: u32 = 2;
const DRIVE_FIXED: u32 = 3;
const DRIVE_REMOTE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DriveKind {
    #[default]
    Other,
    Fixed,
    Removable,
    Remote,
}

#[derive(Debug, Clone)]
pub struct VolumeDetails {
    pub volume_label: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
    pub kind: DriveKind,
}

pub fn volume_details(root: &Path) -> VolumeDetails {
    let root_wide = path_to_wide_root(root);
    let kind = drive_kind(&root_wide);
    let mut details = VolumeDetails {
        volume_label: volume_label(&root_wide),
        total_bytes: None,
        free_bytes: None,
        kind,
    };
    if let Some((total, free)) = query_disk_space(&root_wide) {
        details.total_bytes = Some(total);
        details.free_bytes = Some(free);
    }
    details
}

impl VolumeDetails {
    pub fn space_text(&self) -> Option<String> {
        let total = self.total_bytes?;
        let free = self.free_bytes?;
        let used = total.saturating_sub(free);
        Some(format!("{} / {}", format_bytes(used), format_bytes(total)))
    }

    pub fn used_fraction(&self) -> Option<f32> {
        let total = self.total_bytes?;
        let free = self.free_bytes?;
        if total == 0 {
            return None;
        }
        Some((total.saturating_sub(free) as f32) / total as f32)
    }
}

fn path_to_wide_root(path: &Path) -> Vec<u16> {
    let mut wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    if wide.len() >= 2 && wide[1] == b':' as u16 {
        wide.truncate(3);
        if wide.get(2) != Some(&(b'\\' as u16)) {
            wide[2] = b'\\' as u16;
        }
    }
    wide
}

fn drive_kind(root: &[u16]) -> DriveKind {
    let drive_type = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    match drive_type {
        DRIVE_REMOVABLE => DriveKind::Removable,
        DRIVE_FIXED => DriveKind::Fixed,
        DRIVE_REMOTE => DriveKind::Remote,
        _ => DriveKind::Other,
    }
}

fn volume_label(root: &[u16]) -> Option<String> {
    let mut label = [0u16; 261];
    unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            Some(&mut label),
            None,
            None,
            None,
            None,
        )
        .ok()?;
    }
    let end = label.iter().position(|&c| c == 0).unwrap_or(label.len());
    if end == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&label[..end]))
}

fn query_disk_space(root: &[u16]) -> Option<(u64, u64)> {
    let mut free = 0u64;
    let mut total = 0u64;
    unsafe {
        GetDiskFreeSpaceExW(
            PCWSTR(root.as_ptr()),
            Some(&mut free),
            Some(&mut total),
            None,
        )
        .ok()?;
    }
    Some((total, free))
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{value:.1} {UN}", UN = UNITS[unit])
    }
}
