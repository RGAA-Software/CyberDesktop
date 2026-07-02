#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::io::Write;

use clap::Parser;

fn startup_log(msg: &str) {
    let line = format!("{msg}\n");
    let _ = std::io::stderr().write_all(line.as_bytes());
    let _ = std::io::stderr().flush();
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(dir) = exe_path.parent() {
            let log_path = dir.join("cyber_monitor_startup.log");
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path)
            {
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        startup_log(&format!("[cyber_monitor_host] panic: {info}"));
        if let Some(location) = info.location() {
            startup_log(&format!("  at {location}"));
        }
    }));
    startup_log("[cyber_monitor_host] process started");
    let guard = monitor_app::single_instance::ensure_single_instance(
        "CyberMonitorHost_SingleInstance",
        "CyberMonitorHost_RaiseWindow",
    );
    let Some(guard) = guard else {
        startup_log("[cyber_monitor_host] aborting: another instance is already running or single-instance lock failed");
        return;
    };
    startup_log("[cyber_monitor_host] single-instance lock acquired");
    guard.spawn_raise_listener();

    let cli = monitor_app::monitor_host_app::HostCli::parse();
    monitor_app::monitor_host_app::run(cli);
}
