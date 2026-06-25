use std::collections::VecDeque;
use std::time::Instant;

use monitor_app::monitor_alert::{
    aggregate_top_processes, build_host_summary, evaluate_alerts, AlertLevel, TopProcessSortBy,
};
use monitor_app::monitor_model::{MachineTelemetry, RemoteMachineState};
use monitor_app::sys_info::{SysCpuInfo, SysDiskInfo, SysInfo, SysMemInfo, SysProcessInfo};

fn make_sys_info(
    cpu_usage: f32,
    mem_used: u64,
    mem_total: u64,
    processes: Vec<SysProcessInfo>,
) -> SysInfo {
    let mut info = SysInfo::default();
    info.cpu = SysCpuInfo {
        usage: cpu_usage,
        ..Default::default()
    };
    info.mem = SysMemInfo {
        used: mem_used,
        total: mem_total,
        ..Default::default()
    };
    info.processes = processes;
    info.disks.push(SysDiskInfo {
        total: 1000,
        available: 0,
        ..Default::default()
    });
    info
}

fn make_process(pid: u32, name: &str, cpu: f32, memory_mb: u64) -> SysProcessInfo {
    SysProcessInfo {
        pid,
        name: name.to_string(),
        cpu_usage: cpu,
        memory_mb,
        ..Default::default()
    }
}

fn make_machine(
    id: &str,
    name: &str,
    connected: bool,
    cpu: f32,
    mem_used: u64,
    mem_total: u64,
    processes: Vec<SysProcessInfo>,
) -> RemoteMachineState {
    let info = make_sys_info(cpu, mem_used, mem_total, processes);
    let mut state = RemoteMachineState {
        machine_id: id.to_string(),
        display_name: name.to_string(),
        peer_ip: "127.0.0.1".to_string(),
        peer_addr: "127.0.0.1:0".to_string(),
        active_peer_addr: "127.0.0.1:0".to_string(),
        last_seen: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        last_seen_at: Instant::now(),
        connected,
        telemetry: MachineTelemetry::new(info.clone()),
    };
    state.telemetry.apply_snapshot(info);
    state
}

#[test]
fn test_host_summary_counts_and_alerts() {
    let machines = vec![
        make_machine(
            "m1",
            "Alpha",
            true,
            95.0,
            8500,
            10000,
            vec![make_process(1, "alpha_proc", 60.0, 1200)],
        ),
        make_machine(
            "m2",
            "Beta",
            true,
            45.0,
            4000,
            10000,
            vec![make_process(2, "beta_proc", 30.0, 800)],
        ),
        make_machine("m3", "Gamma", false, 0.0, 0, 1, vec![]),
    ];

    let mut alerts = VecDeque::new();
    for machine in &machines {
        if machine.connected {
            evaluate_alerts(
                &machine.machine_id,
                &machine.display_name,
                &machine.telemetry,
                &mut alerts,
            );
        }
    }

    let summary = build_host_summary(&machines, &alerts);
    assert_eq!(summary.online_count, 2);
    assert_eq!(summary.offline_count, 1);
    assert_eq!(summary.total_machines, 3);

    assert!(!summary.alerts.is_empty());
    let cpu_alert = summary
        .alerts
        .iter()
        .find(|a| a.metric == "cpu" && a.machine_id == "m1")
        .expect("CPU critical alert expected");
    assert_eq!(cpu_alert.level, AlertLevel::Critical);

    let top = aggregate_top_processes(&machines, 10, TopProcessSortBy::Cpu);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].name, "alpha_proc");
    assert_eq!(top[0].display_name, "Alpha");
}

#[test]
fn test_offline_machine_ignored_for_alerts() {
    let machines = vec![make_machine(
        "m1",
        "OfflineBox",
        false,
        99.0,
        9900,
        10000,
        vec![],
    )];

    let mut alerts = VecDeque::new();
    for machine in &machines {
        if machine.connected {
            evaluate_alerts(
                &machine.machine_id,
                &machine.display_name,
                &machine.telemetry,
                &mut alerts,
            );
        }
    }

    assert!(alerts.is_empty());
    let summary = build_host_summary(&machines, &alerts);
    assert_eq!(summary.online_count, 0);
    assert_eq!(summary.offline_count, 1);
}
