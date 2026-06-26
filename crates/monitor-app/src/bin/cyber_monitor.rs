#![cfg_attr(not(test), windows_subsystem = "windows")]

use std::fs::OpenOptions;
use std::io::Write;

fn startup_log(msg: &str) {
    let line = format!("{msg}\n");
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

fn main() {
    std::panic::set_hook(Box::new(|info| {
        startup_log(&format!("[cyber_monitor] panic: {info}"));
        if let Some(location) = info.location() {
            startup_log(&format!("  at {location}"));
        }
    }));
    startup_log("[cyber_monitor] process started (build with file logging)");
    startup_log("[cyber_monitor] about to parse cli");
    startup_log("[cyber_monitor] entering manual arg parse");
    let args: Vec<String> = std::env::args().collect();
    startup_log(&format!("[cyber_monitor] raw args: {args:?}"));
    let startup = args.iter().any(|a| a == "--startup");
    let no_single_instance = args.iter().any(|a| a == "--no-single-instance");
    startup_log(&format!("[cyber_monitor] manual parse: startup={startup}, no_single_instance={no_single_instance}"));
    if no_single_instance {
        startup_log("[cyber_monitor] skipping single-instance check");
    } else {
        let guard = monitor_app::single_instance::ensure_single_instance(
            "CyberMonitor_SingleInstance",
            "CyberMonitor_RaiseWindow",
        );
        let Some(guard) = guard else {
            startup_log("[cyber_monitor] aborting: another instance is already running or single-instance lock failed");
            return;
        };
        startup_log("[cyber_monitor] single-instance lock acquired");
        guard.spawn_raise_listener();
    }
    startup_log("[cyber_monitor] about to call monitor_app::run");
    monitor_app::monitor_app::run(startup);
    startup_log("[cyber_monitor] monitor_app::run returned");
}
