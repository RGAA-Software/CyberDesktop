//! NVIDIA GPU fan speed via NVML (RPM preferred over duty cycle).

use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Device;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuFanReading {
    pub rpm: u32,
    /// True when `nvmlDeviceGetFanSpeedRPM` succeeded (even if RPM is 0).
    pub rpm_valid: bool,
    /// Duty cycle 0–100; only used when RPM API is unavailable.
    pub percent: u32,
}

const MAX_FAN_PROBE: u32 = 8;

pub fn read_nvml_fan(device: &Device) -> GpuFanReading {
    let mut rpm_valid = false;
    let mut max_rpm = 0u32;

    for i in 0..MAX_FAN_PROBE {
        match device.fan_speed_rpm(i) {
            Ok(rpm) => {
                rpm_valid = true;
                max_rpm = max_rpm.max(rpm);
            }
            Err(NvmlError::InvalidArg) => break,
            Err(NvmlError::NotSupported) => break,
            Err(_) => {}
        }
    }

    if rpm_valid {
        return GpuFanReading {
            rpm: max_rpm,
            rpm_valid: true,
            percent: 0,
        };
    }

    let mut max_pct = 0u32;
    for i in 0..MAX_FAN_PROBE {
        match device.fan_speed(i) {
            Ok(pct) => max_pct = max_pct.max(pct.min(100)),
            Err(NvmlError::InvalidArg) => break,
            Err(NvmlError::NotSupported) => break,
            Err(_) => {}
        }
    }

    GpuFanReading {
        rpm: 0,
        rpm_valid: false,
        percent: max_pct,
    }
}
