//! Live GPU PCI address probe — run on a machine with GPU:
//! `cargo test -p monitor-app --test gpu_pci_probe probe_local_gpu_pci -- --ignored --nocapture`

#[cfg(windows)]
#[test]
#[ignore = "requires a machine with NVIDIA/AMD GPU; prints live PCI info"]
fn probe_local_gpu_pci() {
    use nvml_wrapper::Nvml;

    use monitor_app::monitor_model::{format_pci_address, pci_address_from_bus_id};
    use monitor_app::sys_info_mgr::SysInfoManager;

    eprintln!("=== NVML raw pci_info ===");
    match Nvml::init() {
        Ok(nvml) => {
            let count = nvml.device_count().unwrap_or(0);
            eprintln!("NVML device count: {count}");
            if count == 0 {
                eprintln!("No NVIDIA GPU detected by NVML.");
            }
            for i in 0..count {
                let Ok(device) = nvml.device_by_index(i) else {
                    eprintln!("GPU index {i}: device_by_index failed");
                    continue;
                };
                let name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
                eprintln!("\n--- GPU index {i}: {name} ---");
                match device.pci_info() {
                    Ok(pci) => {
                        eprintln!(
                            "  domain={:#x} bus={:#x} device={:#x} pci_device_id={:#x}",
                            pci.domain, pci.bus, pci.device, pci.pci_device_id
                        );
                        if !pci.bus_id.is_empty() {
                            eprintln!("  bus_id string: {}", pci.bus_id);
                            eprintln!(
                                "  pci_address_from_bus_id: {:?}",
                                pci_address_from_bus_id(&pci.bus_id)
                            );
                        }
                        eprintln!(
                            "  format_pci_address(bus,device,0): {}",
                            format_pci_address(pci.bus, pci.device, 0)
                        );
                    }
                    Err(err) => eprintln!("  pci_info error: {err:?}"),
                }
                let num_fans = device.num_fans().unwrap_or(0);
                eprintln!("  num_fans: {num_fans}");
                for fi in 0..8u32 {
                    match device.fan_speed_rpm(fi) {
                        Ok(rpm) => eprintln!("  fan_speed_rpm({fi}): {rpm}"),
                        Err(err) => eprintln!("  fan_speed_rpm({fi}): {err:?}"),
                    }
                }
                for fi in 0..8u32 {
                    match device.fan_speed(fi) {
                        Ok(pct) => eprintln!("  fan_speed%({fi}): {pct}"),
                        Err(err) => eprintln!("  fan_speed%({fi}): {err:?}"),
                    }
                }
                let fan = monitor_app::gpu_nvml::read_nvml_fan(&device);
                eprintln!(
                    "  read_nvml_fan: rpm={} rpm_valid={} percent={}",
                    fan.rpm, fan.rpm_valid, fan.percent
                );
            }
        }
        Err(err) => eprintln!("NVML init failed: {err:?}"),
    }

    eprintln!("\n=== SysInfoManager gpus (final pci_address used by UI) ===");
    let info = SysInfoManager::new().load_system_info();
    if info.gpus.is_empty() {
        eprintln!("No GPU entries in SysInfo (NVML/ADLX may be unavailable on this machine).");
    }
    for (i, gpu) in info.gpus.iter().enumerate() {
        eprintln!(
            "GPU {i}: brand={} pci_address={} fan={} rpm_valid={} fan_pct={} id={}",
            gpu.brand,
            gpu.pci_address,
            gpu.fan_speed,
            gpu.fan_rpm_valid,
            gpu.fan_speed_percent,
            gpu.id
        );
    }
}

#[cfg(not(windows))]
#[test]
#[ignore = "GPU PCI probe is Windows-only in this project"]
fn probe_local_gpu_pci() {
    eprintln!("Skipped: run on Windows with GPU hardware.");
}
