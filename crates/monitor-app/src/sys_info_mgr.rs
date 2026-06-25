use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use adlx::helper::AdlxHelper;
use anyhow::Result;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::Nvml;
use sysinfo::{
    Components, CpuRefreshKind, Disks, MemoryRefreshKind, Networks, Pid, ProcessRefreshKind,
    ProcessStatus, RefreshKind, System, UpdateKind, Users,
};

use crate::monitor_process_detail::{collect_process_details, ProcessDetailInfo};
use crate::sys_info::{
    SysComponentInfo, SysCpuInfo, SysDiskInfo, SysGpuInfo, SysInfo, SysIpNetwork, SysMemInfo,
    SysNetworkInfo, SysOsInfo, SysProcessInfo, SysServiceInfo, SysSingleCpuInfo, SysStartupInfo,
    SysUserInfo,
};

#[cfg(target_os = "windows")]
use crate::cpu_metrics_windows;

fn truncate_string(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!("{}...", text.chars().take(max_len).collect::<String>())
    }
}

/// Temporary profiling helper: prints elapsed time when dropped.
struct SectionTimer {
    label: &'static str,
    start: Instant,
}

impl SectionTimer {
    fn new(label: &'static str) -> Self {
        Self {
            label,
            start: Instant::now(),
        }
    }
}

impl Drop for SectionTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        eprintln!(
            "[cyber_monitor timing] {}: {}.{:03}s",
            self.label,
            elapsed.as_secs(),
            elapsed.subsec_millis()
        );
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
    /// Cached NVML handle to avoid re-loading the NVIDIA management library every second.
    nvml: Option<Nvml>,
    /// Cached ADLX helper to avoid re-initializing the AMD ADLX SDK every second.
    adlx_helper: Option<AdlxHelper>,
    /// Services/startup/users change rarely; cache them for a few seconds to avoid
    /// expensive WMI/registry/SCM scans on every tick.
    cached_services: Option<(Instant, Vec<SysServiceInfo>)>,
    cached_startup: Option<(Instant, Vec<SysStartupInfo>)>,
    cached_users: Option<(Instant, Vec<SysUserInfo>)>,
    cached_thread_count: Option<(Instant, u32)>,
    #[cfg(target_os = "windows")]
    gpu_process_collector: Option<crate::gpu_process_metrics_windows::GpuProcessMetricsCollector>,
}

impl SysInfoManager {
    /// How long to keep the slow service/startup/user scans cached.
    const SLOW_DATA_TTL: Duration = Duration::from_secs(30);
    /// Thread count can change quickly, but a few seconds of staleness is fine
    /// and avoids a ~60 ms Toolhelp32 snapshot every tick.
    const THREAD_COUNT_TTL: Duration = Duration::from_secs(5);
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
            nvml: None,
            adlx_helper: None,
            cached_services: None,
            cached_startup: None,
            cached_users: None,
            cached_thread_count: None,
            #[cfg(target_os = "windows")]
            gpu_process_collector: None,
        }
    }

    pub fn load_system_info(&mut self) -> SysInfo {
        let total_start = Instant::now();

        {
            let _t = SectionTimer::new("refresh system/networks/disks/components/users");
            {
                let _t = SectionTimer::new("  refresh system");
                // Use targeted refreshes instead of `refresh_all()` to avoid pulling in
                // environment/CWD/root/tasks data that the dashboard doesn't need.
                self.system.refresh_specifics(
                    RefreshKind::nothing()
                        .with_cpu(CpuRefreshKind::everything())
                        .with_memory(MemoryRefreshKind::everything())
                        .with_processes(
                            ProcessRefreshKind::nothing()
                                .with_cpu()
                                .with_memory()
                                .with_disk_usage()
                                .with_exe(UpdateKind::OnlyIfNotSet)
                                .with_cmd(UpdateKind::OnlyIfNotSet),
                        ),
                );
            }
            {
                let _t = SectionTimer::new("  refresh networks");
                self.networks.refresh(true);
            }
            {
                let _t = SectionTimer::new("  refresh disks");
                self.disks.refresh(true);
            }
            {
                let _t = SectionTimer::new("  refresh components");
                self.components.refresh(true);
            }
            {
                let _t = SectionTimer::new("  refresh users");
                self.users.refresh();
            }
        }

        let (cpu, cps) = {
            let _t = SectionTimer::new("build cpu info");
            let usage = self.system.global_cpu_usage();
            let vendor = self.system.cpus()[0].vendor_id().to_string();
            let brand = self.system.cpus()[0].brand().trim().to_string();
            let components: Vec<SysComponentInfo> = self
                .components
                .iter()
                .map(|component| SysComponentInfo {
                    temperature: component.temperature().unwrap_or(0.0),
                    max: component.max().unwrap_or(0.0),
                    critical: component.critical().unwrap_or(0.0),
                    label: component.label().to_string(),
                })
                .collect();
            let logical_cores = self.system.cpus().len().max(1);
            let physical_cores = sysinfo::System::physical_core_count()
                .unwrap_or(logical_cores / 2)
                .max(1);
            #[cfg(target_os = "windows")]
            let ohm_snapshot = {
                let _t = SectionTimer::new("cpu ohm snapshot");
                crate::cpu_ohm::read_cpu_hardware_snapshot(&brand, physical_cores, logical_cores)
            };
            #[cfg(not(target_os = "windows"))]
            let ohm_snapshot = None;
            let cpu_temperature = {
                let _t = SectionTimer::new("cpu temperature");
                load_cpu_temperature_celsius(
                    physical_cores,
                    logical_cores,
                    &components,
                    ohm_snapshot.as_ref(),
                )
            };
            let (base_frequency, current_frequency, max_frequency) = {
                let _t = SectionTimer::new("cpu frequencies");
                self.load_cpu_frequencies(&brand, ohm_snapshot)
            };
            let mut cpus = Vec::new();
            for cpu in self.system.cpus() {
                let single_cpu = SysSingleCpuInfo {
                    name: cpu.name().to_string(),
                    usage: cpu.cpu_usage(),
                };
                cpus.push(single_cpu);
            }
            let physical_cores = sysinfo::System::physical_core_count()
                .unwrap_or(cpus.len() / 2)
                .max(1);
            (
                SysCpuInfo {
                    usage,
                    vendor,
                    brand,
                    base_frequency,
                    current_frequency,
                    max_frequency,
                    cpus,
                    physical_cores,
                    temperature: cpu_temperature,
                    virtualization: crate::cpu_platform::read_cpu_virtualization_label(),
                    cache_summary: crate::cpu_platform::read_cpu_cache_label(),
                },
                components,
            )
        };

        let gb = 1024 * 1024 * 1024;
        let mem = {
            let _t = SectionTimer::new("build memory");
            let mut mem = SysMemInfo {
                total: self.system.total_memory(),
                total_gb: self.system.total_memory() / gb,
                used: self.system.used_memory(),
                used_gb: self.system.used_memory() / gb,
                available: self.system.available_memory(),
                available_gb: self.system.available_memory() / gb,
                ..Default::default()
            };
            #[cfg(target_os = "windows")]
            if let Some(ext) = {
                let _t = SectionTimer::new("windows memory metrics");
                crate::mem_metrics_windows::read_windows_mem_metrics()
            } {
                mem.committed = ext.committed;
                mem.commit_peak = ext.commit_peak;
                mem.commit_limit = ext.commit_limit;
                mem.system_cache = ext.system_cache;
                mem.kernel_ws = ext.kernel_ws;
                mem.kernel_paged = ext.kernel_paged;
                mem.kernel_nonpaged = ext.kernel_nonpaged;
                mem.hw_reserved = ext.hw_reserved;
                mem.swap_total = ext.swap_total;
                mem.swap_used = ext.swap_used;
                if ext.physical_used > 0 {
                    mem.used = ext.physical_used;
                    mem.used_gb = ext.physical_used / gb;
                }
            }
            mem
        };

        let disks_info = {
            let _t = SectionTimer::new("build disks");
            let mut disks_info = Vec::new();
            for disk in &mut self.disks {
                let usage = disk.usage();
                disks_info.push(SysDiskInfo {
                    disk_type: disk.kind().to_string(),
                    manufacturer: String::new(),
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
            #[cfg(target_os = "windows")]
            {
                let _t = SectionTimer::new("disk manufacturers");
                let mounts: Vec<String> = disks_info
                    .iter()
                    .map(|disk| disk.mount_on.clone())
                    .collect();
                let manufacturers = crate::disk_metrics_windows::read_disk_manufacturers(&mounts);
                for disk in &mut disks_info {
                    let device_id =
                        crate::disk_metrics_windows::normalize_device_id(&disk.mount_on);
                    if let Some(manufacturer) = manufacturers.get(&device_id) {
                        disk.manufacturer = manufacturer.clone();
                    }
                }
            }
            disks_info
        };

        let (networks, os, uptime) = {
            let _t = SectionTimer::new("build networks + os");
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
                sys_kernel_version: System::kernel_version()
                    .unwrap_or_else(|| "<unknown>".to_owned()),
                sys_os_version: System::os_version().unwrap_or_else(|| "<unknown>".to_owned()),
                sys_os_long_version: System::long_os_version()
                    .unwrap_or_else(|| "<unknown>".to_owned()),
                sys_host_name: System::host_name().unwrap_or_else(|| "<unknown>".to_owned()),
                sys_kernel: System::kernel_long_version().to_string(),
            };

            let up = System::uptime();
            let mut uptime = up;
            let days = uptime / 86400;
            uptime -= days * 86400;
            let hours = uptime / 3600;
            uptime -= hours * 3600;
            let minutes = uptime / 60;
            let uptime = format!("{days} days {hours} hours {minutes} minutes");

            (networks, os, uptime)
        };

        let (gpus, process_gpu_stats) = {
            let _t = SectionTimer::new("build gpu info + per-process gpu stats");
            let mut gpus = Vec::new();
            if self.nvml.is_none() {
                self.nvml = Nvml::init().ok();
            }
            if let Some(nvml) = self.nvml.as_ref() {
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

                        if let Ok(pci) = device.pci_info() {
                            gpu_info.pci_address =
                                crate::monitor_model::pci_address_from_bus_id(&pci.bus_id)
                                    .unwrap_or_else(|| {
                                        crate::monitor_model::format_pci_address(
                                            pci.bus, pci.device, 0,
                                        )
                                    });
                        }

                        let brand = device.name();
                        if let Ok(brand) = brand {
                            gpu_info.brand = brand.trim_matches('"').replace('"', "");
                        }

                        let fan = crate::gpu_nvml::read_nvml_fan(&device);
                        gpu_info.fan_speed = fan.rpm;
                        gpu_info.fan_rpm_valid = fan.rpm_valid;
                        gpu_info.fan_speed_percent = if fan.rpm_valid { 0 } else { fan.percent };
                        gpu_info.power_limit = device.enforced_power_limit().unwrap_or(0);

                        if let Ok(u) = device.encoder_utilization() {
                            gpu_info.encoder_utilization = u.utilization;
                        }

                        if let Ok(u) = device.decoder_utilization() {
                            gpu_info.decoder_utilization = u.utilization;
                        }

                        if let Ok(u) = device.utilization_rates() {
                            gpu_info.gpu_utilization = u.gpu;
                            gpu_info.mem_utilization = u.memory;
                        }

                        gpu_info.temperature =
                            device.temperature(TemperatureSensor::Gpu).unwrap_or(0);

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

            if gpus.is_empty() {
                if let Ok(amd_gpus) = self.load_amd_gpu_info() {
                    for amd_gpu_info in amd_gpus {
                        gpus.push(amd_gpu_info);
                    }
                }
            }

            #[cfg(target_os = "windows")]
            let gpu_names: Vec<String> = gpus
                .iter()
                .map(|gpu| {
                    if gpu.brand.is_empty() {
                        String::new()
                    } else {
                        gpu.brand.clone()
                    }
                })
                .collect();
            #[cfg(target_os = "windows")]
            let process_gpu_stats = self.sample_process_gpu_stats(&gpu_names);
            #[cfg(not(target_os = "windows"))]
            let process_gpu_stats = HashMap::new();

            (gpus, process_gpu_stats)
        };

        let now = Instant::now();
        let elapsed_secs = self
            .last_sample_time
            .map(|last| last.elapsed().as_secs_f64())
            .unwrap_or(0.0)
            .max(0.001);

        let mut handle_count = 0u64;
        let processes = {
            let _t = SectionTimer::new("iterate processes");
            let mut processes: Vec<SysProcessInfo> = self
                .system
                .processes()
                .values()
                .map(|process| {
                    if let Some(handles) = process.open_files() {
                        handle_count += handles as u64;
                    }
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

                    #[cfg(target_os = "windows")]
                    let (gpu_name, gpu_usage, gpu_dedicated_bytes) = process_gpu_stats
                        .get(&pid)
                        .map(|stats| {
                            (
                                stats.gpu_name.clone(),
                                stats.usage_percent,
                                stats.dedicated_bytes,
                            )
                        })
                        .unwrap_or_default();
                    #[cfg(not(target_os = "windows"))]
                    let (gpu_name, gpu_usage, gpu_dedicated_bytes) = (String::new(), 0.0f32, 0u64);

                    SysProcessInfo {
                        pid,
                        parent_pid: process.parent().map(|p| p.as_u32()).unwrap_or(0),
                        name: truncate_string(&name, 128),
                        command_line: if command_line.is_empty() {
                            exe.clone()
                        } else {
                            command_line
                        },
                        exe,
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
                        gpu_name,
                        gpu_usage,
                        gpu_dedicated_bytes,
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
            processes
        };

        let thread_count = {
            let _t = SectionTimer::new("count system threads");
            let now = Instant::now();
            if self.cached_thread_count.as_ref().map_or(true, |(t, _)| {
                now.duration_since(*t) >= Self::THREAD_COUNT_TTL
            }) {
                let count = count_system_threads(&self.system);
                self.cached_thread_count = Some((now, count));
                count
            } else {
                self.cached_thread_count.as_ref().unwrap().1
            }
        };

        let (services, startup_items, users) = {
            let _t = SectionTimer::new("load services + startup + users");
            let now = Instant::now();
            let needs_services = self
                .cached_services
                .as_ref()
                .map_or(true, |(t, _)| now.duration_since(*t) >= Self::SLOW_DATA_TTL);
            let needs_startup = self
                .cached_startup
                .as_ref()
                .map_or(true, |(t, _)| now.duration_since(*t) >= Self::SLOW_DATA_TTL);
            let needs_users = self
                .cached_users
                .as_ref()
                .map_or(true, |(t, _)| now.duration_since(*t) >= Self::SLOW_DATA_TTL);

            if !needs_services && !needs_startup && !needs_users {
                (
                    self.cached_services.as_ref().unwrap().1.clone(),
                    self.cached_startup.as_ref().unwrap().1.clone(),
                    self.cached_users.as_ref().unwrap().1.clone(),
                )
            } else {
                let mut services = self
                    .cached_services
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let mut startup_items = self
                    .cached_startup
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                let mut users = self
                    .cached_users
                    .as_ref()
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();

                {
                    let _t2 = SectionTimer::new("slow data cache miss parallel load");
                    thread::scope(|s| {
                        let services_handle = if needs_services {
                            Some(s.spawn(|| self.load_services()))
                        } else {
                            None
                        };
                        let startup_handle = if needs_startup {
                            Some(s.spawn(|| self.load_startup_items()))
                        } else {
                            None
                        };
                        let users_handle = if needs_users {
                            Some(s.spawn(|| self.load_users()))
                        } else {
                            None
                        };

                        if let Some(h) = services_handle {
                            services = h.join().unwrap();
                        }
                        if let Some(h) = startup_handle {
                            startup_items = h.join().unwrap();
                        }
                        if let Some(h) = users_handle {
                            users = h.join().unwrap();
                        }
                    });
                }

                if needs_services {
                    self.cached_services = Some((now, services.clone()));
                }
                if needs_startup {
                    self.cached_startup = Some((now, startup_items.clone()));
                }
                if needs_users {
                    self.cached_users = Some((now, users.clone()));
                }

                (services, startup_items, users)
            }
        };

        let total_elapsed = total_start.elapsed();
        eprintln!(
            "[cyber_monitor timing] load_system_info total: {}.{:03}s\n",
            total_elapsed.as_secs(),
            total_elapsed.subsec_millis()
        );

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
            thread_count,
            handle_count: handle_count.min(u32::MAX as u64) as u32,
            gpus,
            processes,
            services,
            startup_items,
            users,
        }
    }

    fn resolve_base_frequency_ghz(&mut self, brand: &str) -> f32 {
        if let Some(ghz) = parse_base_frequency_ghz(brand) {
            return ghz;
        }

        self.system.refresh_cpu_frequency();
        let sysinfo_base_mhz = self
            .system
            .cpus()
            .first()
            .map(|cpu| cpu.frequency() as f32)
            .unwrap_or(0.0);
        if sysinfo_base_mhz > 0.0 {
            return sysinfo_base_mhz / 1000.0;
        }

        if let Some(mhz) = read_cpuid_base_mhz() {
            if mhz > 0.0 {
                return mhz / 1000.0;
            }
        }

        #[cfg(target_os = "windows")]
        {
            if let Some((_, max)) = cpu_metrics_windows::read_processor_clocks_mhz() {
                if max > 0.0 {
                    return max / 1000.0;
                }
            }
            if let Some(mhz) = cpu_metrics_windows::read_registry_max_mhz() {
                if mhz > 0 {
                    return mhz as f32 / 1000.0;
                }
            }
        }

        0.0
    }

    fn load_cpu_frequencies(
        &mut self,
        brand: &str,
        ohm_snapshot: Option<crate::cpu_ohm::CpuHardwareSnapshot>,
    ) -> (f32, f32, f32) {
        let mut base_ghz = self.resolve_base_frequency_ghz(brand);

        if let Some(hw) = ohm_snapshot {
            if hw.current_frequency_ghz > 0.0 {
                if self.max_frequency <= 0.1 && hw.max_frequency_ghz > 0.0 {
                    self.max_frequency = hw.max_frequency_ghz;
                } else if hw.max_frequency_ghz > self.max_frequency {
                    self.max_frequency = hw.max_frequency_ghz;
                }
                let max = self.max_frequency.max(hw.max_frequency_ghz).max(base_ghz);
                self.max_frequency = max;
                return (base_ghz, hw.current_frequency_ghz, max);
            }
        }

        let cpu_count = self.system.cpus().len().max(1);

        let mut power_current_mhz = None::<f32>;
        let mut power_max_mhz = None::<f32>;

        #[cfg(target_os = "windows")]
        if let Some(infos) = read_processor_power_info(cpu_count) {
            if !infos.is_empty() {
                power_current_mhz = Some(
                    infos.iter().map(|info| info.CurrentMhz as f32).sum::<f32>()
                        / infos.len() as f32,
                );
                power_max_mhz = Some(
                    infos
                        .iter()
                        .map(|info| info.MaxMhz as f32)
                        .fold(0.0_f32, f32::max),
                );
            }
        }

        #[cfg(target_os = "windows")]
        let wmi_clocks = cpu_metrics_windows::read_processor_clocks_mhz();
        #[cfg(not(target_os = "windows"))]
        let wmi_clocks: Option<(f32, f32)> = None;

        #[cfg(target_os = "windows")]
        let registry_max_mhz = cpu_metrics_windows::read_registry_max_mhz().map(|mhz| mhz as f32);
        #[cfg(not(target_os = "windows"))]
        let registry_max_mhz = None::<f32>;

        self.system.refresh_cpu_frequency();
        let sysinfo_current_mhz = if self.system.cpus().is_empty() {
            0.0
        } else {
            self.system
                .cpus()
                .iter()
                .map(|cpu| cpu.frequency() as f32)
                .sum::<f32>()
                / self.system.cpus().len() as f32
        };

        let current_ghz = power_current_mhz
            .filter(|mhz| *mhz > 0.0)
            .or_else(|| {
                wmi_clocks
                    .map(|(current, _)| current)
                    .filter(|mhz| *mhz > 0.0)
            })
            .unwrap_or(sysinfo_current_mhz)
            / 1000.0;

        let candidate_max_mhz = [
            power_max_mhz,
            registry_max_mhz,
            wmi_clocks.map(|(_, max)| max),
        ]
        .into_iter()
        .flatten()
        .filter(|mhz| *mhz > 0.0)
        .fold(0.0_f32, f32::max);

        if base_ghz <= 0.0 {
            base_ghz = power_max_mhz
                .map(|mhz| mhz / 1000.0)
                .filter(|ghz| *ghz > 0.0)
                .unwrap_or(current_ghz);
        }

        if self.max_frequency <= 0.1 {
            if candidate_max_mhz > 0.0 {
                self.max_frequency = candidate_max_mhz / 1000.0;
            } else {
                self.max_frequency = base_ghz.max(current_ghz);
            }
        }

        #[cfg(target_os = "linux")]
        {
            if self.max_frequency <= 0.1 {
                if let Some(max_ghz) = read_linux_max_frequency_ghz(cpu_count) {
                    self.max_frequency = max_ghz;
                }
            }
        }

        let max_frequency = self.max_frequency.max(base_ghz);
        (base_ghz, current_ghz, max_frequency)
    }

    fn load_amd_gpu_info(&mut self) -> Result<Vec<SysGpuInfo>, anyhow::Error> {
        if self.adlx_helper.is_none() {
            self.adlx_helper = AdlxHelper::new().ok();
        }
        let helper = self
            .adlx_helper
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("ADLX not available"))?;
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
            if gpu_fan_speed > 100 {
                info.fan_speed = gpu_fan_speed as u32;
                info.fan_rpm_valid = true;
            } else if gpu_fan_speed > 0 {
                info.fan_speed_percent = gpu_fan_speed as u32;
            }
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

    #[cfg(target_os = "windows")]
    fn sample_process_gpu_stats(
        &mut self,
        gpu_names: &[String],
    ) -> std::collections::HashMap<u32, crate::gpu_process_metrics_windows::ProcessGpuUsage> {
        if self.gpu_process_collector.is_none() {
            self.gpu_process_collector =
                crate::gpu_process_metrics_windows::GpuProcessMetricsCollector::open();
        }
        self.gpu_process_collector
            .as_mut()
            .map(|collector| collector.sample(gpu_names))
            .unwrap_or_default()
    }
}

/// Owns `SysInfoManager` on a dedicated thread so WMI/COM probes and heavy sysinfo
/// refresh never block the GPUI main thread (STA COM apartment).
#[derive(Clone)]
pub struct SysInfoWorker {
    tx: std::sync::Arc<std::sync::mpsc::SyncSender<WorkerRequest>>,
}

enum WorkerRequest {
    Collect(std::sync::mpsc::SyncSender<SysInfo>),
    KillProcess {
        pid: u32,
        reply: std::sync::mpsc::SyncSender<bool>,
    },
    StartService {
        name: String,
        reply: std::sync::mpsc::SyncSender<bool>,
    },
    StopService {
        name: String,
        reply: std::sync::mpsc::SyncSender<bool>,
    },
    ProcessDetails {
        pid: u32,
        reply: std::sync::mpsc::SyncSender<Option<ProcessDetailInfo>>,
    },
}

struct WorkerState {
    manager: SysInfoManager,
    /// First snapshot produced during startup, before the UI requests data.
    prefetch: Option<SysInfo>,
}

impl WorkerState {
    fn collect(&mut self) -> SysInfo {
        if let Some(snapshot) = self.prefetch.take() {
            return snapshot;
        }
        self.manager.load_system_info()
    }
}

impl SysInfoWorker {
    /// Spawns the collector thread immediately (call before GPUI init to overlap startup work).
    pub fn start() -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<WorkerRequest>(8);
        let tx = std::sync::Arc::new(tx);
        std::thread::Builder::new()
            .name("cyber-monitor-sysinfo".into())
            .spawn({
                let rx = rx;
                move || {
                    let mut manager = SysInfoManager::new();
                    let mut state = WorkerState {
                        prefetch: Some(manager.load_system_info()),
                        manager,
                    };
                    while let Ok(request) = rx.recv() {
                        match request {
                            WorkerRequest::Collect(reply) => {
                                let _ = reply.send(state.collect());
                            }
                            WorkerRequest::KillProcess { pid, reply } => {
                                let _ = reply.send(state.manager.kill_process(pid));
                            }
                            WorkerRequest::StartService { name, reply } => {
                                let _ = reply.send(state.manager.start_service(&name));
                            }
                            WorkerRequest::StopService { name, reply } => {
                                let _ = reply.send(state.manager.stop_service(&name));
                            }
                            WorkerRequest::ProcessDetails { pid, reply } => {
                                let _ = reply.send(state.manager.get_process_details(pid));
                            }
                        }
                    }
                }
            })
            .expect("spawn cyber-monitor-sysinfo thread");
        Self { tx }
    }

    fn send_request<T, F>(&self, build: F) -> T
    where
        F: FnOnce(std::sync::mpsc::SyncSender<T>) -> WorkerRequest,
    {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        self.tx
            .send(build(reply_tx))
            .expect("sysinfo worker thread alive");
        reply_rx.recv().expect("sysinfo worker reply")
    }

    pub fn collect(&self) -> SysInfo {
        self.send_request(WorkerRequest::Collect)
    }

    pub fn kill_process(&self, pid: u32) -> bool {
        self.send_request(|reply| WorkerRequest::KillProcess { pid, reply })
    }

    pub fn start_service(&self, name: &str) -> bool {
        let name = name.to_string();
        self.send_request(|reply| WorkerRequest::StartService { name, reply })
    }

    pub fn stop_service(&self, name: &str) -> bool {
        let name = name.to_string();
        self.send_request(|reply| WorkerRequest::StopService { name, reply })
    }

    pub fn get_process_details(&self, pid: u32) -> Option<ProcessDetailInfo> {
        self.send_request(|reply| WorkerRequest::ProcessDetails { pid, reply })
    }
}

fn parse_base_frequency_ghz(brand: &str) -> Option<f32> {
    let at = brand.find('@')?;
    let tail = brand[at + 1..].trim_start();
    let digits: String = tail
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse().ok().filter(|ghz: &f32| *ghz > 0.0)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn read_cpuid_base_mhz() -> Option<f32> {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::__cpuid;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::__cpuid;

    let result = __cpuid(0x16);
    if result.eax > 0 {
        Some(result.eax as f32)
    } else {
        None
    }
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn read_cpuid_base_mhz() -> Option<f32> {
    None
}

fn load_cpu_temperature_celsius(
    physical_cores: usize,
    logical_cores: usize,
    components: &[SysComponentInfo],
    ohm_snapshot: Option<&crate::cpu_ohm::CpuHardwareSnapshot>,
) -> f32 {
    if let Some(hw) = ohm_snapshot {
        if hw.temperature_c > 0.0 {
            return hw.temperature_c;
        }
    }

    #[cfg(target_os = "windows")]
    if let Some(temp) = crate::cpu_ohm::read_cpu_temperature_celsius(physical_cores, logical_cores)
    {
        return temp;
    }

    #[cfg(target_os = "windows")]
    if let Some(temp) = cpu_metrics_windows::read_cpu_temperature_celsius() {
        return temp;
    }

    crate::monitor_model::cpu_package_temperature(components).unwrap_or(0.0)
}

#[cfg(target_os = "windows")]
fn read_processor_power_info(
    cpu_count: usize,
) -> Option<Vec<windows::Win32::System::Power::PROCESSOR_POWER_INFORMATION>> {
    use windows::Win32::System::Power::{
        CallNtPowerInformation, ProcessorInformation, PROCESSOR_POWER_INFORMATION,
    };

    if cpu_count == 0 {
        return None;
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
        Some(buffer)
    } else {
        None
    }
}

#[cfg(target_os = "linux")]
fn read_linux_max_frequency_ghz(cpu_count: usize) -> Option<f32> {
    let mut max_khz = 0u64;
    for index in 0..cpu_count {
        let path = format!("/sys/devices/system/cpu/cpu{index}/cpufreq/scaling_max_freq");
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        if let Ok(khz) = contents.trim().parse::<u64>() {
            max_khz = max_khz.max(khz);
        }
    }
    if max_khz > 0 {
        Some(max_khz as f32 / 1_000_000.0)
    } else {
        None
    }
}

#[cfg_attr(target_os = "windows", allow(unused_variables))]
fn count_system_threads(system: &System) -> u32 {
    #[cfg(target_os = "windows")]
    {
        count_system_threads_windows()
    }
    #[cfg(not(target_os = "windows"))]
    {
        system
            .processes()
            .values()
            .filter_map(|process| process.tasks())
            .map(|tasks| tasks.len() as u32)
            .sum()
    }
}

#[cfg(target_os = "windows")]
fn count_system_threads_windows() -> u32 {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
            return 0;
        };
        let mut count = 0u32;
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        if Thread32First(snapshot, &mut entry).is_ok() {
            count += 1;
            while Thread32Next(snapshot, &mut entry).is_ok() {
                count += 1;
            }
        }
        let _ = CloseHandle(snapshot);
        count
    }
}

#[cfg(test)]
mod frequency_tests {
    use super::parse_base_frequency_ghz;

    #[test]
    fn parse_base_frequency_from_intel_brand() {
        let parsed = parse_base_frequency_ghz("Intel(R) Core(TM) i9-10900 CPU @ 2.80GHz").unwrap();
        assert!((parsed - 2.80).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_base_frequency_missing_for_amd_brand() {
        assert!(parse_base_frequency_ghz("AMD Ryzen 9 5900X 12-Core Processor").is_none());
    }
}
