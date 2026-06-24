//! Live WinRing0 / CPU MSR probe (Windows only).
#![cfg(target_os = "windows")]

#[test]
#[ignore = "requires admin; loads WinRing0 kernel driver"]
fn probe_winring0_and_cpu_msrs() {
    let mut mgr = monitor_app::sys_info_mgr::SysInfoManager::new();
    let info = mgr.load_system_info();
    let cpu = &info.cpu;

    eprintln!("CPU brand: {}", cpu.brand);
    eprintln!(
        "frequencies GHz: base={:.2} current={:.2} max={:.2}",
        cpu.base_frequency, cpu.current_frequency, cpu.max_frequency
    );
    eprintln!("temperature C: {:.1}", cpu.temperature);
    eprintln!("physical_cores={}", cpu.physical_cores);

    if cpu.current_frequency <= cpu.base_frequency + 0.01 && cpu.max_frequency <= cpu.base_frequency + 0.01 {
        eprintln!("WARNING: current/max stuck at base — WinRing0 MSR path likely failed");
    }
    if cpu.temperature <= 0.0 {
        eprintln!("WARNING: no CPU temperature — MSR/WMI/sysinfo all failed");
    }
}
