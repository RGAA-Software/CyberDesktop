use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use adlx::helper::AdlxHelper;
use anyhow::Result;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use sysinfo::{Components, Disks, Networks, Pid, ProcessStatus, System, Users};

use crate::monitor_process_detail::{collect_process_details, ProcessDetailInfo};
use crate::sys_info::{
    SysComponentInfo, SysCpuInfo, SysDiskInfo, SysGpuInfo, SysInfo, SysIpNetwork, SysMemInfo,
    SysNetworkInfo, SysOsInfo, SysProcessInfo, SysServiceInfo, SysSingleCpuInfo, SysStartupInfo,
    SysUserInfo,
};

fn truncate_string(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

fn format_process_status(status: ProcessStatus) -> &'static str {
    match status {
        ProcessStatus::Run => "运行中",
        ProcessStatus::Sleep => "休眠",
        ProcessStatus::Idle => "空闲",
        ProcessStatus::Stop => "已停止",
        ProcessStatus::Zombie => "僵尸",
        ProcessStatus::Dead => "无响应",
        ProcessStatus::Tracing => "跟踪",
        ProcessStatus::Wakekill => "唤醒终止",
        ProcessStatus::Waking => "唤醒中",
        ProcessStatus::Parked => "已驻留",
        ProcessStatus::LockBlocked => "锁阻塞",
        ProcessStatus::UninterruptibleDiskSleep => "不可中断磁盘睡眠",
        ProcessStatus::Unknown(_) => "未知",
    }
}

fn current_timestamp_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn current_readable_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(target_os = "windows")]
unsafe fn expand_environment_strings(input: &str) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use windows::core::PCWSTR;
    use windows::Win32::System::Environment::ExpandEnvironmentStringsW;

    let wide: Vec<u16> = OsString::from(input)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut buf = vec![0u16; 4096];
    let len = ExpandEnvironmentStringsW(PCWSTR(wide.as_ptr()), Some(&mut buf));
    if len > 0 && (len as usize) <= buf.len() {
        let len = len as usize;
        OsString::from_wide(&buf[..len.saturating_sub(1)])
            .to_string_lossy()
            .into_owned()
    } else {
        input.to_string()
    }
}

#[cfg(target_os = "windows")]
unsafe fn wide_ptr_to_string(ptr: *const u16) -> String {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    if ptr.is_null() {
        return String::new();
    }
    let len = (0..).take_while(|&i| *ptr.add(i) != 0).count();
    OsString::from_wide(std::slice::from_raw_parts(ptr, len))
        .to_string_lossy()
        .into_owned()
}

#[derive(Debug, Clone)]
struct DefaultEthernet {
    ipv4: String,
    transmit_speed: u64,
    receive_speed: u64,
}

pub struct SysInfoManager {
    system: System,
    networks: Networks,
    disks: Disks,
    components: Components,
    users: Users,
    max_frequency: f32,
    def_ethernet: Option<DefaultEthernet>,
    last_process_io: HashMap<u32, (u64, u64)>,
    last_sample_time: Option<Instant>,
}

impl SysInfoManager {
    pub fn new() -> Self {
        let system = System::new_all();
        let networks = Networks::new_with_refreshed_list();
        let disks = Disks::new_with_refreshed_list();
        let components = Components::new_with_refreshed_list();
        let users = Users::new_with_refreshed_list();

        SysInfoManager {
            system,
            networks,
            disks,
            components,
            users,
            max_frequency: 0.0,
            def_ethernet: None,
            last_process_io: HashMap::new(),
            last_sample_time: None,
        }
    }

    pub fn load_system_info(&mut self) -> SysInfo {
        self.system.refresh_all();
        self.networks.refresh(true);
        self.disks.refresh(true);
        self.components.refresh(true);
        self.users.refresh();

        let usage = self.system.global_cpu_usage();
        let vendor = self.system.cpus()[0].vendor_id();
        let brand = self.system.cpus()[0].brand();
        let base_frequency = self.system.cpus()[0].frequency() as f32 / 1000.0;
        let current_frequency = self.load_current_frequency(base_frequency);
        if self.max_frequency <= 0.1 {
            self.max_frequency = calcmhz::mhz().unwrap_or(0.0) as f32 / 1000.0;
        }
        let mut cpus = Vec::new();
        for cpu in self.system.cpus() {
            let single_cpu = SysSingleCpuInfo {
                name: cpu.name().to_string(),
                usage: cpu.cpu_usage(),
            };
            cpus.push(single_cpu);
        }
        let physical_cores = sysinfo::System::physical_core_count()
            .unwrap_or(cpus.len())
            .max(1);
        let cpu = SysCpuInfo {
            usage,
            vendor: vendor.to_string(),
            brand: brand.to_string(),
            base_frequency,
            current_frequency,
            max_frequency: self.max_frequency,
            cpus,
            physical_cores,
        };

        let gb = 1024 * 1024 * 1024;
        let mem = SysMemInfo {
            total: self.system.total_memory(),
            total_gb: self.system.total_memory() / gb,
            used: self.system.used_memory(),
            used_gb: self.system.used_memory() / gb,
            available: self.system.available_memory(),
            available_gb: self.system.available_memory() / gb,
        };

        let mut disks_info = Vec::new();
        for disk in &mut self.disks {
            let usage = disk.usage();
            disks_info.push(SysDiskInfo {
                disk_type: disk.kind().to_string(),
                mount_on: disk.mount_point().to_str().unwrap_or("").to_string(),
                filesystem: disk.file_system().to_str().unwrap_or("").to_string(),
                available: disk.available_space(),
                available_gb: disk.available_space() / gb,
                total: disk.total_space(),
                total_gb: disk.total_space() / gb,
                read_bytes: usage.total_read_bytes,
                written_bytes: usage.total_written_bytes,
                read_rate: usage.read_bytes as f64 / 1024.0 / 1024.0,
                write_rate: usage.written_bytes as f64 / 1024.0 / 1024.0,
            });
        }

        if self.def_ethernet.is_none() {
            let def_ethernet = match netdev::get_default_interface() {
                Ok(interface) => {
                    if interface.ipv4.is_empty() {
                        None
                    } else {
                        Some(DefaultEthernet {
                            ipv4: interface.ipv4[0].addr().to_string(),
                            transmit_speed: interface.transmit_speed.unwrap_or(0),
                            receive_speed: interface.receive_speed.unwrap_or(0),
                        })
                    }
                }
                _ => None,
            };
            self.def_ethernet = def_ethernet;
        }

        let mut networks = Vec::new();
        for (interface_name, data) in self.networks.iter() {
            if interface_name.contains("VMware") {
                continue;
            }

            let mut max_transmit_speed = 0;
            let mut max_receive_speed = 0;
            let mut nts = Vec::new();
            let mut _found_def_ethernet = false;
            for nt in data.ip_networks() {
                let addr = nt.addr.to_string();
                if addr.contains(':') || addr.contains("::") {
                    continue;
                }
                nts.push(SysIpNetwork {
                    addr: addr.clone(),
                    prefix: nt.prefix,
                });

                if let Some(def_ethernet) = self.def_ethernet.clone() {
                    if addr == def_ethernet.ipv4 {
                        max_transmit_speed = def_ethernet.transmit_speed;
                        max_receive_speed = def_ethernet.receive_speed;
                        _found_def_ethernet = true;
                    }
                }
            }

            networks.push(SysNetworkInfo {
                name: interface_name.clone(),
                mac: data.mac_address().to_string(),
                ip_networks: nts,
                received_data: data.total_received(),
                sent_data: data.total_transmitted(),
                max_transmit_speed,
                max_receive_speed,
            });
        }

        let os = SysOsInfo {
            sys_name: System::name().unwrap_or_else(|| "<unknown>".to_owned()),
            sys_kernel_version: System::kernel_version().unwrap_or_else(|| "<unknown>".to_owned()),
            sys_os_version: System::os_version().unwrap_or_else(|| "<unknown>".to_owned()),
            sys_os_long_version: System::long_os_version()
                .unwrap_or_else(|| "<unknown>".to_owned()),
            sys_host_name: System::host_name().unwrap_or_else(|| "<unknown>".to_owned()),
            sys_kernel: System::kernel_long_version().to_string(),
        };

        let mut cps = Vec::new();
        for component in self.components.iter() {
            cps.push(SysComponentInfo {
                temperature: component.temperature().unwrap_or(0.0),
                max: component.max().unwrap_or(0.0),
                critical: component.critical().unwrap_or(0.0),
                label: component.label().to_string(),
            });
        }

        let up = System::uptime();
        let mut uptime = up;
        let days = uptime / 86400;
        uptime -= days * 86400;
        let hours = uptime / 3600;
        uptime -= hours * 3600;
        let minutes = uptime / 60;
        let uptime = format!("{days} days {hours} hours {minutes} minutes");

        let mut gpus = Vec::new();
        let nvml = Nvml::init();
        if let Ok(nvml) = nvml {
            let device_count = nvml.device_count().unwrap_or(0);
            for i in 0..device_count {
                let mut gpu_info = SysGpuInfo::default();
                let device = nvml.device_by_index(i);
                if let Ok(device) = device {
                    if let Ok(serial) = device.serial() {
                        gpu_info.id = serial;
                    } else if let Ok(uuid) = device.uuid() {
                        gpu_info.id = uuid;
                    }

                    let brand = device.name();
                    if let Ok(brand) = brand {
                        gpu_info.brand = brand.trim_matches('"').replace('"', "");
                    }

                    gpu_info.fan_speed = device.fan_speed_rpm(0).unwrap_or(0);
                    gpu_info.power_limit = device.enforced_power_limit().unwrap_or(0);

                    if let Ok(u) = device.encoder_utilization() {
                        gpu_info.encoder_utilization = u.utilization;
                    }

                    if let Ok(u) = device.utilization_rates() {
                        gpu_info.gpu_utilization = u.gpu;
                        gpu_info.mem_utilization = u.memory;
                    }

                    gpu_info.temperature = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);

                    if let Ok(mi) = device.memory_info() {
                        gpu_info.mem_free = mi.free;
                        gpu_info.mem_free_gb = mi.free as f32 * 1.0 / (gb as f32);
                        gpu_info.mem_total = mi.total;
                        gpu_info.mem_total_gb = mi.total as f32 * 1.0 / (gb as f32);
                        gpu_info.mem_used = mi.used;
                        gpu_info.mem_used_gb = mi.used as f32 * 1.0 / (gb as f32);
                    }

                    gpus.push(gpu_info);
                }
            }
        }

        if let Ok(amd_gpus) = self.load_amd_gpu_info() {
            for amd_gpu_info in amd_gpus {
                gpus.push(amd_gpu_info);
            }
        }

        let now = Instant::now();
        let elapsed_secs = self
            .last_sample_time
            .map(|last| last.elapsed().as_secs_f64())
            .unwrap_or(0.0)
            .max(0.001);

        let mut processes: Vec<SysProcessInfo> = self
            .system
            .processes()
            .values()
            .map(|process| {
                let disk = process.disk_usage();
                let pid = process.pid().as_u32();
                let command_line = process
                    .cmd()
                    .iter()
                    .map(|arg| arg.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
                let exe = process
                    .exe()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let name = process.name().to_string_lossy().into_owned();
                let name = if name.is_empty() {
                    exe.rsplit_once(|c| c == '\\' || c == '/')
                        .map(|(_, file)| file.to_string())
                        .unwrap_or_else(|| exe.clone())
                } else {
                    name
                };

                let (read_rate, write_rate) = self
                    .last_process_io
                    .get(&pid)
                    .map(|(last_read, last_write)| {
                        let read_delta = disk.read_bytes.saturating_sub(*last_read);
                        let write_delta = disk.written_bytes.saturating_sub(*last_write);
                        (
                            read_delta as f64 / elapsed_secs / 1024.0 / 1024.0,
                            write_delta as f64 / elapsed_secs / 1024.0 / 1024.0,
                        )
                    })
                    .unwrap_or((0.0, 0.0));

                SysProcessInfo {
                    pid,
                    parent_pid: process.parent().map(|p| p.as_u32()).unwrap_or(0),
                    name: truncate_string(&name, 128),
                    command_line: if command_line.is_empty() {
                        truncate_string(&exe, 256)
                    } else {
                        truncate_string(&command_line, 256)
                    },
                    exe: truncate_string(&exe, 256),
                    status: format_process_status(process.status()).to_string(),
                    cpu_usage: process.cpu_usage(),
                    memory: process.memory(),
                    memory_mb: process.memory() / 1024 / 1024,
                    virtual_memory: process.virtual_memory(),
                    virtual_memory_mb: process.virtual_memory() / 1024 / 1024,
                    disk_read_bytes: disk.read_bytes,
                    disk_written_bytes: disk.written_bytes,
                    disk_read_rate: read_rate,
                    disk_write_rate: write_rate,
                    start_time: process.start_time(),
                    run_time: process.run_time(),
                }
            })
            .collect();

        self.last_process_io.clear();
        for process in &processes {
            self.last_process_io.insert(
                process.pid,
                (process.disk_read_bytes, process.disk_written_bytes),
            );
        }
        self.last_sample_time = Some(now);
        processes.sort_by(|a, b| {
            b.cpu_usage
                .partial_cmp(&a.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let services = self.load_services();
        let startup_items = self.load_startup_items();
        let users = self.load_users();

        SysInfo {
            timestamp: current_timestamp_ms(),
            timestamp_readable: current_readable_timestamp(),
            cpu,
            mem,
            disks: disks_info,
            networks,
            os,
            components: cps,
            uptime,
            gpus,
            processes,
            services,
            startup_items,
            users,
        }
    }

    fn load_current_frequency(&self, fallback: f32) -> f32 {
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::System::Power::{
                CallNtPowerInformation, ProcessorInformation, PROCESSOR_POWER_INFORMATION,
            };

            let cpu_count = self.system.cpus().len();
            if cpu_count == 0 {
                return fallback;
            }

            let mut buffer = vec![PROCESSOR_POWER_INFORMATION::default(); cpu_count];
            let status = unsafe {
                CallNtPowerInformation(
                    ProcessorInformation,
                    None,
                    0,
                    Some(buffer.as_mut_ptr() as *mut _),
                    (std::mem::size_of::<PROCESSOR_POWER_INFORMATION>() * cpu_count) as u32,
                )
            };
            if status.is_ok() {
                let total_mhz: u64 = buffer.iter().map(|info| info.CurrentMhz as u64).sum();
                return total_mhz as f32 / cpu_count as f32 / 1000.0;
            }
        }

        let total: f32 = self
            .system
            .cpus()
            .iter()
            .map(|c| c.frequency() as f32)
            .sum();
        if self.system.cpus().is_empty() {
            fallback
        } else {
            total / self.system.cpus().len() as f32 / 1000.0
        }
    }

    fn load_amd_gpu_info(&self) -> Result<Vec<SysGpuInfo>, anyhow::Error> {
        let helper = AdlxHelper::new()?;
        let system = helper.system();
        let gpu_list = system.gpus()?;
        let performance_monitoring_services = system.performance_monitoring_services()?;

        let mut gpus_info = Vec::new();
        for gpu in 0..gpu_list.size() {
            let gpu = gpu_list.at(gpu)?;

            let gpu_id = if let Ok(id) = gpu.unique_id() {
                id.to_string()
            } else {
                String::new()
            };

            let gpu_name = gpu.name().unwrap_or("<unknown>");
            let gpu_ram = gpu.total_vram().unwrap_or(0);

            let gpu_metrics = performance_monitoring_services.current_gpu_metrics(&gpu)?;
            let supported_metrics = performance_monitoring_services.supported_gpu_metrics(&gpu)?;

            let gpu_usage = if supported_metrics.is_supported_gpu_usage().unwrap_or(false) {
                gpu_metrics.usage().unwrap_or(0.0)
            } else {
                0.0
            };

            let gpu_used_ram = if supported_metrics.is_supported_gpu_vram().unwrap_or(false) {
                gpu_metrics.vram().unwrap_or(0)
            } else {
                0
            };

            let gpu_fan_speed = if supported_metrics
                .is_supported_gpu_fan_speed()
                .unwrap_or(false)
            {
                gpu_metrics.fan_speed().unwrap_or(0)
            } else {
                0
            };

            let gpu_temperature = if supported_metrics
                .is_supported_gpu_temperature()
                .unwrap_or(false)
            {
                gpu_metrics.temperature().unwrap_or(0.0)
            } else {
                0.0
            };

            let mut info = SysGpuInfo::default();
            info.id = gpu_id;
            info.brand = gpu_name.to_string();
            info.gpu_utilization = gpu_usage as u32;
            info.mem_total_gb = gpu_ram as f32 * 1.0 / 1024.0;
            info.mem_used_gb = gpu_used_ram as f32 * 1.0 / 1024.0;
            info.fan_speed = gpu_fan_speed as u32;
            info.temperature = gpu_temperature as u32;
            gpus_info.push(info);
        }

        Ok(gpus_info)
    }

    pub fn load_system_info_as_json(&mut self) -> String {
        let info = self.load_system_info();
        serde_json::to_string(&info).unwrap_or_default()
    }

    #[cfg(target_os = "windows")]
    fn load_services(&self) -> Vec<SysServiceInfo> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Foundation::ERROR_MORE_DATA;
        use windows::Win32::System::Services::{
            CloseServiceHandle, EnumServicesStatusExW, OpenSCManagerW, OpenServiceW,
            QueryServiceConfigW, ENUM_SERVICE_STATUS_PROCESSW, QUERY_SERVICE_CONFIGW, SC_ENUM_TYPE,
            SC_MANAGER_ENUMERATE_SERVICE, SERVICE_QUERY_CONFIG, SERVICE_STATE_ALL, SERVICE_WIN32,
        };

        let mut services = Vec::new();
        unsafe {
            let scm = match OpenSCManagerW(None, None, SC_MANAGER_ENUMERATE_SERVICE) {
                Ok(h) if !h.is_invalid() => h,
                _ => return services,
            };

            let mut resume_handle = 0u32;
            let mut buffer = vec![0u8; 64 * 1024];
            let mut bytes_needed = 0u32;
            let mut services_returned = 0u32;

            loop {
                let result = EnumServicesStatusExW(
                    scm,
                    SC_ENUM_TYPE::default(),
                    SERVICE_WIN32,
                    SERVICE_STATE_ALL,
                    Some(buffer.as_mut_slice()),
                    &mut bytes_needed,
                    &mut services_returned,
                    Some(&mut resume_handle),
                    None,
                );
                let more_data = result
                    .as_ref()
                    .err()
                    .map(|e| e.code() == ERROR_MORE_DATA.into())
                    .unwrap_or(false);

                if result.is_ok() || more_data {
                    let slice = std::slice::from_raw_parts(
                        buffer.as_ptr() as *const ENUM_SERVICE_STATUS_PROCESSW,
                        services_returned as usize,
                    );
                    for entry in slice {
                        let name = wide_ptr_to_string(entry.lpServiceName.0 as *const u16);
                        let display_name = wide_ptr_to_string(entry.lpDisplayName.0 as *const u16);

                        let status = match entry.ServiceStatusProcess.dwCurrentState.0 {
                            4 => "运行中",
                            1 => "已停止",
                            7 => "已暂停",
                            2 => "正在启动",
                            3 => "正在停止",
                            6 => "正在暂停",
                            5 => "正在恢复",
                            _ => "未知",
                        };

                        let mut start_type = "未知".to_string();
                        let wide_name: Vec<u16> = OsString::from(&name)
                            .encode_wide()
                            .chain(std::iter::once(0))
                            .collect();
                        if let Ok(service) = OpenServiceW(
                            scm,
                            windows::core::PCWSTR(wide_name.as_ptr()),
                            SERVICE_QUERY_CONFIG,
                        ) {
                            if !service.is_invalid() {
                                let mut config_buffer = vec![0u8; 1024];
                                let mut cfg_bytes_needed = 0u32;
                                let cfg_result = QueryServiceConfigW(
                                    service,
                                    Some(config_buffer.as_mut_ptr() as *mut QUERY_SERVICE_CONFIGW),
                                    config_buffer.len() as u32,
                                    &mut cfg_bytes_needed,
                                );
                                if cfg_result.is_err()
                                    && cfg_bytes_needed as usize > config_buffer.len()
                                {
                                    config_buffer.resize(cfg_bytes_needed as usize, 0);
                                    let _ = QueryServiceConfigW(
                                        service,
                                        Some(config_buffer.as_mut_ptr()
                                            as *mut QUERY_SERVICE_CONFIGW),
                                        config_buffer.len() as u32,
                                        &mut cfg_bytes_needed,
                                    );
                                }
                                if cfg_result.is_ok()
                                    || cfg_bytes_needed as usize <= config_buffer.len()
                                {
                                    let cfg =
                                        &*(config_buffer.as_ptr() as *const QUERY_SERVICE_CONFIGW);
                                    start_type = match cfg.dwStartType {
                                        windows::Win32::System::Services::SERVICE_AUTO_START => {
                                            "自动"
                                        }
                                        windows::Win32::System::Services::SERVICE_BOOT_START => {
                                            "引导"
                                        }
                                        windows::Win32::System::Services::SERVICE_DEMAND_START => {
                                            "手动"
                                        }
                                        windows::Win32::System::Services::SERVICE_DISABLED => {
                                            "已禁用"
                                        }
                                        windows::Win32::System::Services::SERVICE_SYSTEM_START => {
                                            "系统"
                                        }
                                        _ => "未知",
                                    }
                                    .to_string();
                                }
                                let _ = CloseServiceHandle(service);
                            }
                        }

                        services.push(SysServiceInfo {
                            name,
                            display_name,
                            status: status.to_string(),
                            start_type,
                        });
                    }
                }

                if !more_data {
                    break;
                }
                if bytes_needed as usize > buffer.len() {
                    buffer.resize(bytes_needed as usize, 0);
                }
            }

            let _ = CloseServiceHandle(scm);
        }

        services.sort_by(|a, b| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        });
        services
    }

    #[cfg(not(target_os = "windows"))]
    fn load_services(&self) -> Vec<SysServiceInfo> {
        Vec::new()
    }

    #[cfg(target_os = "windows")]
    fn load_startup_items(&self) -> Vec<SysStartupInfo> {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
        use windows::Win32::System::Registry::{
            RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
            KEY_READ, REG_EXPAND_SZ, REG_SZ,
        };

        let mut items = Vec::new();

        let registry_locations = [
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
                "HKLM\\Run",
            ),
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run",
                "HKLM\\Run (WOW6432Node)",
            ),
            (
                HKEY_LOCAL_MACHINE,
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
                "HKLM\\RunOnce",
            ),
            (
                HKEY_CURRENT_USER,
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
                "HKCU\\Run",
            ),
            (
                HKEY_CURRENT_USER,
                "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
                "HKCU\\RunOnce",
            ),
        ];

        unsafe {
            for (root, subkey, location_label) in registry_locations {
                let subkey_wide: Vec<u16> = OsString::from(subkey)
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect();
                let mut hkey = std::mem::zeroed();
                if RegOpenKeyExW(root, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut hkey).is_ok()
                {
                    let mut index = 0u32;
                    loop {
                        let mut name_buf = vec![0u16; 512];
                        let mut name_len = name_buf.len() as u32;
                        let mut data_buf = vec![0u8; 4096];
                        let mut data_len = data_buf.len() as u32;
                        let mut value_type = 0u32;

                        let result = RegEnumValueW(
                            hkey,
                            index,
                            PWSTR(name_buf.as_mut_ptr()),
                            &mut name_len,
                            None,
                            Some(&mut value_type),
                            Some(data_buf.as_mut_ptr()),
                            Some(&mut data_len),
                        );

                        if result == ERROR_NO_MORE_ITEMS {
                            break;
                        }
                        if result != ERROR_SUCCESS {
                            index += 1;
                            continue;
                        }

                        if value_type == REG_SZ.0 || value_type == REG_EXPAND_SZ.0 {
                            let command = wide_ptr_to_string(data_buf.as_ptr() as *const u16);
                            let command = if value_type == REG_EXPAND_SZ.0 {
                                expand_environment_strings(&command)
                            } else {
                                command
                            };
                            let name = wide_ptr_to_string(name_buf.as_ptr());
                            if !name.is_empty() && !command.is_empty() {
                                items.push(SysStartupInfo {
                                    name,
                                    command,
                                    location: location_label.to_string(),
                                    enabled: true,
                                });
                            }
                        }

                        index += 1;
                    }
                    let _ = RegCloseKey(hkey);
                }
            }
        }

        let startup_folders = [
            (
                std::env::var("APPDATA").unwrap_or_default()
                    + "\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
                "启动文件夹 (用户)",
            ),
            (
                std::env::var("ProgramData").unwrap_or_default()
                    + "\\Microsoft\\Windows\\Start Menu\\Programs\\Startup",
                "启动文件夹 (公共)",
            ),
        ];
        for (folder, location_label) in startup_folders {
            if let Ok(entries) = std::fs::read_dir(&folder) {
                for entry in entries.flatten() {
                    if let Ok(meta) = entry.metadata() {
                        if !meta.is_file() {
                            continue;
                        }
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if !name.to_lowercase().ends_with(".lnk") {
                            continue;
                        }
                        let command = entry.path().to_string_lossy().into_owned();
                        if !command.is_empty() {
                            items.push(SysStartupInfo {
                                name,
                                command,
                                location: location_label.to_string(),
                                enabled: true,
                            });
                        }
                    }
                }
            }
        }

        items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        items
    }

    #[cfg(not(target_os = "windows"))]
    fn load_startup_items(&self) -> Vec<SysStartupInfo> {
        Vec::new()
    }

    fn load_users(&self) -> Vec<SysUserInfo> {
        self.users
            .list()
            .iter()
            .map(|user| SysUserInfo {
                name: user.name().to_string(),
                uid: user.id().to_string(),
                gid: user.group_id().to_string(),
                groups: user
                    .groups()
                    .iter()
                    .map(|group| group.name().to_string())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
            .collect()
    }

    pub fn start_service(&self, name: &str) -> bool {
        self.control_service(name, true)
    }

    pub fn stop_service(&self, name: &str) -> bool {
        self.control_service(name, false)
    }

    #[cfg(target_os = "windows")]
    fn control_service(&self, name: &str, start: bool) -> bool {
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::System::Services::{
            CloseServiceHandle, ControlService, OpenSCManagerW, OpenServiceW, StartServiceW,
            SC_MANAGER_CONNECT, SERVICE_CONTROL_STOP, SERVICE_START, SERVICE_STOP,
        };

        unsafe {
            let scm = match OpenSCManagerW(None, None, SC_MANAGER_CONNECT) {
                Ok(h) if !h.is_invalid() => h,
                _ => return false,
            };

            let wide_name: Vec<u16> = OsString::from(name)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();
            let access = if start { SERVICE_START } else { SERVICE_STOP };
            let service = match OpenServiceW(scm, windows::core::PCWSTR(wide_name.as_ptr()), access)
            {
                Ok(h) if !h.is_invalid() => h,
                _ => {
                    let _ = CloseServiceHandle(scm);
                    return false;
                }
            };

            let ok = if start {
                StartServiceW(service, None).is_ok()
            } else {
                let mut status = Default::default();
                ControlService(service, SERVICE_CONTROL_STOP, &mut status).is_ok()
            };

            let _ = CloseServiceHandle(service);
            let _ = CloseServiceHandle(scm);
            ok
        }
    }

    #[cfg(not(target_os = "windows"))]
    fn control_service(&self, _name: &str, _start: bool) -> bool {
        false
    }

    pub fn load_system_info_as_encrypt_json(&mut self) -> String {
        self.load_system_info_as_json()
    }

    pub fn get_process_details(&self, pid: u32) -> Option<ProcessDetailInfo> {
        collect_process_details(pid, &self.system)
    }

    pub fn kill_process(&self, pid: u32) -> bool {
        self.system
            .processes()
            .get(&Pid::from_u32(pid))
            .map(|process| process.kill())
            .unwrap_or(false)
    }
}
