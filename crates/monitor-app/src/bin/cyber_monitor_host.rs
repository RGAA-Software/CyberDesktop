#![cfg_attr(not(test), windows_subsystem = "windows")]

use clap::Parser;

fn main() {
    let guard = monitor_app::single_instance::ensure_single_instance(
        "CyberMonitorHost_SingleInstance",
        "CyberMonitorHost_RaiseWindow",
    );
    let Some(guard) = guard else {
        return;
    };
    guard.spawn_raise_listener();

    let cli = monitor_app::monitor_host_app::HostCli::parse();
    monitor_app::monitor_host_app::run(cli);
}
