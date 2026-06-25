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

/// Returns true when the error indicates the RPM API itself is unavailable
/// (e.g. the NVML symbol could not be loaded). In that case probing more
/// fan indices will never succeed.
fn rpm_api_unavailable(err: &NvmlError) -> bool {
    // nvml-wrapper surfaces a missing DLL symbol as a libloading error.
    let s = format!("{:?}", err);
    s.contains("FailedToLoadSymbol") || s.contains("GetProcAddress")
}

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
            Err(NvmlError::NotSupported) => {
                // RPM not supported on this device/driver combo.
                break;
            }
            Err(err) if rpm_api_unavailable(&err) => {
                // The nvmlDeviceGetFanSpeedRPM symbol is not present in this
                // nvml.dll (e.g. driver older than 565). Stop probing and let
                // the percent fallback take over.
                break;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_reading_is_zero() {
        let r = GpuFanReading::default();
        assert_eq!(r.rpm, 0);
        assert!(!r.rpm_valid);
        assert_eq!(r.percent, 0);
    }
}
