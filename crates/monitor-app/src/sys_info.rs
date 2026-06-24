use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysSingleCpuInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub usage: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysCpuInfo {
    #[serde(default)]
    pub usage: f32,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub base_frequency: f32,
    #[serde(default)]
    pub current_frequency: f32,
    #[serde(default)]
    pub max_frequency: f32,
    #[serde(default)]
    pub cpus: Vec<SysSingleCpuInfo>,
    /// Physical core count (unique `physical_core_id` values from sysinfo).
    #[serde(default)]
    pub physical_cores: usize,
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub virtualization: String,
    #[serde(default)]
    pub cache_summary: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysMemInfo {
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub total_gb: u64,
    #[serde(default)]
    pub used: u64,
    #[serde(default)]
    pub used_gb: u64,
    #[serde(default)]
    pub available: u64,
    #[serde(default)]
    pub available_gb: u64,
    /// Commit charge (virtual memory in use).
    #[serde(default)]
    pub committed: u64,
    #[serde(default)]
    pub commit_peak: u64,
    #[serde(default)]
    pub commit_limit: u64,
    #[serde(default)]
    pub system_cache: u64,
    #[serde(default)]
    pub kernel_ws: u64,
    #[serde(default)]
    pub kernel_paged: u64,
    #[serde(default)]
    pub kernel_nonpaged: u64,
    #[serde(default)]
    pub hw_reserved: u64,
    #[serde(default)]
    pub swap_total: u64,
    #[serde(default)]
    pub swap_used: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysDiskInfo {
    #[serde(default)]
    pub disk_type: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub mount_on: String,
    #[serde(default)]
    pub filesystem: String,
    #[serde(default)]
    pub available: u64,
    #[serde(default)]
    pub available_gb: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub total_gb: u64,
    #[serde(default)]
    pub read_bytes: u64,
    #[serde(default)]
    pub written_bytes: u64,
    #[serde(default)]
    pub read_rate: f64,
    #[serde(default)]
    pub write_rate: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysIpNetwork {
    #[serde(default)]
    pub addr: String,
    #[serde(default)]
    pub prefix: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysNetworkInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mac: String,
    #[serde(default)]
    pub ip_networks: Vec<SysIpNetwork>,
    #[serde(default)]
    pub received_data: u64,
    #[serde(default)]
    pub sent_data: u64,
    #[serde(default)]
    pub max_transmit_speed: u64,
    #[serde(default)]
    pub max_receive_speed: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysUserInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub gid: String,
    #[serde(default)]
    pub groups: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysServiceInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub start_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysStartupInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysProcessInfo {
    #[serde(default)]
    pub pid: u32,
    #[serde(default)]
    pub parent_pid: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub command_line: String,
    #[serde(default)]
    pub exe: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub cpu_usage: f32,
    #[serde(default)]
    pub memory: u64,
    #[serde(default)]
    pub memory_mb: u64,
    #[serde(default)]
    pub virtual_memory: u64,
    #[serde(default)]
    pub virtual_memory_mb: u64,
    #[serde(default)]
    pub disk_read_bytes: u64,
    #[serde(default)]
    pub disk_written_bytes: u64,
    #[serde(default)]
    pub disk_read_rate: f64,
    #[serde(default)]
    pub disk_write_rate: f64,
    /// Adapter label for the GPU this process is using (Windows PDH).
    #[serde(default)]
    pub gpu_name: String,
    /// GPU utilization percent for this process (Task Manager style, 0–100).
    #[serde(default)]
    pub gpu_usage: f32,
    /// Dedicated GPU memory in bytes (Windows PDH).
    #[serde(default)]
    pub gpu_dedicated_bytes: u64,
    #[serde(default)]
    pub start_time: u64,
    #[serde(default)]
    pub run_time: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysOsInfo {
    #[serde(default)]
    pub sys_name: String,
    #[serde(default)]
    pub sys_kernel_version: String,
    #[serde(default)]
    pub sys_os_version: String,
    #[serde(default)]
    pub sys_os_long_version: String,
    #[serde(default)]
    pub sys_host_name: String,
    #[serde(default)]
    pub sys_kernel: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysComponentInfo {
    #[serde(default)]
    pub temperature: f32,
    #[serde(default)]
    pub max: f32,
    #[serde(default)]
    pub critical: f32,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysGpuInfo {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub pci_address: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub fan_speed: u32,
    /// True when RPM was read successfully via NVML (includes 0 RPM).
    #[serde(default)]
    pub fan_rpm_valid: bool,
    /// Fan duty cycle 0–100 when RPM API is unavailable (AMD ADLX fallback).
    #[serde(default)]
    pub fan_speed_percent: u32,
    #[serde(default)]
    pub power_limit: u32,
    #[serde(default)]
    pub encoder_utilization: u32,
    #[serde(default)]
    pub decoder_utilization: u32,
    #[serde(default)]
    pub gpu_utilization: u32,
    #[serde(default)]
    pub mem_utilization: u32,
    #[serde(default)]
    pub temperature: u32,
    #[serde(default)]
    pub mem_free: u64,
    #[serde(default)]
    pub mem_free_gb: f32,
    #[serde(default)]
    pub mem_used: u64,
    #[serde(default)]
    pub mem_used_gb: f32,
    #[serde(default)]
    pub mem_total: u64,
    #[serde(default)]
    pub mem_total_gb: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct SysInfo {
    #[serde(default)]
    pub timestamp: i64,
    #[serde(default)]
    pub timestamp_readable: String,
    #[serde(default)]
    pub cpu: SysCpuInfo,
    #[serde(default)]
    pub mem: SysMemInfo,
    #[serde(default)]
    pub disks: Vec<SysDiskInfo>,
    #[serde(default)]
    pub networks: Vec<SysNetworkInfo>,
    #[serde(default)]
    pub os: SysOsInfo,
    #[serde(default)]
    pub components: Vec<SysComponentInfo>,
    #[serde(default)]
    pub uptime: String,
    #[serde(default)]
    pub thread_count: u32,
    #[serde(default)]
    pub handle_count: u32,
    #[serde(default)]
    pub gpus: Vec<SysGpuInfo>,
    #[serde(default)]
    pub processes: Vec<SysProcessInfo>,
    #[serde(default)]
    pub services: Vec<SysServiceInfo>,
    #[serde(default)]
    pub startup_items: Vec<SysStartupInfo>,
    #[serde(default)]
    pub users: Vec<SysUserInfo>,
}
