//! Windows-specific CPU temperature and clock helpers (WMI + registry).

use std::collections::HashMap;

use wmi::{COMLibrary, Variant, WMIConnection};

fn variant_as_u32(value: Option<&Variant>) -> Option<u32> {
    match value? {
        Variant::UI1(v) => Some(*v as u32),
        Variant::I1(v) => Some(*v as u32),
        Variant::UI2(v) => Some(*v as u32),
        Variant::I2(v) => Some(*v as u32),
        Variant::UI4(v) => Some(*v),
        Variant::I4(v) => Some(*v as u32),
        Variant::UI8(v) => Some(*v as u32),
        Variant::I8(v) => Some(*v as u32),
        _ => None,
    }
}

fn tenths_kelvin_to_celsius(raw: u32) -> f32 {
    (raw as f32 / 10.0) - 273.15
}

pub fn read_cpu_temperature_celsius() -> Option<f32> {
    read_acpi_thermal_zone_celsius().or_else(read_perf_thermal_zone_celsius)
}

fn read_acpi_thermal_zone_celsius() -> Option<f32> {
    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::with_namespace_path("ROOT\\WMI", com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query(
            "SELECT CurrentTemperature FROM MSAcpi_ThermalZoneTemperature WHERE CurrentTemperature IS NOT NULL",
        )
        .ok()?;
    rows.iter()
        .filter_map(|row| variant_as_u32(row.get("CurrentTemperature")))
        .map(tenths_kelvin_to_celsius)
        .filter(|temp| *temp > 0.0 && *temp < 150.0)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
}

fn read_perf_thermal_zone_celsius() -> Option<f32> {
    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query(
            "SELECT Temperature FROM Win32_PerfFormattedData_Counters_ThermalZoneInformation WHERE Temperature IS NOT NULL",
        )
        .ok()?;
    rows.iter()
        .filter_map(|row| variant_as_u32(row.get("Temperature")))
        .map(|raw| {
            if raw > 200 {
                tenths_kelvin_to_celsius(raw)
            } else {
                raw as f32
            }
        })
        .filter(|temp| *temp > 0.0 && *temp < 150.0)
        .max_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal))
}

/// Returns `(current_mhz, max_mhz)` from `Win32_Processor`.
pub fn read_processor_clocks_mhz() -> Option<(f32, f32)> {
    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query("SELECT CurrentClockSpeed, MaxClockSpeed FROM Win32_Processor")
        .ok()?;
    let row = rows.first()?;
    let current = variant_as_u32(row.get("CurrentClockSpeed"))? as f32;
    let max = variant_as_u32(row.get("MaxClockSpeed"))? as f32;
    if current <= 0.0 && max <= 0.0 {
        return None;
    }
    Some((current, max))
}

pub fn read_registry_max_mhz() -> Option<u32> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
        REG_VALUE_TYPE,
    };

    unsafe {
        let subkey: Vec<u16> = OsStr::new("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return None;
        }

        let value_name: Vec<u16> = OsStr::new("~MHz")
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let mut data = 0u32;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut value_type = REG_VALUE_TYPE::default();
        let result = RegQueryValueExW(
            key,
            PCWSTR(value_name.as_ptr()),
            None,
            Some(&mut value_type),
            Some((&mut data as *mut u32).cast()),
            Some(&mut size),
        );
        let _ = RegCloseKey(key);
        if result.is_ok() && data > 0 {
            Some(data)
        } else {
            None
        }
    }
}
