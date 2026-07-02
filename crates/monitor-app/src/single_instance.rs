use std::fs::OpenOptions;
use std::io::Write;
use std::thread;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject, EVENT_MODIFY_STATE,
    SYNCHRONIZATION_SYNCHRONIZE,
};

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
            let Ok(event) = OpenEventW(
                SYNCHRONIZATION_SYNCHRONIZE,
                false,
                PCWSTR(name_wide.as_ptr()),
            ) else {
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

fn si_log(msg: &str) {
    let line = format!("[single_instance] {msg}\n");
    let _ = std::io::stderr().write_all(line.as_bytes());
    let _ = std::io::stderr().flush();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let log_path = dir.join("cyber_monitor_startup.log");
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log_path) {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    }
}

pub fn ensure_single_instance(mutex_name: &str, event_name: &str) -> Option<SingleInstanceGuard> {
    let mutex_name_wide = to_wide(mutex_name);
    let event_name_wide = to_wide(event_name);

    unsafe {
        let mutex = match CreateMutexW(None, true, PCWSTR(mutex_name_wide.as_ptr())).ok() {
            Some(m) => m,
            None => {
                let err = windows::Win32::Foundation::GetLastError();
                si_log(&format!("CreateMutexW({mutex_name}) failed: {err:?}"));
                return None;
            }
        };
        if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            si_log(&format!(
                "mutex {mutex_name} already exists; another instance is running"
            ));
            // Try to signal the existing instance to raise its window.
            if let Ok(event) =
                OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(event_name_wide.as_ptr()))
            {
                let _ = SetEvent(event);
                let _ = CloseHandle(event);
            } else {
                si_log(&format!(
                    "could not signal existing instance via event {event_name}"
                ));
            }
            return None;
        }

        let event = match CreateEventW(None, false, false, PCWSTR(event_name_wide.as_ptr())).ok() {
            Some(e) => e,
            None => {
                let err = windows::Win32::Foundation::GetLastError();
                si_log(&format!("CreateEventW({event_name}) failed: {err:?}"));
                let _ = CloseHandle(mutex);
                return None;
            }
        };

        si_log(&format!(
            "acquired mutex {mutex_name} and event {event_name}"
        ));
        Some(SingleInstanceGuard {
            mutex,
            event,
            event_name: event_name.to_string(),
        })
    }
}
