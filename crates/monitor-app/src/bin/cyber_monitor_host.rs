#![cfg_attr(not(test), windows_subsystem = "windows")]

use clap::Parser;

fn main() {
    let _guard = monitor_app::single_instance::ensure_single_instance("CyberMonitorHost_SingleInstance");
    if _guard.is_none() {
        return;
    }

    let cli = monitor_app::monitor_host_app::HostCli::parse();
    monitor_app::monitor_host_app::run(cli);
}
