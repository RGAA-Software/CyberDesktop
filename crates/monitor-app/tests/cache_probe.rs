//! Live CPU cache probe (matches Task Manager on i9-10900):
//! `cargo test -p monitor-app --test cache_probe -- --ignored --nocapture`

const EXPECTED: &str = "L1 640 KB / L2 2.5 MB / L3 20 MB";

#[test]
#[ignore = "prints live cache detection"]
fn probe_cache_label() {
    let label = monitor_app::cpu_platform::read_cpu_cache_label();
    eprintln!("cache label: {label}");
    assert_eq!(label, EXPECTED);
}

#[test]
#[ignore = "prints live cache detection"]
fn probe_cache_signals() {
    eprintln!("{}", monitor_app::cpu_platform::debug_cache_signals());
}

#[test]
#[ignore = "prints live cache detection"]
fn probe_sysinfo_manager_cache_summary() {
    let info = monitor_app::sys_info_mgr::SysInfoManager::new().load_system_info();
    eprintln!("SysInfoManager cache_summary: {}", info.cpu.cache_summary);
    assert_eq!(info.cpu.cache_summary, EXPECTED);
}
