pub mod monitor_actions;
pub mod monitor_alert;
pub mod monitor_app;
pub mod monitor_codec;
pub mod monitor_dashboard;
pub mod monitor_host_app;
pub mod monitor_icons;
pub mod monitor_model;
pub mod monitor_process_ctrl;
pub mod monitor_process_detail;
pub mod monitor_process_details;
pub mod monitor_sender;
pub mod monitor_settings;
pub mod single_instance;
pub mod sys_info;
pub mod sys_info_mgr;
pub mod tray;

pub mod cpu_platform;

pub mod gpu_nvml;
#[cfg(target_os = "windows")]
mod cpu_metrics_windows;
#[cfg(target_os = "windows")]
pub mod disk_metrics_windows;
#[cfg(target_os = "windows")]
mod mem_metrics_windows;
#[cfg(target_os = "windows")]
mod cpu_ohm;
#[cfg(target_os = "windows")]
mod cpu_ring0;
