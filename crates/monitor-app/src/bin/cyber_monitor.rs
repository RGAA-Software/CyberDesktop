#![cfg_attr(not(test), windows_subsystem = "windows")]

use clap::Parser;

#[derive(Parser, Debug, Clone)]
struct MonitorCli {
    #[arg(long, default_value_t = false)]
    startup: bool,
}

fn main() {
    let _guard = monitor_app::single_instance::ensure_single_instance("CyberMonitor_SingleInstance");
    if _guard.is_none() {
        return;
    }

    let cli = MonitorCli::parse();
    monitor_app::monitor_app::run(cli.startup);
}
