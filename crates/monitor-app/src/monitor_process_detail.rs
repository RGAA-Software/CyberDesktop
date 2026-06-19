use std::collections::BTreeMap;

use sysinfo::{Pid, System};

#[derive(Debug, Clone, Default)]
pub struct ProcessThreadInfo {
    pub tid: u32,
    pub start_address: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessModuleInfo {
    pub name: String,
    pub path: String,
    pub base_address: String,
    pub size: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessNetworkInfo {
    pub protocol: String,
    pub local: String,
    pub remote: String,
    pub state: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessDetailInfo {
    pub threads: Vec<ProcessThreadInfo>,
    pub modules: Vec<ProcessModuleInfo>,
    pub handle_count: u32,
    pub network: Vec<ProcessNetworkInfo>,
    pub environment: BTreeMap<String, String>,
    pub token_user: String,
}

pub fn collect_process_details(pid: u32, system: &System) -> Option<ProcessDetailInfo> {
    let process = system.process(Pid::from_u32(pid))?;

    let environment = parse_environment(process.environ());
    let token_user = collect_token_user(pid).unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        Some(ProcessDetailInfo {
            threads: collect_threads(pid),
            modules: collect_modules(pid),
            handle_count: collect_handle_count(pid).unwrap_or(0),
            network: collect_network(pid),
            environment,
            token_user,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        Some(ProcessDetailInfo {
            environment,
            token_user,
            ..Default::default()
        })
    }
}

fn parse_environment(environ: &[std::ffi::OsString]) -> BTreeMap<String, String> {
    environ
        .iter()
        .filter_map(|entry| {
            let s = entry.to_string_lossy();
            s.find('=').map(|pos| {
                let key = s[..pos].to_string();
                let value = s[pos + 1..].to_string();
                (key, value)
            })
        })
        .collect()
}

#[cfg(target_os = "windows")]
fn collect_threads(pid: u32) -> Vec<ProcessThreadInfo> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };

    let mut threads = Vec::new();
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            Ok(h) if !h.is_invalid() => h,
            _ => return threads,
        };

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    threads.push(ProcessThreadInfo {
                        tid: entry.th32ThreadID,
                        start_address: format!("0x{:016X}", entry.tpBasePri as usize),
                    });
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    threads
}

#[cfg(target_os = "windows")]
fn collect_modules(pid: u32) -> Vec<ProcessModuleInfo> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
        TH32CS_SNAPMODULE32,
    };

    let mut modules = Vec::new();
    unsafe {
        let flags = TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32;
        let snapshot = match CreateToolhelp32Snapshot(flags, pid) {
            Ok(h) if !h.is_invalid() => h,
            _ => return modules,
        };

        let mut entry = MODULEENTRY32W {
            dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32,
            ..Default::default()
        };

        if Module32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = OsString::from_wide(&entry.szModule)
                    .to_string_lossy()
                    .into_owned();
                let path = OsString::from_wide(&entry.szExePath)
                    .to_string_lossy()
                    .into_owned();
                modules.push(ProcessModuleInfo {
                    name,
                    path,
                    base_address: format!("0x{:016X}", entry.modBaseAddr as usize),
                    size: format!("{}", entry.modBaseSize),
                });
                if Module32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    modules
}

#[cfg(target_os = "windows")]
fn collect_handle_count(pid: u32) -> Option<u32> {
    use windows::Wdk::System::Threading::{NtQueryInformationProcess, PROCESSINFOCLASS};
    use windows::Win32::System::Threading::OpenProcess;

    const PROCESS_HANDLE_COUNT: PROCESSINFOCLASS = PROCESSINFOCLASS(20);

    unsafe {
        let handle = match OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        ) {
            Ok(h) if !h.is_invalid() => h,
            _ => return None,
        };

        let mut count = 0u32;
        let mut returned = 0u32;
        let status = NtQueryInformationProcess(
            handle,
            PROCESS_HANDLE_COUNT,
            &mut count as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
            &mut returned,
        );
        let _ = windows::Win32::Foundation::CloseHandle(handle);
        if status.is_ok() {
            Some(count)
        } else {
            None
        }
    }
}

#[cfg(target_os = "windows")]
fn collect_network(pid: u32) -> Vec<ProcessNetworkInfo> {
    let mut network = Vec::new();
    network.extend(collect_tcp_connections(pid));
    network.extend(collect_udp_connections(pid));
    network
}

#[cfg(target_os = "windows")]
fn collect_tcp_connections(pid: u32) -> Vec<ProcessNetworkInfo> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedTcpTable, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_ALL,
    };

    let mut result = Vec::new();
    unsafe {
        let mut size = 0u32;
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            true,
            2, // AF_INET
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if size == 0 {
            return result;
        }

        let mut buffer = vec![0u8; size as usize];
        let ret = GetExtendedTcpTable(
            Some(buffer.as_mut_ptr() as *mut _),
            &mut size,
            true,
            2,
            TCP_TABLE_OWNER_PID_ALL,
            0,
        );
        if ret != 0 {
            return result;
        }

        let table = &*(buffer.as_ptr() as *const MIB_TCPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            if row.dwOwningPid != pid {
                continue;
            }
            result.push(ProcessNetworkInfo {
                protocol: "TCP".to_string(),
                local: format_address_port(row.dwLocalAddr, row.dwLocalPort),
                remote: format_address_port(row.dwRemoteAddr, row.dwRemotePort),
                state: tcp_state_name(row.dwState).to_string(),
                pid: row.dwOwningPid,
            });
        }
    }
    result
}

#[cfg(target_os = "windows")]
fn collect_udp_connections(pid: u32) -> Vec<ProcessNetworkInfo> {
    use windows::Win32::NetworkManagement::IpHelper::{
        GetExtendedUdpTable, MIB_UDPTABLE_OWNER_PID, UDP_TABLE_OWNER_PID,
    };

    let mut result = Vec::new();
    unsafe {
        let mut size = 0u32;
        let _ = GetExtendedUdpTable(
            None,
            &mut size,
            true,
            2, // AF_INET
            UDP_TABLE_OWNER_PID,
            0,
        );
        if size == 0 {
            return result;
        }

        let mut buffer = vec![0u8; size as usize];
        let ret = GetExtendedUdpTable(
            Some(buffer.as_mut_ptr() as *mut _),
            &mut size,
            true,
            2,
            UDP_TABLE_OWNER_PID,
            0,
        );
        if ret != 0 {
            return result;
        }

        let table = &*(buffer.as_ptr() as *const MIB_UDPTABLE_OWNER_PID);
        let rows = std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize);
        for row in rows {
            if row.dwOwningPid != pid {
                continue;
            }
            result.push(ProcessNetworkInfo {
                protocol: "UDP".to_string(),
                local: format_address_port(row.dwLocalAddr, row.dwLocalPort),
                remote: "*:*".to_string(),
                state: "-".to_string(),
                pid: row.dwOwningPid,
            });
        }
    }
    result
}

#[cfg(target_os = "windows")]
fn format_address_port(addr: u32, port: u32) -> String {
    let ip_bytes = addr.to_le_bytes();
    let port = (port as u16).to_be();
    format!(
        "{}.{}.{}.{}:{}",
        ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3], port
    )
}

#[cfg(target_os = "windows")]
fn tcp_state_name(state: u32) -> &'static str {
    match state {
        1 => "CLOSED",
        2 => "LISTEN",
        3 => "SYN_SENT",
        4 => "SYN_RCVD",
        5 => "ESTABLISHED",
        6 => "FIN_WAIT1",
        7 => "FIN_WAIT2",
        8 => "CLOSE_WAIT",
        9 => "CLOSING",
        10 => "LAST_ACK",
        11 => "TIME_WAIT",
        12 => "DELETE_TCB",
        _ => "UNKNOWN",
    }
}

#[cfg(target_os = "windows")]
fn collect_token_user(pid: u32) -> Option<String> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::Security::{
        GetTokenInformation, LookupAccountSidW, TOKEN_QUERY, TOKEN_USER,
    };
    use windows::Win32::System::Threading::{OpenProcess, OpenProcessToken};

    unsafe {
        let process = match OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            pid,
        ) {
            Ok(h) if !h.is_invalid() => h,
            _ => return None,
        };

        let mut token = HANDLE::default();
        if OpenProcessToken(process, TOKEN_QUERY, &mut token).is_err() {
            let _ = CloseHandle(process);
            return None;
        }
        let _ = CloseHandle(process);

        let mut size = 0u32;
        let _ = GetTokenInformation(
            token,
            windows::Win32::Security::TokenUser,
            None,
            0,
            &mut size,
        );
        if size == 0 {
            let _ = CloseHandle(token);
            return None;
        }

        let mut buffer = vec![0u8; size as usize];
        if GetTokenInformation(
            token,
            windows::Win32::Security::TokenUser,
            Some(buffer.as_mut_ptr() as *mut _),
            size,
            &mut size,
        )
        .is_err()
        {
            let _ = CloseHandle(token);
            return None;
        }

        let token_user = &*(buffer.as_ptr() as *const TOKEN_USER);
        let sid = token_user.User.Sid;

        let mut name = [0u16; 256];
        let mut domain = [0u16; 256];
        let mut name_len = name.len() as u32;
        let mut domain_len = domain.len() as u32;
        let mut sid_name_use = Default::default();

        let result = LookupAccountSidW(
            None,
            sid,
            PWSTR(name.as_mut_ptr()),
            &mut name_len,
            PWSTR(domain.as_mut_ptr()),
            &mut domain_len,
            &mut sid_name_use,
        );

        let _ = CloseHandle(token);

        if result.is_ok() {
            let name = OsString::from_wide(&name[..name_len as usize])
                .to_string_lossy()
                .into_owned();
            let domain = OsString::from_wide(&domain[..domain_len as usize])
                .to_string_lossy()
                .into_owned();
            Some(format!("{}\\{}", domain, name))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_environment() {
        let input = vec![
            std::ffi::OsString::from("PATH=C:\\Windows"),
            std::ffi::OsString::from("USER=hy"),
            std::ffi::OsString::from("EMPTY="),
            std::ffi::OsString::from("NO_EQUALS"),
        ];
        let env = parse_environment(&input);
        assert_eq!(env.get("PATH"), Some(&"C:\\Windows".to_string()));
        assert_eq!(env.get("USER"), Some(&"hy".to_string()));
        assert_eq!(env.get("EMPTY"), Some(&"".to_string()));
        assert!(!env.contains_key("NO_EQUALS"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn test_collect_current_process_details() {
        let mut system = System::new_all();
        system.refresh_all();
        let current_pid = std::process::id();
        let details = collect_process_details(current_pid, &system)
            .expect("failed to collect current process details");

        assert!(!details.modules.is_empty(), "modules should not be empty");
        assert!(
            !details.environment.is_empty(),
            "environment should not be empty"
        );
        assert!(
            !details.token_user.is_empty(),
            "token user should not be empty"
        );
        assert!(
            details
                .modules
                .iter()
                .any(|m| m.name.to_lowercase().contains("ntdll")),
            "ntdll module should be present"
        );
    }
}
