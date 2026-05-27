//! Whether Windows records recent documents (Explorer privacy).

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
};

const EXPLORER_ADVANCED: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced";
const START_TRACK_DOCS: &str = "Start_TrackDocs";

/// `true` when Explorer is configured to track recently opened documents.
pub fn recent_documents_tracking_enabled() -> bool {
    read_dword(HKEY_CURRENT_USER, EXPLORER_ADVANCED, START_TRACK_DOCS).unwrap_or(1) != 0
}

fn read_dword(root: HKEY, subkey: &str, value: &str) -> Option<u32> {
    unsafe {
        let subkey_wide: Vec<u16> = subkey.encode_utf16().chain([0]).collect();
        let value_wide: Vec<u16> = value.encode_utf16().chain([0]).collect();
        let mut key = HKEY::default();
        if RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut key).is_err() {
            return None;
        }
        let mut kind = REG_DWORD;
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let ok = RegQueryValueExW(
            key,
            PCWSTR(value_wide.as_ptr()),
            None,
            Some(&mut kind),
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size),
        )
        .is_ok();
        let _ = RegCloseKey(key);
        ok.then_some(data)
    }
}
