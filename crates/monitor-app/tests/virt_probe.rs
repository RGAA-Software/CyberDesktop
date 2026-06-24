//! Live hardware virtualization probe:
//! `cargo test -p monitor-app --test virt_probe -- --ignored --nocapture`

#[test]
#[ignore = "prints live virtualization detection"]
fn probe_virt_label() {
    let label = monitor_app::cpu_platform::read_cpu_virtualization_label();
    eprintln!("final label: {label}");
    assert_ne!(label, "不支持", "expected enabled on Hyper-V / VT-x systems");
}

#[test]
#[ignore = "prints live virtualization detection"]
fn probe_virt_signals() {
    let signals = monitor_app::cpu_platform::debug_virtualization_signals();
    eprintln!("{signals}");
}

#[test]
#[ignore = "prints live virtualization detection"]
fn probe_sysinfo_manager_cpu_virtualization() {
    let info = monitor_app::sys_info_mgr::SysInfoManager::new().load_system_info();
    eprintln!(
        "SysInfoManager cpu.virtualization: {}",
        info.cpu.virtualization
    );
    assert_ne!(info.cpu.virtualization, "不支持");
}
