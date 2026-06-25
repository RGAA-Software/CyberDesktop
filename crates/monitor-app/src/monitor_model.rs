use std::collections::{BTreeMap, VecDeque};
use std::time::Instant;

use gpui::{rgb, Hsla};
use gpui_component::Theme;
use serde::Deserialize;

use crate::sys_info::{SysDiskInfo, SysGpuInfo, SysInfo, SysNetworkInfo, SysProcessInfo};

pub const MAX_POINTS: usize = 180;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum SortDirection {
    #[default]
    Desc,
    Asc,
}

impl SortDirection {
    pub fn toggle(self) -> Self {
        match self {
            Self::Desc => Self::Asc,
            Self::Asc => Self::Desc,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum ProcessSortColumn {
    Cpu,
    Memory,
    #[default]
    Name,
    DiskRead,
    DiskWrite,
    Gpu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct ProcessSort {
    pub column: ProcessSortColumn,
    pub direction: SortDirection,
}

impl Default for ProcessSort {
    fn default() -> Self {
        Self {
            column: ProcessSortColumn::Name,
            direction: SortDirection::Asc,
        }
    }
}

pub fn sort_processes(processes: &mut [SysProcessInfo], sort: ProcessSort) {
    processes.sort_by(|a, b| {
        let ord: std::cmp::Ordering = match sort.column {
            ProcessSortColumn::Cpu => a
                .cpu_usage
                .partial_cmp(&b.cpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal),
            ProcessSortColumn::Memory => a.memory.cmp(&b.memory),
            ProcessSortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            ProcessSortColumn::DiskRead => a.disk_read_bytes.cmp(&b.disk_read_bytes),
            ProcessSortColumn::DiskWrite => a.disk_written_bytes.cmp(&b.disk_written_bytes),
            ProcessSortColumn::Gpu => a
                .gpu_usage
                .partial_cmp(&b.gpu_usage)
                .unwrap_or(std::cmp::Ordering::Equal),
        };
        match sort.direction {
            SortDirection::Desc => ord.reverse(),
            SortDirection::Asc => ord,
        }
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
pub enum MonitorTab {
    #[default]
    Overview = 0,
    Cpu = 1,
    Memory = 2,
    Gpu = 3,
    Storage = 4,
    Network = 5,
    Processes = 6,
    Services = 7,
    Startup = 8,
    Users = 9,
}

impl MonitorTab {
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Self::Overview,
            1 => Self::Cpu,
            2 => Self::Memory,
            3 => Self::Gpu,
            4 => Self::Storage,
            5 => Self::Network,
            6 => Self::Processes,
            7 => Self::Services,
            8 => Self::Startup,
            9 => Self::Users,
            _ => Self::Overview,
        }
    }
}

#[derive(Clone, Default)]
pub struct HistoryPoint {
    pub time: String,
    pub cpu_usage: f64,
    pub cpu_frequency: f64,
    pub cpu_cores: Vec<f64>,
    pub mem_used_gb: f64,
    pub mem_usage_percent: f64,
    pub net_send_mb: f64,
    pub net_recv_mb: f64,
    pub disk_read_mb: f64,
    pub disk_write_mb: f64,
}

#[derive(Clone, Default)]
pub struct GpuHistoryPoint {
    pub time: String,
    pub usage: f64,
    pub temperature: f64,
    pub memory_used_gb: f64,
    pub decoder_usage: f64,
}

#[derive(Clone, Default)]
pub struct DiskHistoryPoint {
    pub time: String,
    pub read_mb: f64,
    pub write_mb: f64,
}

#[derive(Clone, Default)]
pub struct NetworkHistoryPoint {
    pub time: String,
    pub send_mb: f64,
    pub recv_mb: f64,
}

#[derive(Clone, Default)]
pub struct MachineTelemetry {
    pub current: SysInfo,
    pub history: VecDeque<HistoryPoint>,
    pub gpu_history: BTreeMap<String, VecDeque<GpuHistoryPoint>>,
    pub disk_history: BTreeMap<String, VecDeque<DiskHistoryPoint>>,
    pub network_history: BTreeMap<String, VecDeque<NetworkHistoryPoint>>,
    last_net_by_iface: BTreeMap<String, (u64, u64)>,
    last_disk_totals: BTreeMap<String, (u64, u64)>,
    last_sample_ready: bool,
    last_sample_timestamp_ms: i64,
}

impl MachineTelemetry {
    pub fn new(initial: SysInfo) -> Self {
        let mut this = Self {
            current: initial,
            history: VecDeque::with_capacity(MAX_POINTS),
            gpu_history: BTreeMap::new(),
            disk_history: BTreeMap::new(),
            network_history: BTreeMap::new(),
            last_net_by_iface: BTreeMap::new(),
            last_disk_totals: BTreeMap::new(),
            last_sample_ready: false,
            last_sample_timestamp_ms: 0,
        };
        this.record_sample();
        this
    }

    pub fn apply_snapshot(&mut self, snapshot: SysInfo) {
        self.current = snapshot;
        self.record_sample();
    }

    pub fn latest_cpu_percent(&self) -> f32 {
        self.current.cpu.usage
    }

    pub fn latest_mem_percent(&self) -> f32 {
        let total = self.current.mem.total as f64;
        let used = self.current.mem.used as f64;
        if total <= 0.0 {
            0.0
        } else {
            (used / total * 100.0) as f32
        }
    }

    pub fn highest_disk_percent(&self) -> f32 {
        self.current
            .disks
            .iter()
            .map(disk_usage_percent)
            .fold(0.0, f32::max)
    }

    pub fn system_disk_percent(&self) -> f32 {
        system_disk(&self.current.disks)
            .map(disk_usage_percent)
            .unwrap_or(0.0)
    }

    pub fn system_disk_label(&self) -> String {
        system_disk(&self.current.disks)
            .map(format_system_disk_usage)
            .unwrap_or_else(|| "系统盘不可用".to_string())
    }

    pub fn primary_gpu_percent(&self) -> f32 {
        self.current
            .gpus
            .first()
            .map(|gpu| gpu.gpu_utilization as f32)
            .unwrap_or(0.0)
    }

    pub fn latest_send_rate(&self) -> f64 {
        self.history
            .back()
            .map(|point| point.net_send_mb)
            .unwrap_or(0.0)
    }

    pub fn latest_recv_rate(&self) -> f64 {
        self.history
            .back()
            .map(|point| point.net_recv_mb)
            .unwrap_or(0.0)
    }

    pub fn latest_disk_read_rate(&self) -> f64 {
        self.history
            .back()
            .map(|point| point.disk_read_mb)
            .unwrap_or(0.0)
    }

    pub fn latest_disk_write_rate(&self) -> f64 {
        self.history
            .back()
            .map(|point| point.disk_write_mb)
            .unwrap_or(0.0)
    }

    fn record_sample(&mut self) {
        let total_mem = self.current.mem.total as f64;
        let used_mem = self.current.mem.used as f64;
        let mem_usage_percent = if total_mem > 0.0 {
            (used_mem / total_mem * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };

        // Compute the actual elapsed time since the last sample. If the interval is
        // unreasonable (negative, zero, or no previous sample), treat it as no delta.
        let delta_seconds = if self.last_sample_ready && self.last_sample_timestamp_ms > 0 {
            ((self.current.timestamp - self.last_sample_timestamp_ms) as f64 / 1000.0).max(0.0)
        } else {
            0.0
        };

        // Compute per-interface network deltas, discarding any value that exceeds the
        // interface's link speed over the elapsed interval. This prevents counter
        // wraparounds, driver resets, or sysinfo glitches from producing huge spikes.
        let mut net_send_mb = 0.0;
        let mut net_recv_mb = 0.0;
        let mut iface_deltas: BTreeMap<String, (f64, f64)> = BTreeMap::new();

        for network in &self.current.networks {
            let id = network_key(network);
            let (send_mb, recv_mb) = if self.last_sample_ready && delta_seconds > 0.0 {
                match self.last_net_by_iface.get(&id) {
                    Some((last_sent, last_recv)) => {
                        let delta_sent = network.sent_data.saturating_sub(*last_sent);
                        let delta_recv = network.received_data.saturating_sub(*last_recv);

                        // Maximum bytes the interface could have transferred in this interval.
                        let max_sent =
                            (network.max_transmit_speed as f64 / 8.0 * delta_seconds) as u64;
                        let max_recv =
                            (network.max_receive_speed as f64 / 8.0 * delta_seconds) as u64;

                        // Only validate when the interface reports a link speed. If the speed
                        // is unknown (0), keep the raw delta: we have no basis to reject it.
                        let valid_sent = if network.max_transmit_speed > 0 && delta_sent > max_sent
                        {
                            0
                        } else {
                            delta_sent
                        };
                        let valid_recv = if network.max_receive_speed > 0 && delta_recv > max_recv {
                            0
                        } else {
                            delta_recv
                        };

                        (bytes_to_mb(valid_sent), bytes_to_mb(valid_recv))
                    }
                    None => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };

            self.last_net_by_iface
                .insert(id.clone(), (network.sent_data, network.received_data));
            iface_deltas.insert(id.clone(), (send_mb, recv_mb));
            net_send_mb += send_mb;
            net_recv_mb += recv_mb;
        }

        // Drop interfaces that no longer exist so stale totals don't affect future deltas.
        self.last_net_by_iface
            .retain(|id, _| self.current.networks.iter().any(|n| network_key(n) == *id));
        self.last_sample_ready = true;
        self.last_sample_timestamp_ms = self.current.timestamp;

        let (disk_read_bytes, disk_write_bytes) =
            self.current
                .processes
                .iter()
                .fold((0u64, 0u64), |(read, write), process| {
                    (
                        read + process.disk_read_bytes,
                        write + process.disk_written_bytes,
                    )
                });

        let point = HistoryPoint {
            time: short_time_label(&self.current.timestamp_readable),
            cpu_usage: self.current.cpu.usage as f64,
            cpu_frequency: self.current.cpu.current_frequency as f64,
            cpu_cores: self
                .current
                .cpu
                .cpus
                .iter()
                .map(|cpu| cpu.usage as f64)
                .collect(),
            mem_used_gb: bytes_to_gb(self.current.mem.used),
            mem_usage_percent,
            net_send_mb,
            net_recv_mb,
            disk_read_mb: bytes_to_mb(disk_read_bytes),
            disk_write_mb: bytes_to_mb(disk_write_bytes),
        };
        push_point(&mut self.history, point);

        for gpu in &self.current.gpus {
            let id = gpu_key(gpu);
            let point = GpuHistoryPoint {
                time: short_time_label(&self.current.timestamp_readable),
                usage: gpu.gpu_utilization as f64,
                temperature: gpu.temperature as f64,
                memory_used_gb: gpu.mem_used_gb as f64,
                decoder_usage: gpu.decoder_utilization as f64,
            };
            let history = self.gpu_history.entry(id).or_default();
            push_point(history, point);
        }

        let sample_time = short_time_label(&self.current.timestamp_readable);
        for disk in &self.current.disks {
            let id = disk_key(disk);
            let (read_mb, write_mb) = if self.last_sample_ready {
                match self.last_disk_totals.get(&id) {
                    Some((last_read, last_write)) => (
                        bytes_to_mb(disk.read_bytes.saturating_sub(*last_read)),
                        bytes_to_mb(disk.written_bytes.saturating_sub(*last_write)),
                    ),
                    None => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };
            self.last_disk_totals
                .insert(id.clone(), (disk.read_bytes, disk.written_bytes));
            let point = DiskHistoryPoint {
                time: sample_time.clone(),
                read_mb,
                write_mb,
            };
            push_point(self.disk_history.entry(id).or_default(), point);
        }

        for network in &self.current.networks {
            let id = network_key(network);
            let (send_mb, recv_mb) = iface_deltas.get(&id).copied().unwrap_or((0.0, 0.0));
            let point = NetworkHistoryPoint {
                time: sample_time.clone(),
                send_mb,
                recv_mb,
            };
            push_point(self.network_history.entry(id).or_default(), point);
        }
    }
}

#[derive(Clone)]
pub struct RemoteMachineState {
    pub machine_id: String,
    pub display_name: String,
    pub peer_ip: String,
    pub peer_addr: String,
    pub active_peer_addr: String,
    pub last_seen: String,
    pub last_seen_at: Instant,
    pub connected: bool,
    pub telemetry: MachineTelemetry,
}

pub fn push_point<T>(queue: &mut VecDeque<T>, point: T) {
    if queue.len() >= MAX_POINTS {
        queue.pop_front();
    }
    queue.push_back(point);
}

pub fn bytes_to_gb(value: u64) -> f64 {
    value as f64 / 1024.0 / 1024.0 / 1024.0
}

pub fn format_mem_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / 1024.0 / 1024.0 / 1024.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

pub fn format_gpu_fan_speed(gpu: &crate::sys_info::SysGpuInfo) -> String {
    if gpu.fan_rpm_valid {
        format!("{} RPM", gpu.fan_speed)
    } else if gpu.fan_speed_percent > 0 {
        format!("{}%", gpu.fan_speed_percent)
    } else {
        "—".to_string()
    }
}

pub fn gpu_fan_meter_percent(gpu: &crate::sys_info::SysGpuInfo) -> Option<f32> {
    if gpu.fan_rpm_valid {
        if gpu.fan_speed == 0 {
            None
        } else {
            Some(((gpu.fan_speed as f32 / 5000.0) * 100.0).clamp(0.0, 100.0))
        }
    } else if gpu.fan_speed_percent > 0 {
        Some(gpu.fan_speed_percent as f32)
    } else {
        None
    }
}

pub fn bytes_to_mb(value: u64) -> f64 {
    value as f64 / 1024.0 / 1024.0
}

pub fn short_time_label(raw: &str) -> String {
    raw.rsplit_once(' ')
        .map(|(_, time)| time.to_string())
        .unwrap_or_else(|| raw.to_string())
}

pub fn disk_usage_percent(disk: &SysDiskInfo) -> f32 {
    if disk.total == 0 {
        0.0
    } else {
        ((disk.total.saturating_sub(disk.available)) as f64 / disk.total as f64 * 100.0) as f32
    }
}

/// Returns the OS system volume from mounted disks (Windows: `SystemDrive`, Linux: `/`).
pub fn system_disk<'a>(disks: &'a [SysDiskInfo]) -> Option<&'a SysDiskInfo> {
    let prefix = system_drive_prefix();
    disks
        .iter()
        .find(|disk| disk_matches_system_drive(&disk.mount_on, &prefix))
        .or_else(|| disks.first())
}

fn system_drive_prefix() -> String {
    #[cfg(windows)]
    {
        std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string())
    }
    #[cfg(not(windows))]
    {
        "/".to_string()
    }
}

fn disk_matches_system_drive(mount_on: &str, system_prefix: &str) -> bool {
    let mount = mount_on.trim_end_matches(['\\', '/']).to_lowercase();
    #[cfg(windows)]
    {
        let prefix = system_prefix.trim_end_matches(['\\', '/']).to_lowercase();
        mount == prefix || mount.starts_with(&format!("{prefix}\\"))
    }
    #[cfg(not(windows))]
    {
        mount == system_prefix.trim_end_matches('/')
            || mount.starts_with(system_prefix)
            || system_prefix == "/" && (mount.is_empty() || mount == "/")
    }
}

pub fn format_system_disk_usage(disk: &SysDiskInfo) -> String {
    let used_gb = bytes_to_gb(disk.total.saturating_sub(disk.available));
    let total_gb = bytes_to_gb(disk.total);
    let mount = disk.mount_on.trim_end_matches(['\\', '/']).to_string();
    let label = if mount.is_empty() {
        "系统盘".to_string()
    } else {
        mount
    };
    format!("{label} {used_gb:.1} / {total_gb:.1} GB")
}

/// CPU charts and utilization progress bars (#05DF72).
pub fn cpu_metric_color() -> Hsla {
    rgb(0x05DF72).into()
}

/// Memory charts and utilization progress bars (#21BCFF).
pub fn mem_metric_color() -> Hsla {
    rgb(0x21BCFF).into()
}

/// Distinct chart colors per GPU index (avoids CPU green and memory cyan).
pub fn gpu_chart_color(index: usize) -> Hsla {
    const COLORS: [u32; 8] = [
        0xA855F7, 0xF97316, 0xF472B6, 0xEAB308, 0x8B5CF6, 0xEC4899, 0x6366F1, 0x14B8A6,
    ];
    rgb(COLORS[index % COLORS.len()]).into()
}

/// Stable per-GPU color based on position in the current GPU list.
pub fn gpu_color_for(gpu: &SysGpuInfo, gpus: &[SysGpuInfo]) -> Hsla {
    let key = gpu_key(gpu);
    let index = gpus
        .iter()
        .position(|item| gpu_key(item) == key)
        .unwrap_or(0);
    gpu_chart_color(index)
}

/// PCI BDF in decimal — matches Windows Task Manager (bus:device:function).
/// NVML/lspci use hex internally; we convert for display. Example: `PCI: 1:0:0`
pub fn format_pci_address(bus: u32, device: u32, function: u32) -> String {
    format!("PCI: {bus}:{device}:{function}")
}

/// Parses NVML-style bus id (hex) such as `00000000:01:00.0`, returns decimal display.
pub fn pci_address_from_bus_id(bus_id: &str) -> Option<String> {
    let s = bus_id.trim();
    if s.is_empty() {
        return None;
    }
    let (head, function) = s.rsplit_once('.').unwrap_or((s, "0"));
    let function = u32::from_str_radix(function, 16).ok()?;
    let parts: Vec<&str> = head.split(':').collect();
    let (bus, device) = match parts.len() {
        0 | 1 => return None,
        2 => (parts[0], parts[1]),
        _ => (parts[parts.len() - 2], parts[parts.len() - 1]),
    };
    let bus = u32::from_str_radix(bus, 16).ok()?;
    let device = u32::from_str_radix(device, 16).ok()?;
    Some(format_pci_address(bus, device, function))
}

pub fn gpu_key(gpu: &SysGpuInfo) -> String {
    if !gpu.id.is_empty() {
        gpu.id.clone()
    } else if !gpu.pci_address.is_empty() {
        gpu.pci_address.clone()
    } else {
        gpu.brand.clone()
    }
}

pub fn disk_key(disk: &SysDiskInfo) -> String {
    if disk.mount_on.is_empty() {
        format!("{}-{}", disk.disk_type, disk.filesystem)
    } else {
        disk.mount_on.clone()
    }
}

pub fn network_key(network: &SysNetworkInfo) -> String {
    network.name.clone()
}

pub fn disk_used_gb(disk: &SysDiskInfo) -> f64 {
    bytes_to_gb(disk.total.saturating_sub(disk.available))
}

fn format_link_speed_bps(bps: u64) -> String {
    if bps == 0 {
        return "—".to_string();
    }

    const G: f64 = 1_000_000_000.0;
    const M: f64 = 1_000_000.0;
    const K: f64 = 1_000.0;

    let bps_f = bps as f64;
    if bps_f >= G {
        format_link_speed_unit(bps_f / G, "Gbps")
    } else if bps_f >= M {
        format_link_speed_unit(bps_f / M, "Mbps")
    } else if bps_f >= K {
        format_link_speed_unit(bps_f / K, "Kbps")
    } else {
        format!("{bps} bps")
    }
}

fn format_link_speed_unit(value: f64, unit: &str) -> String {
    if (value - value.round()).abs() < 0.05 {
        format!("{:.0} {unit}", value.round())
    } else {
        format!("{:.1} {unit}", value)
    }
}

pub fn format_network_link_speed(network: &SysNetworkInfo) -> String {
    let tx = network.max_transmit_speed;
    let rx = network.max_receive_speed;
    if tx == 0 && rx == 0 {
        return "—".to_string();
    }
    if tx > 0 && (rx == 0 || tx == rx) {
        format_link_speed_bps(tx)
    } else if rx > 0 && tx == 0 {
        format_link_speed_bps(rx)
    } else {
        format!(
            "↑ {} / ↓ {}",
            format_link_speed_bps(tx),
            format_link_speed_bps(rx)
        )
    }
}

pub fn latest_disk_rates(telemetry: &MachineTelemetry, disk_id: &str) -> (f64, f64) {
    telemetry
        .disk_history
        .get(disk_id)
        .and_then(|history| history.back())
        .map(|point| (point.read_mb, point.write_mb))
        .unwrap_or((0.0, 0.0))
}

pub fn latest_network_rates(telemetry: &MachineTelemetry, network_id: &str) -> (f64, f64) {
    telemetry
        .network_history
        .get(network_id)
        .and_then(|history| history.back())
        .map(|point| (point.send_mb, point.recv_mb))
        .unwrap_or((0.0, 0.0))
}

/// Strips vendor prefix from GPU marketing name for compact display.
pub fn gpu_display_model(brand: &str) -> String {
    let trimmed = brand.trim();
    let lower = trimmed.to_lowercase();
    for prefix in ["nvidia ", "amd ", "intel ", "apple "] {
        if lower.starts_with(prefix) {
            return trimmed[prefix.len()..].trim().to_string();
        }
    }
    trimmed.to_string()
}

pub fn gpu_chart_title(kind: &str, gpu: &SysGpuInfo) -> String {
    let name = gpu.brand.trim();
    if gpu.pci_address.is_empty() {
        format!("{kind} ({name})")
    } else {
        format!("{kind} ({name} · {})", gpu.pci_address)
    }
}

pub fn gpu_memory_percent(gpu: &SysGpuInfo) -> f32 {
    if gpu.mem_total == 0 {
        0.0
    } else {
        (gpu.mem_used as f64 / gpu.mem_total as f64 * 100.0) as f32
    }
}

pub fn network_ipv4(network: &SysNetworkInfo) -> String {
    network
        .ip_networks
        .iter()
        .map(|ip| format!("{}/{}", ip.addr, ip.prefix))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn format_optional_frequency(freq: f32) -> String {
    if freq <= 0.0 {
        "暂不可用".to_string()
    } else {
        format!("{freq:.2} GHz")
    }
}

/// Reads the best available CPU package temperature from hardware sensors.
pub fn cpu_package_temperature(components: &[crate::sys_info::SysComponentInfo]) -> Option<f32> {
    let mut package_temp = None::<f32>;
    for component in components {
        let label = component.label.to_lowercase();
        let is_cpu_sensor = label.contains("cpu")
            || label.contains("core")
            || label.contains("package")
            || label.contains("tctl")
            || label.contains("ccd");
        if is_cpu_sensor && component.temperature > 0.0 {
            package_temp = Some(
                package_temp
                    .map(|current| current.max(component.temperature))
                    .unwrap_or(component.temperature),
            );
        }
    }
    if package_temp.is_some() {
        return package_temp;
    }

    components
        .iter()
        .filter(|component| {
            let label = component.label.to_lowercase();
            component.temperature > 0.0
                && !label.contains("gpu")
                && !label.contains("nvidia")
                && !label.contains("radeon")
                && !label.contains("disk")
                && !label.contains("hdd")
                && !label.contains("ssd")
        })
        .map(|component| component.temperature)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
}

pub fn format_cpu_temperature(
    cpu: &crate::sys_info::SysCpuInfo,
    components: &[crate::sys_info::SysComponentInfo],
) -> String {
    if cpu.temperature > 0.0 {
        return format!("{:.0} °C", cpu.temperature);
    }
    match cpu_package_temperature(components) {
        Some(temp) if temp > 0.0 => format!("{temp:.0} °C"),
        _ => "暂不可用".to_string(),
    }
}

pub fn cpu_color(cpu: f32, theme: &Theme) -> Hsla {
    if cpu >= 80.0 {
        theme.red
    } else if cpu >= 45.0 {
        theme.yellow
    } else {
        theme.green
    }
}

pub fn chart_ticks(max_value: f64) -> [f64; 5] {
    let top = if max_value <= 0.0 { 1.0 } else { max_value };
    [top, top * 0.75, top * 0.50, top * 0.25, 0.0]
}

pub fn format_tick(value: f64, unit: &str) -> String {
    match unit {
        "%" => format!("{value:.0}%"),
        "MB/s" => format!("{value:.1}"),
        "GB" => format!("{value:.1}"),
        "°C" => format!("{value:.0}"),
        "GHz" => format!("{value:.2}"),
        _ => format!("{value:.1}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys_info::{SysCpuInfo, SysGpuInfo, SysInfo, SysNetworkInfo, SysSingleCpuInfo};

    #[test]
    fn gpu_display_model_strips_nvidia_prefix() {
        assert_eq!(
            gpu_display_model("NVIDIA GeForce RTX 4070 Ti"),
            "GeForce RTX 4070 Ti"
        );
    }

    #[test]
    fn format_pci_address_uses_decimal_for_windows() {
        assert_eq!(format_pci_address(1, 0, 0), "PCI: 1:0:0");
        assert_eq!(format_pci_address(10, 31, 0), "PCI: 10:31:0");
    }

    #[test]
    fn pci_address_from_bus_id_converts_hex_nvml_to_decimal() {
        assert_eq!(
            pci_address_from_bus_id("00000000:01:00.0").as_deref(),
            Some("PCI: 1:0:0")
        );
        assert_eq!(
            pci_address_from_bus_id("01:00.0").as_deref(),
            Some("PCI: 1:0:0")
        );
        assert_eq!(
            pci_address_from_bus_id("00000000:0A:1F.0").as_deref(),
            Some("PCI: 10:31:0")
        );
    }

    #[test]
    fn gpu_chart_title_includes_pci_address() {
        let gpu = SysGpuInfo {
            brand: "NVIDIA GeForce RTX 4070 Ti".to_string(),
            pci_address: "PCI: 1:0:0".to_string(),
            ..Default::default()
        };
        assert_eq!(
            gpu_chart_title("GPU 使用率", &gpu),
            "GPU 使用率 (NVIDIA GeForce RTX 4070 Ti · PCI: 1:0:0)"
        );
    }

    #[test]
    fn format_gpu_fan_speed_prefers_rpm_including_zero() {
        let gpu = SysGpuInfo {
            fan_speed: 0,
            fan_rpm_valid: true,
            fan_speed_percent: 100,
            ..Default::default()
        };
        assert_eq!(format_gpu_fan_speed(&gpu), "0 RPM");
        assert_eq!(gpu_fan_meter_percent(&gpu), None);
    }

    #[test]
    fn format_gpu_fan_speed_shows_rpm_when_available() {
        let gpu = SysGpuInfo {
            fan_speed: 1650,
            fan_rpm_valid: true,
            ..Default::default()
        };
        assert_eq!(format_gpu_fan_speed(&gpu), "1650 RPM");
    }

    #[test]
    fn format_gpu_fan_speed_uses_percent_only_without_rpm_api() {
        let gpu = SysGpuInfo {
            fan_speed_percent: 42,
            ..Default::default()
        };
        assert_eq!(format_gpu_fan_speed(&gpu), "42%");
    }

    #[test]
    fn gpu_chart_color_cycles_at_sixteen() {
        let c0 = gpu_chart_color(0);
        let c8 = gpu_chart_color(8);
        assert_eq!(c0, c8);
    }

    #[test]
    fn system_disk_prefers_os_system_volume() {
        let disks = vec![
            SysDiskInfo {
                mount_on: "D:\\".to_string(),
                total: 1000,
                available: 500,
                ..Default::default()
            },
            SysDiskInfo {
                mount_on: "C:\\".to_string(),
                total: 2000,
                available: 1000,
                ..Default::default()
            },
            SysDiskInfo {
                mount_on: "/".to_string(),
                total: 3000,
                available: 1500,
                ..Default::default()
            },
        ];
        let disk = system_disk(&disks).unwrap();
        #[cfg(windows)]
        {
            assert_eq!(disk.mount_on, "C:\\");
            assert!(format_system_disk_usage(disk).starts_with("C:"));
        }
        #[cfg(not(windows))]
        {
            assert_eq!(disk.mount_on, "/");
            assert!(format_system_disk_usage(disk).starts_with('/'));
        }
        assert!((disk_usage_percent(disk) - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_history_records_cpu_cores() {
        let mut info = SysInfo::default();
        info.cpu = SysCpuInfo {
            cpus: vec![
                SysSingleCpuInfo {
                    name: "Core 0".to_string(),
                    usage: 12.5,
                },
                SysSingleCpuInfo {
                    name: "Core 1".to_string(),
                    usage: 34.0,
                },
            ],
            ..Default::default()
        };
        let telemetry = MachineTelemetry::new(info);
        let last = telemetry
            .history
            .back()
            .expect("history should have a point");
        assert_eq!(last.cpu_cores.len(), 2);
        assert_eq!(last.cpu_cores[0], 12.5);
        assert_eq!(last.cpu_cores[1], 34.0);
    }

    #[test]
    fn test_monitor_tab_from_index_split() {
        assert_eq!(MonitorTab::from_index(0), MonitorTab::Overview);
        assert_eq!(MonitorTab::from_index(1), MonitorTab::Cpu);
        assert_eq!(MonitorTab::from_index(2), MonitorTab::Memory);
        assert_eq!(MonitorTab::from_index(3), MonitorTab::Gpu);
        assert_eq!(MonitorTab::from_index(9), MonitorTab::Users);
    }

    #[test]
    fn format_link_speed_bps_uses_readable_units() {
        assert_eq!(format_link_speed_bps(1_000_000_000), "1 Gbps");
        assert_eq!(format_link_speed_bps(100_000_000), "100 Mbps");
        assert_eq!(format_link_speed_bps(2_500_000_000), "2.5 Gbps");
        assert_eq!(format_link_speed_bps(1_000_000), "1 Mbps");
    }

    #[test]
    fn format_network_link_speed_collapses_symmetric_links() {
        let network = SysNetworkInfo {
            max_transmit_speed: 1_000_000_000,
            max_receive_speed: 1_000_000_000,
            ..Default::default()
        };
        assert_eq!(format_network_link_speed(&network), "1 Gbps");
    }

    #[test]
    fn format_network_link_speed_shows_asymmetric_links() {
        let network = SysNetworkInfo {
            max_transmit_speed: 1_000_000_000,
            max_receive_speed: 100_000_000,
            ..Default::default()
        };
        assert_eq!(format_network_link_speed(&network), "↑ 1 Gbps / ↓ 100 Mbps");
    }
}
