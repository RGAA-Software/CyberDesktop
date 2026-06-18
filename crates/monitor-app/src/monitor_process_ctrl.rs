use std::collections::{HashMap, HashSet};

pub fn parse_priority(priority: &str) -> Option<u32> {
    match priority.to_lowercase().as_str() {
        "idle" | "低" => Some(64),
        "below_normal" | "低于标准" => Some(16384),
        "normal" | "标准" => Some(32),
        "above_normal" | "高于标准" => Some(32768),
        "high" | "高" => Some(128),
        "realtime" | "实时" => Some(256),
        _ => None,
    }
}

pub fn priority_label(priority_class: u32) -> &'static str {
    match priority_class {
        64 => "idle",
        16384 => "below_normal",
        32 => "normal",
        32768 => "above_normal",
        128 => "high",
        256 => "realtime",
        _ => "unknown",
    }
}

pub fn parse_io_priority(priority: &str) -> Option<i32> {
    match priority.to_lowercase().as_str() {
        "very_low" | "很低" => Some(0),
        "low" | "低" => Some(1),
        "normal" | "标准" => Some(2),
        "high" | "高" => Some(3),
        _ => None,
    }
}

pub fn io_priority_label(value: i32) -> &'static str {
    match value {
        0 => "very_low",
        1 => "low",
        2 => "normal",
        3 => "high",
        _ => "unknown",
    }
}

pub fn affinity_from_cores(cores: &[usize]) -> u64 {
    cores.iter().fold(0u64, |mask, &core| {
        if core < 64 {
            mask | (1u64 << core)
        } else {
            mask
        }
    })
}

pub fn cores_from_affinity(mask: u64) -> Vec<usize> {
    (0..64).filter(|&i| mask & (1u64 << i) != 0).collect()
}

pub fn build_process_tree(processes: &[(u32, Option<u32>)]) -> HashMap<u32, Vec<u32>> {
    let mut tree: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pid, parent) in processes {
        tree.entry(*pid).or_default();
        if let Some(parent) = parent {
            tree.entry(*parent).or_default().push(*pid);
        }
    }
    tree
}

pub fn collect_descendants(tree: &HashMap<u32, Vec<u32>>, root: u32) -> Vec<u32> {
    let mut result = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }
        if pid != root {
            result.push(pid);
        }
        if let Some(children) = tree.get(&pid) {
            for &child in children {
                stack.push(child);
            }
        }
    }
    result
}

#[cfg(target_os = "windows")]
pub fn set_process_priority(pid: u32, priority: &str) -> bool {
    windows::set_process_priority(pid, priority)
}

#[cfg(not(target_os = "windows"))]
pub fn set_process_priority(_pid: u32, _priority: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn set_process_io_priority(pid: u32, priority: &str) -> bool {
    windows::set_process_io_priority(pid, priority)
}

#[cfg(not(target_os = "windows"))]
pub fn set_process_io_priority(_pid: u32, _priority: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn set_process_affinity(pid: u32, mask: u64) -> bool {
    windows::set_process_affinity(pid, mask)
}

#[cfg(not(target_os = "windows"))]
pub fn set_process_affinity(_pid: u32, _mask: u64) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn suspend_process(pid: u32) -> bool {
    windows::suspend_process(pid)
}

#[cfg(not(target_os = "windows"))]
pub fn suspend_process(_pid: u32) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn resume_process(pid: u32) -> bool {
    windows::resume_process(pid)
}

#[cfg(not(target_os = "windows"))]
pub fn resume_process(_pid: u32) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn terminate_process(pid: u32) -> bool {
    windows::terminate_process(pid)
}

#[cfg(not(target_os = "windows"))]
pub fn terminate_process(_pid: u32) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn terminate_process_tree(pid: u32, processes: &[(u32, Option<u32>)]) -> bool {
    let tree = build_process_tree(processes);
    let mut targets = collect_descendants(&tree, pid);
    targets.push(pid);
    targets
        .iter()
        .all(|&target| windows::terminate_process(target))
}

#[cfg(not(target_os = "windows"))]
pub fn terminate_process_tree(_pid: u32, _processes: &[(u32, Option<u32>)]) -> bool {
    false
}

#[cfg(target_os = "windows")]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStrExt;

    use windows::Win32::Foundation::{CloseHandle, HANDLE, NTSTATUS};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Threading::{
        OpenProcess, SetPriorityClass, SetProcessAffinityMask, TerminateProcess,
        PROCESS_CREATION_FLAGS, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION,
        PROCESS_TERMINATE,
    };

    type NtSetInformationProcessFn = unsafe extern "system" fn(
        processhandle: HANDLE,
        processinformationclass: i32,
        processinformation: *const core::ffi::c_void,
        processinformationsize: u32,
    ) -> NTSTATUS;

    const PROCESS_IO_PRIORITY: i32 = 33;

    pub fn set_process_priority(pid: u32, priority: &str) -> bool {
        let Some(class) = super::parse_priority(priority) else {
            return false;
        };
        unsafe {
            let handle = match OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };
            let result = SetPriorityClass(handle, PROCESS_CREATION_FLAGS(class)).is_ok();
            let _ = CloseHandle(handle);
            result
        }
    }

    pub fn set_process_io_priority(pid: u32, priority: &str) -> bool {
        let Some(value) = super::parse_io_priority(priority) else {
            return false;
        };
        unsafe {
            let handle = match OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };

            let result = if let Some(nt_set) = load_nt_set_information_process() {
                let info: i32 = value;
                nt_set(
                    handle,
                    PROCESS_IO_PRIORITY,
                    &info as *const _ as *const _,
                    std::mem::size_of::<i32>() as u32,
                )
                .is_ok()
            } else {
                false
            };

            let _ = CloseHandle(handle);
            result
        }
    }

    pub fn set_process_affinity(pid: u32, mask: u64) -> bool {
        unsafe {
            let handle = match OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };
            let result = SetProcessAffinityMask(handle, mask as usize).is_ok();
            let _ = CloseHandle(handle);
            result
        }
    }

    pub fn suspend_process(pid: u32) -> bool {
        change_process_threads(pid, true)
    }

    pub fn resume_process(pid: u32) -> bool {
        change_process_threads(pid, false)
    }

    fn change_process_threads(pid: u32, suspend: bool) -> bool {
        use windows::Win32::System::Threading::{
            OpenThread, ResumeThread, SuspendThread, THREAD_SUSPEND_RESUME,
        };

        unsafe {
            let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };

            let mut entry = THREADENTRY32 {
                dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
                ..Default::default()
            };

            let mut success = false;
            if Thread32First(snapshot, &mut entry).is_ok() {
                loop {
                    if entry.th32OwnerProcessID == pid {
                        if let Ok(thread) =
                            OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID)
                        {
                            if !thread.is_invalid() {
                                if suspend {
                                    let _ = SuspendThread(thread);
                                } else {
                                    let _ = ResumeThread(thread);
                                }
                                let _ = CloseHandle(thread);
                                success = true;
                            }
                        }
                    }
                    if Thread32Next(snapshot, &mut entry).is_err() {
                        break;
                    }
                }
            }
            let _ = CloseHandle(snapshot);
            success
        }
    }

    pub fn terminate_process(pid: u32) -> bool {
        unsafe {
            let handle = match OpenProcess(
                PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION,
                false,
                pid,
            ) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };
            let result = TerminateProcess(handle, 1).is_ok();
            let _ = CloseHandle(handle);
            result
        }
    }

    #[cfg(test)]
    pub fn get_process_priority_class(pid: u32) -> Option<u32> {
        use windows::Win32::System::Threading::GetPriorityClass;
        unsafe {
            let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
                Ok(h) if !h.is_invalid() => h,
                _ => return None,
            };
            let result = GetPriorityClass(handle);
            let _ = CloseHandle(handle);
            if result == 0 {
                None
            } else {
                Some(result)
            }
        }
    }

    unsafe fn load_nt_set_information_process() -> Option<NtSetInformationProcessFn> {
        let wide: Vec<u16> = OsString::from("ntdll.dll")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let module = LoadLibraryW(windows::core::PCWSTR(wide.as_ptr())).ok()?;
        let proc = GetProcAddress(module, windows::core::s!("NtSetInformationProcess"))?;
        Some(std::mem::transmute(proc))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_priority() {
        assert_eq!(parse_priority("idle"), Some(64));
        assert_eq!(parse_priority("below_normal"), Some(16384));
        assert_eq!(parse_priority("normal"), Some(32));
        assert_eq!(parse_priority("above_normal"), Some(32768));
        assert_eq!(parse_priority("high"), Some(128));
        assert_eq!(parse_priority("realtime"), Some(256));
        assert_eq!(parse_priority("unknown"), None);
    }

    #[test]
    fn test_parse_io_priority() {
        assert_eq!(parse_io_priority("very_low"), Some(0));
        assert_eq!(parse_io_priority("low"), Some(1));
        assert_eq!(parse_io_priority("normal"), Some(2));
        assert_eq!(parse_io_priority("high"), Some(3));
        assert_eq!(parse_io_priority("unknown"), None);
    }

    #[test]
    fn test_affinity_conversions() {
        assert_eq!(affinity_from_cores(&[0, 2, 4]), 0b10101);
        assert_eq!(cores_from_affinity(0b10101), vec![0, 2, 4]);
        assert!(cores_from_affinity(0).is_empty());
    }

    #[test]
    fn test_build_process_tree() {
        let processes = vec![(1, None), (2, Some(1)), (3, Some(1)), (4, Some(2))];
        let tree = build_process_tree(&processes);
        assert_eq!(tree.get(&1).unwrap().len(), 2);
        assert!(tree.get(&1).unwrap().contains(&2));
        assert!(tree.get(&1).unwrap().contains(&3));
        assert_eq!(tree.get(&2).unwrap(), &vec![4]);
    }

    #[test]
    fn test_collect_descendants() {
        let processes = vec![(1, None), (2, Some(1)), (3, Some(1)), (4, Some(2))];
        let tree = build_process_tree(&processes);
        let descendants = collect_descendants(&tree, 1);
        assert_eq!(descendants.len(), 3);
        assert!(descendants.contains(&2));
        assert!(descendants.contains(&3));
        assert!(descendants.contains(&4));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_set_and_get_priority_class() {
        let mut child = std::process::Command::new("ping")
            .arg("-t")
            .arg("127.0.0.1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to start test child process");
        let pid = child.id();

        std::thread::sleep(std::time::Duration::from_millis(200));

        let result = std::panic::catch_unwind(|| {
            assert!(
                set_process_priority(pid, "below_normal"),
                "failed to set priority"
            );
            assert_eq!(
                windows::get_process_priority_class(pid),
                Some(16384),
                "priority class mismatch"
            );
            assert!(
                set_process_priority(pid, "normal"),
                "failed to restore priority"
            );
            assert_eq!(windows::get_process_priority_class(pid), Some(32));
        });

        let _ = child.kill();
        let _ = child.wait();
        result.unwrap();
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_suspend_resume_and_terminate_process() {
        let mut child = std::process::Command::new("ping")
            .arg("-t")
            .arg("127.0.0.1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to start test child process");
        let pid = child.id();

        std::thread::sleep(std::time::Duration::from_millis(200));

        assert!(suspend_process(pid), "failed to suspend test child process");
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(resume_process(pid), "failed to resume test child process");
        assert!(
            terminate_process(pid),
            "failed to terminate test child process"
        );

        let _ = child.wait();
    }

    #[test]
    fn test_terminate_process_tree_targets() {
        let processes = vec![
            (1, None),
            (2, Some(1)),
            (3, Some(1)),
            (4, Some(2)),
            (5, Some(2)),
        ];
        let tree = build_process_tree(&processes);
        let targets = collect_descendants(&tree, 1);
        assert_eq!(targets.len(), 4);
        assert!(targets.contains(&2));
        assert!(targets.contains(&3));
        assert!(targets.contains(&4));
        assert!(targets.contains(&5));

        let subtree = collect_descendants(&tree, 2);
        assert_eq!(subtree.len(), 2);
        assert!(subtree.contains(&4));
        assert!(subtree.contains(&5));
    }
}
