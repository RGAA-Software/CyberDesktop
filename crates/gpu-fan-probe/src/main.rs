//! Standalone GPU fan-speed probe (NVIDIA NVML + AMD ADLX).
//!
//! Build:   cargo build -p gpu-fan-probe --release
//! Run:     gpu-fan-probe.exe
//! Loop:    gpu-fan-probe.exe --loop

use chrono::Local;
use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::{Device, Nvml};
use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::thread;
use std::time::Duration;

const MAX_FAN_PROBE: u32 = 8;
const LOG_FILE_NAME: &str = "gpu-fan-probe.log";

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S%.3f").to_string()
}

fn log_line(line: &str) {
    let stamped = format!("[{}] {}", now(), line);
    println!("{}", stamped);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE_NAME)
    {
        let _ = writeln!(file, "{}", stamped);
    }
}

fn log_blank() {
    println!();
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_FILE_NAME)
    {
        let _ = writeln!(file);
    }
}

fn describe_nvml_error(err: &NvmlError) -> String {
    match err {
        NvmlError::NotSupported => "NotSupported (API not supported on this GPU/Driver)".to_string(),
        NvmlError::InvalidArg => "InvalidArg (fan index out of range)".to_string(),
        NvmlError::Uninitialized => "Uninitialized".to_string(),
        NvmlError::GpuLost => "GpuLost".to_string(),
        NvmlError::Unknown => "Unknown".to_string(),
        other => format!("{:?}", other),
    }
}

fn is_failed_to_load_symbol(err: &NvmlError) -> bool {
    format!("{:?}", err).contains("FailedToLoadSymbol")
}

fn rpm_unavailable_reason(err: &NvmlError, driver_version: &str) -> String {
    if is_failed_to_load_symbol(err) {
        format!(
            "{} -- NOTE: nvmlDeviceGetFanSpeedRPM requires NVIDIA driver 565+; current driver is {}.",
            describe_nvml_error(err),
            driver_version
        )
    } else {
        describe_nvml_error(err)
    }
}

fn probe_nvidia_device(device: &Device, index: u32, driver_version: &str) {
    log_line(&format!("--- NVIDIA GPU index {} ---", index));

    match device.name() {
        Ok(name) => log_line(&format!("name: {}", name)),
        Err(err) => log_line(&format!("name error: {:?}", err)),
    }

    match device.uuid() {
        Ok(uuid) => log_line(&format!("uuid: {}", uuid)),
        Err(err) => log_line(&format!("uuid error: {:?}", err)),
    }

    match device.serial() {
        Ok(serial) => log_line(&format!("serial: {}", serial)),
        Err(err) => log_line(&format!("serial error: {:?}", err)),
    }

    match device.pci_info() {
        Ok(pci) => {
            log_line(&format!(
                "pci_info: domain={:#x} bus={:#x} device={:#x}",
                pci.domain, pci.bus, pci.device
            ));
            log_line(&format!("pci_bus_id: {}", pci.bus_id));
            log_line(&format!("pci_device_id: {:#x}", pci.pci_device_id));
        }
        Err(err) => log_line(&format!("pci_info error: {:?}", err)),
    }

    match device.temperature(TemperatureSensor::Gpu) {
        Ok(t) => log_line(&format!("temperature.gpu: {} C", t)),
        Err(err) => log_line(&format!("temperature.gpu error: {}", describe_nvml_error(&err))),
    }

    match device.power_usage() {
        Ok(p) => log_line(&format!("power_usage: {} mW", p)),
        Err(err) => log_line(&format!("power_usage error: {}", describe_nvml_error(&err))),
    }

    match device.enforced_power_limit() {
        Ok(p) => log_line(&format!("enforced_power_limit: {} mW", p)),
        Err(err) => log_line(&format!(
            "enforced_power_limit error: {}",
            describe_nvml_error(&err)
        )),
    }

    match device.num_fans() {
        Ok(n) => log_line(&format!("num_fans: {}", n)),
        Err(err) => log_line(&format!("num_fans error: {}", describe_nvml_error(&err))),
    }

    log_line("probing fan_speed_rpm(index) ...");
    let mut any_rpm_ok = false;
    let mut max_rpm = 0u32;
    for i in 0..MAX_FAN_PROBE {
        match device.fan_speed_rpm(i) {
            Ok(rpm) => {
                any_rpm_ok = true;
                max_rpm = max_rpm.max(rpm);
                log_line(&format!("  fan_speed_rpm({}) = {} RPM", i, rpm));
            }
            Err(NvmlError::InvalidArg) => {
                log_line(&format!(
                    "  fan_speed_rpm({}) = {} -> stop probing higher indices",
                    i,
                    describe_nvml_error(&NvmlError::InvalidArg)
                ));
                break;
            }
            Err(NvmlError::NotSupported) => {
                log_line(&format!(
                    "  fan_speed_rpm({}) = {}",
                    i,
                    describe_nvml_error(&NvmlError::NotSupported)
                ));
                break;
            }
            Err(err) => {
                log_line(&format!(
                    "  fan_speed_rpm({}) = {}",
                    i,
                    rpm_unavailable_reason(&err, driver_version)
                ));
                if is_failed_to_load_symbol(&err) {
                    log_line("  -> symbol missing in nvml.dll; no need to probe remaining indices");
                    break;
                }
            }
        }
    }
    if any_rpm_ok {
        log_line(&format!("max rpm across probed fans: {} RPM", max_rpm));
    } else {
        log_line("no fan_speed_rpm reading succeeded");
    }

    log_line("probing fan_speed(index) (duty cycle %) ...");
    let mut any_pct_ok = false;
    let mut max_pct = 0u32;
    for i in 0..MAX_FAN_PROBE {
        match device.fan_speed(i) {
            Ok(pct) => {
                any_pct_ok = true;
                max_pct = max_pct.max(pct.min(100));
                log_line(&format!("  fan_speed({}) = {}%", i, pct));
            }
            Err(NvmlError::InvalidArg) => {
                log_line(&format!(
                    "  fan_speed({}) = {} -> stop probing higher indices",
                    i,
                    describe_nvml_error(&NvmlError::InvalidArg)
                ));
                break;
            }
            Err(NvmlError::NotSupported) => {
                log_line(&format!(
                    "  fan_speed({}) = {}",
                    i,
                    describe_nvml_error(&NvmlError::NotSupported)
                ));
                break;
            }
            Err(err) => {
                log_line(&format!("  fan_speed({}) = {}", i, describe_nvml_error(&err)));
            }
        }
    }
    if any_pct_ok {
        log_line(&format!("max duty cycle across probed fans: {}%", max_pct));
    } else {
        log_line("no fan_speed% reading succeeded");
    }

    let final_rpm = if any_rpm_ok { max_rpm } else { 0 };
    let final_pct = if any_pct_ok { 0 } else { max_pct };
    log_line(&format!(
        "NVIDIA SUGGESTED fan reading: rpm={} rpm_valid={} percent={}",
        final_rpm, any_rpm_ok, final_pct
    ));
}

fn probe_nvidia() {
    log_line("=== NVIDIA (NVML) ===");
    log_line("NOTE: nvmlDeviceGetFanSpeedRPM requires NVIDIA driver 565 or newer.");
    let nvml = match Nvml::init() {
        Ok(n) => {
            log_line("NVML::init() OK");
            n
        }
        Err(err) => {
            log_line(&format!("NVML::init() FAILED: {:?}", err));
            log_line("(请检查 NVIDIA 驱动是否正常，nvml.dll 是否存在)");
            return;
        }
    };

    let driver_version = match nvml.sys_driver_version() {
        Ok(v) => {
            log_line(&format!("NVML driver version: {}", v));
            v
        }
        Err(err) => {
            log_line(&format!("NVML driver version error: {:?}", err));
            "unknown".to_string()
        }
    };

    match nvml.sys_nvml_version() {
        Ok(v) => log_line(&format!("NVML library version: {}", v)),
        Err(err) => log_line(&format!("NVML library version error: {:?}", err)),
    }

    let count = match nvml.device_count() {
        Ok(c) => {
            log_line(&format!("NVML device count: {}", c));
            c
        }
        Err(err) => {
            log_line(&format!("NVML device_count error: {:?}", err));
            return;
        }
    };

    if count == 0 {
        log_line("No NVIDIA GPU detected by NVML.");
        return;
    }

    for i in 0..count {
        match nvml.device_by_index(i) {
            Ok(device) => probe_nvidia_device(&device, i, &driver_version),
            Err(err) => log_line(&format!("device_by_index({}) error: {:?}", i, err)),
        }
    }
}

fn probe_amd() {
    log_line("=== AMD (ADLX) ===");

    let helper = match adlx::helper::AdlxHelper::new() {
        Ok(h) => {
            log_line("ADLX helper created OK");
            h
        }
        Err(err) => {
            log_line(&format!("ADLX helper creation FAILED: {:?}", err));
            log_line("(请检查 AMD 驱动是否正常，ADLX 是否可用)");
            return;
        }
    };

    let system = helper.system();

    let gpu_list = match system.gpus() {
        Ok(list) => {
            let size = list.size();
            log_line(&format!("ADLX GPU list size: {}", size));
            list
        }
        Err(err) => {
            log_line(&format!("ADLX system.gpus() FAILED: {:?}", err));
            return;
        }
    };

    let perf_services = match system.performance_monitoring_services() {
        Ok(s) => {
            log_line("ADLX performance monitoring services OK");
            s
        }
        Err(err) => {
            log_line(&format!(
                "ADLX performance_monitoring_services() FAILED: {:?}",
                err
            ));
            return;
        }
    };

    if gpu_list.size() == 0 {
        log_line("No AMD GPU detected by ADLX.");
        return;
    }

    for i in 0..gpu_list.size() {
        let gpu = match gpu_list.at(i) {
            Ok(g) => g,
            Err(err) => {
                log_line(&format!("gpu_list.at({}) FAILED: {:?}", i, err));
                continue;
            }
        };

        log_line(&format!("--- AMD GPU index {} ---", i));

        match gpu.unique_id() {
            Ok(id) => log_line(&format!("unique_id: {}", id)),
            Err(err) => log_line(&format!("unique_id error: {:?}", err)),
        }

        match gpu.name() {
            Ok(name) => log_line(&format!("name: {}", name)),
            Err(err) => log_line(&format!("name error: {:?}", err)),
        }

        match gpu.total_vram() {
            Ok(vram) => log_line(&format!("total_vram: {} bytes", vram)),
            Err(err) => log_line(&format!("total_vram error: {:?}", err)),
        }

        let supported = match perf_services.supported_gpu_metrics(&gpu) {
            Ok(m) => {
                log_line("supported_gpu_metrics OK");
                m
            }
            Err(err) => {
                log_line(&format!("supported_gpu_metrics error: {:?}", err));
                continue;
            }
        };

        let metrics = match perf_services.current_gpu_metrics(&gpu) {
            Ok(m) => {
                log_line("current_gpu_metrics OK");
                m
            }
            Err(err) => {
                log_line(&format!("current_gpu_metrics error: {:?}", err));
                continue;
            }
        };

        let fan_supported = supported.is_supported_gpu_fan_speed().unwrap_or(false);
        log_line(&format!("is_supported_gpu_fan_speed: {}", fan_supported));
        if fan_supported {
            match metrics.fan_speed() {
                Ok(fan) => {
                    log_line(&format!("fan_speed (raw ADLX): {}", fan));
                    if fan > 100 {
                        log_line(&format!("interpreted: {} RPM", fan));
                    } else if fan > 0 {
                        log_line(&format!("interpreted: {}% duty cycle", fan));
                    } else {
                        log_line("fan_speed returned 0");
                    }
                }
                Err(err) => log_line(&format!("fan_speed error: {:?}", err)),
            }
        }

        let temp_supported = supported.is_supported_gpu_temperature().unwrap_or(false);
        log_line(&format!("is_supported_gpu_temperature: {}", temp_supported));
        if temp_supported {
            match metrics.temperature() {
                Ok(t) => log_line(&format!("temperature: {} C", t)),
                Err(err) => log_line(&format!("temperature error: {:?}", err)),
            }
        }

        let usage_supported = supported.is_supported_gpu_usage().unwrap_or(false);
        log_line(&format!("is_supported_gpu_usage: {}", usage_supported));
        if usage_supported {
            match metrics.usage() {
                Ok(u) => log_line(&format!("usage: {}%", u)),
                Err(err) => log_line(&format!("usage error: {:?}", err)),
            }
        }

        let vram_supported = supported.is_supported_gpu_vram().unwrap_or(false);
        log_line(&format!("is_supported_gpu_vram: {}", vram_supported));
        if vram_supported {
            match metrics.vram() {
                Ok(v) => log_line(&format!("vram used: {} bytes", v)),
                Err(err) => log_line(&format!("vram error: {:?}", err)),
            }
        }
    }
}

fn run_once() {
    log_line("==============================================");
    log_line("GPU Fan Speed Probe (NVIDIA + AMD)");
    log_line(&format!("exe args: {:?}", env::args().collect::<Vec<_>>()));
    log_line(&format!("working dir: {:?}", env::current_dir().unwrap_or_default()));
    log_line("==============================================");

    probe_nvidia();
    log_blank();
    probe_amd();
    log_blank();
}

fn main() {
    let loop_mode = env::args().any(|a| a == "--loop" || a == "-l");

    loop {
        run_once();
        if !loop_mode {
            break;
        }
        log_line("sleeping 2s ... (--loop active, press Ctrl+C to stop)");
        log_blank();
        thread::sleep(Duration::from_secs(2));
    }
}
