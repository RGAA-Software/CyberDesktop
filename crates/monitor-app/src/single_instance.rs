use std::thread;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    SYNCHRONIZATION_SYNCHRONIZE,
};
use windows::core::PCWSTR;

use crate::tray::{self, TrayCommand};

pub struct SingleInstanceGuard {
    mutex: HANDLE,
    event: HANDLE,
    event_name: String,
}

impl Drop for SingleInstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.event);
            let _ = CloseHandle(self.mutex);
        }
    }
}

impl SingleInstanceGuard {
    pub fn spawn_raise_listener(&self) {
        let event_name = self.event_name.clone();
        thread::spawn(move || unsafe {
            let name_wide = to_wide(&event_name);
            let Ok(event) = OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, false, PCWSTR(name_wide.as_ptr())) else {
                return;
            };
            while WaitForSingleObject(event, windows::Win32::System::Threading::INFINITE)
                == windows::Win32::Foundation::WAIT_OBJECT_0
            {
                tray::push_command(TrayCommand::ShowWindow);
            }
            let _ = CloseHandle(event);
        });
    }
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn ensure_single_instance(mutex_name: &str, event_name: &str) -> Option<SingleInstanceGuard> {
    let mutex_name_wide = to_wide(mutex_name);
    let event_name_wide = to_wide(event_name);

    unsafe {
        let mutex = CreateMutexW(None, true, PCWSTR(mutex_name_wide.as_ptr())).ok()?;
        if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            // Try to signal the existing instance to raise its window.
            if let Ok(event) = OpenEventW(
                EVENT_MODIFY_STATE,
                false,
                PCWSTR(event_name_wide.as_ptr()),
            ) {
                let _ = SetEvent(event);
                let _ = CloseHandle(event);
            }
            return None;
        }

        let event = CreateEventW(None, false, false, PCWSTR(event_name_wide.as_ptr())).ok()?;

        Some(SingleInstanceGuard {
            mutex,
            event,
            event_name: event_name.to_string(),
        })
    }
}
