use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use chrono::NaiveDateTime;

use crate::monitor_model::{MachineTelemetry, RemoteMachineState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    Warning,
    Critical,
}

impl AlertLevel {
    pub fn label(&self) -> &'static str {
        match self {
            AlertLevel::Warning => "警告",
            AlertLevel::Critical => "严重",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub id: String,
    pub machine_id: String,
    pub display_name: String,
    pub level: AlertLevel,
    pub metric: String,
    pub value: f64,
    pub threshold: f64,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Default)]
pub struct AggregatedProcess {
    pub machine_id: String,
    pub display_name: String,
    pub pid: u32,
    pub name: String,
    pub cpu_usage: f32,
    pub memory_mb: u64,
}

#[derive(Debug, Clone, Default)]
pub struct HostSummary {
    pub online_count: usize,
    pub offline_count: usize,
    pub total_machines: usize,
    pub top_processes: Vec<AggregatedProcess>,
    pub alerts: Vec<Alert>,
}

const CPU_CRITICAL_THRESHOLD: f64 = 90.0;
const CPU_WARNING_THRESHOLD: f64 = 80.0;
const MEMORY_CRITICAL_THRESHOLD: f64 = 90.0;
const MEMORY_WARNING_THRESHOLD: f64 = 80.0;
const DISK_CRITICAL_THRESHOLD: f64 = 90.0;
const DISK_WARNING_THRESHOLD: f64 = 80.0;
const MAX_ALERTS: usize = 100;
const DEFAULT_SUPPRESSION_INTERVAL: Duration = Duration::from_secs(300);

#[derive(Debug, Default)]
pub struct AlertSuppressor {
    last_triggered: BTreeMap<(String, String), Instant>,
}

impl AlertSuppressor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn may_trigger(&mut self, machine_id: &str, metric: &str, min_interval: Duration) -> bool {
        let key = (machine_id.to_string(), metric.to_string());
        let now = Instant::now();
        match self.last_triggered.get(&key) {
            Some(last) if now.duration_since(*last) < min_interval => false,
            _ => {
                self.last_triggered.insert(key, now);
                true
            }
        }
    }
}

pub fn evaluate_alerts(
    machine_id: &str,
    display_name: &str,
    telemetry: &MachineTelemetry,
    existing_alerts: &mut VecDeque<Alert>,
) {
    evaluate_alerts_with_suppression(
        machine_id,
        display_name,
        telemetry,
        existing_alerts,
        None,
        DEFAULT_SUPPRESSION_INTERVAL,
    );
}

pub fn evaluate_alerts_with_suppression(
    machine_id: &str,
    display_name: &str,
    telemetry: &MachineTelemetry,
    existing_alerts: &mut VecDeque<Alert>,
    mut suppressor: Option<&mut AlertSuppressor>,
    min_interval: Duration,
) {
    let cpu = telemetry.latest_cpu_percent() as f64;
    check_metric_alert(
        machine_id,
        display_name,
        "cpu",
        cpu,
        CPU_WARNING_THRESHOLD,
        CPU_CRITICAL_THRESHOLD,
        "%",
        existing_alerts,
        suppressor.as_deref_mut(),
        min_interval,
    );

    let mem = telemetry.latest_mem_percent() as f64;
    check_metric_alert(
        machine_id,
        display_name,
        "memory",
        mem,
        MEMORY_WARNING_THRESHOLD,
        MEMORY_CRITICAL_THRESHOLD,
        "%",
        existing_alerts,
        suppressor.as_deref_mut(),
        min_interval,
    );

    let disk = telemetry.highest_disk_percent() as f64;
    check_metric_alert(
        machine_id,
        display_name,
        "disk",
        disk,
        DISK_WARNING_THRESHOLD,
        DISK_CRITICAL_THRESHOLD,
        "%",
        existing_alerts,
        suppressor.as_deref_mut(),
        min_interval,
    );
}

fn check_metric_alert(
    machine_id: &str,
    display_name: &str,
    metric: &str,
    value: f64,
    warning_threshold: f64,
    critical_threshold: f64,
    unit: &str,
    existing_alerts: &mut VecDeque<Alert>,
    suppressor: Option<&mut AlertSuppressor>,
    min_interval: Duration,
) {
    let level = if value >= critical_threshold {
        AlertLevel::Critical
    } else if value >= warning_threshold {
        AlertLevel::Warning
    } else {
        return;
    };

    if let Some(suppressor) = suppressor {
        if !suppressor.may_trigger(machine_id, metric, min_interval) {
            return;
        }
    }

    let id = format!(
        "{}-{}-{}",
        machine_id,
        metric,
        chrono::Local::now().timestamp_millis()
    );
    let message = format!(
        "{} {} 达到 {:.1}{}（阈值 {:.1}{}）",
        display_name,
        metric_name(metric),
        value,
        unit,
        if level == AlertLevel::Critical {
            critical_threshold
        } else {
            warning_threshold
        },
        unit
    );

    existing_alerts.push_back(Alert {
        id,
        machine_id: machine_id.to_string(),
        display_name: display_name.to_string(),
        level,
        metric: metric.to_string(),
        value,
        threshold: if level == AlertLevel::Critical {
            critical_threshold
        } else {
            warning_threshold
        },
        message,
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    });

    while existing_alerts.len() > MAX_ALERTS {
        existing_alerts.pop_front();
    }
}

fn metric_name(metric: &str) -> &'static str {
    match metric {
        "cpu" => "CPU 使用率",
        "memory" => "内存使用率",
        "disk" => "磁盘最高占用",
        _ => "指标",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopProcessSortBy {
    Cpu,
    Memory,
}

pub fn aggregate_top_processes(
    machines: &[RemoteMachineState],
    n: usize,
    sort_by: TopProcessSortBy,
) -> Vec<AggregatedProcess> {
    let mut aggregated: Vec<AggregatedProcess> = machines
        .iter()
        .flat_map(|machine| {
            machine
                .telemetry
                .current
                .processes
                .iter()
                .map(move |process| AggregatedProcess {
                    machine_id: machine.machine_id.clone(),
                    display_name: machine.display_name.clone(),
                    pid: process.pid,
                    name: process.name.clone(),
                    cpu_usage: process.cpu_usage,
                    memory_mb: process.memory_mb,
                })
        })
        .collect();

    aggregated.sort_by(|a, b| match sort_by {
        TopProcessSortBy::Cpu => b
            .cpu_usage
            .partial_cmp(&a.cpu_usage)
            .unwrap_or(std::cmp::Ordering::Equal),
        TopProcessSortBy::Memory => b.memory_mb.cmp(&a.memory_mb),
    });

    aggregated.truncate(n);
    aggregated
}

pub fn machine_offline_duration(last_seen: &str) -> Option<Duration> {
    let parsed = NaiveDateTime::parse_from_str(last_seen, "%Y-%m-%d %H:%M:%S").ok()?;
    let now = chrono::Local::now().naive_local();
    let diff = now.signed_duration_since(parsed);
    if diff.num_seconds() < 0 {
        return Some(Duration::from_secs(0));
    }
    Some(Duration::from_secs(diff.num_seconds() as u64))
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{}天{:02}时{:02}分{:02}秒", days, hours, minutes, seconds)
    } else if hours > 0 {
        format!("{}时{:02}分{:02}秒", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}分{:02}秒", minutes, seconds)
    } else {
        format!("{}秒", seconds)
    }
}

pub fn build_host_summary(
    machines: &[RemoteMachineState],
    alerts: &VecDeque<Alert>,
) -> HostSummary {
    let online_count = machines.iter().filter(|m| m.connected).count();
    let offline_count = machines.len() - online_count;
    let top_processes = aggregate_top_processes(machines, 10, TopProcessSortBy::Cpu);
    let alerts: Vec<Alert> = alerts.iter().rev().take(20).cloned().collect();

    HostSummary {
        online_count,
        offline_count,
        total_machines: machines.len(),
        top_processes,
        alerts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monitor_model::MachineTelemetry;
    use crate::sys_info::{SysInfo, SysProcessInfo};

    fn make_machine(
        id: &str,
        name: &str,
        connected: bool,
        processes: Vec<SysProcessInfo>,
    ) -> RemoteMachineState {
        let mut info = SysInfo::default();
        info.processes = processes;
        let mut state = RemoteMachineState {
            machine_id: id.to_string(),
            display_name: name.to_string(),
            peer_ip: "127.0.0.1".to_string(),
            peer_addr: "127.0.0.1:0".to_string(),
            active_peer_addr: "127.0.0.1:0".to_string(),
            last_seen: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            connected,
            telemetry: MachineTelemetry::new(info.clone()),
        };
        state.telemetry.apply_snapshot(info);
        state
    }

    fn make_process(pid: u32, name: &str, cpu: f32, mem_mb: u64) -> SysProcessInfo {
        SysProcessInfo {
            pid,
            name: name.to_string(),
            cpu_usage: cpu,
            memory_mb: mem_mb,
            ..Default::default()
        }
    }

    #[test]
    fn test_evaluate_alerts_cpu_critical() {
        let mut info = SysInfo::default();
        info.cpu.usage = 95.0;
        let telemetry = MachineTelemetry::new(info);
        let mut alerts = VecDeque::new();
        evaluate_alerts("m1", "machine1", &telemetry, &mut alerts);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].level, AlertLevel::Critical);
        assert_eq!(alerts[0].metric, "cpu");
    }

    #[test]
    fn test_evaluate_alerts_no_alert() {
        let telemetry = MachineTelemetry::new(SysInfo::default());
        let mut alerts = VecDeque::new();
        evaluate_alerts("m1", "machine1", &telemetry, &mut alerts);
        assert!(alerts.is_empty());
    }

    #[test]
    fn test_aggregate_top_processes() {
        let machines = vec![
            make_machine(
                "m1",
                "machine1",
                true,
                vec![
                    make_process(1, "a", 10.0, 100),
                    make_process(2, "b", 30.0, 200),
                ],
            ),
            make_machine(
                "m2",
                "machine2",
                true,
                vec![
                    make_process(3, "c", 20.0, 50),
                    make_process(4, "d", 5.0, 500),
                ],
            ),
        ];

        let top = aggregate_top_processes(&machines, 3, TopProcessSortBy::Cpu);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].name, "b");
        assert_eq!(top[1].name, "c");
        assert_eq!(top[2].name, "a");

        let top_mem = aggregate_top_processes(&machines, 2, TopProcessSortBy::Memory);
        assert_eq!(top_mem.len(), 2);
        assert_eq!(top_mem[0].name, "d");
        assert_eq!(top_mem[1].name, "b");
    }

    #[test]
    fn test_machine_offline_duration() {
        let now = chrono::Local::now();
        let past = now - chrono::Duration::seconds(125);
        let duration = machine_offline_duration(&past.format("%Y-%m-%d %H:%M:%S").to_string())
            .expect("should parse");
        assert!(duration.as_secs() >= 120 && duration.as_secs() <= 130);

        assert!(machine_offline_duration("invalid").is_none());
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45秒");
        assert_eq!(format_duration(Duration::from_secs(125)), "2分05秒");
        assert_eq!(format_duration(Duration::from_secs(3725)), "1时02分05秒");
        assert_eq!(
            format_duration(Duration::from_secs(90061)),
            "1天01时01分01秒"
        );
    }

    #[test]
    fn test_alert_suppressor() {
        let mut suppressor = AlertSuppressor::new();
        assert!(suppressor.may_trigger("m1", "cpu", Duration::from_millis(100)));
        assert!(!suppressor.may_trigger("m1", "cpu", Duration::from_millis(100)));
        assert!(suppressor.may_trigger("m1", "memory", Duration::from_millis(100)));
        assert!(suppressor.may_trigger("m2", "cpu", Duration::from_millis(100)));

        std::thread::sleep(Duration::from_millis(120));
        assert!(suppressor.may_trigger("m1", "cpu", Duration::from_millis(100)));
    }
}
