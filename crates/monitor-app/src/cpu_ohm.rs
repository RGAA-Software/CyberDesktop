//! CPU temperature and frequency via OpenHardwareMonitor-style MSR access.

/// Pure decode helpers (unit-tested against OpenHardwareMonitor reference values).
pub mod formulas {
    pub const AMD_THM_TCON_TEMP_RANGE_SEL: u32 = 0x0008_0000;
    pub const AMD_CCD_TEMP_VALID: u32 = 0x800;

    pub fn amd_max_ccd_count(model: u32) -> usize {
        match model & 0xF0 {
            0x30 | 0x70 => 8,
            _ => 4,
        }
    }

    pub fn amd_package_temperature_celsius(value: u32) -> f32 {
        let mut temp = ((value >> 21) & 0x7FF) as f32 / 8.0;
        if value & AMD_THM_TCON_TEMP_RANGE_SEL != 0 {
            temp -= 49.0;
        }
        temp
    }

    pub fn amd_ccd_temperature_celsius(value: u32) -> Option<f32> {
        if value & AMD_CCD_TEMP_VALID == 0 {
            return None;
        }
        let temp = (value & 0x7FF) as f32 / 8.0 - 49.0;
        (temp > 0.0 && temp < 150.0).then_some(temp)
    }

    pub fn intel_msr_temperature_celsius(eax: u32, tj_max: f32) -> Option<f32> {
        if eax & 0x8000_0000 == 0 {
            return None;
        }
        let delta_t = ((eax & 0x007F_0000) >> 16) as f32;
        let temp = tj_max - delta_t;
        (temp > 0.0 && temp < 150.0).then_some(temp)
    }

    pub fn amd_core_clock_mhz(fid: u32, dfs_id: u32, bus_mhz: f64) -> Option<f64> {
        if dfs_id == 0 {
            return None;
        }
        let multiplier = 2.0 * fid as f64 / dfs_id as f64;
        (multiplier > 0.0).then_some(multiplier * bus_mhz)
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use std::arch::x86_64::__cpuid;
    use std::sync::OnceLock;

    use super::super::cpu_ring0::with_ring0;
    use super::formulas::{
        amd_ccd_temperature_celsius, amd_max_ccd_count, amd_package_temperature_celsius,
        intel_msr_temperature_celsius,
    };
    use super::CpuHardwareSnapshot;

    const IA32_PERF_STATUS: u32 = 0x0198;
    const IA32_THERM_STATUS: u32 = 0x019C;
    const IA32_TEMPERATURE_TARGET: u32 = 0x01A2;
    const IA32_PACKAGE_THERM_STATUS: u32 = 0x01B1;
    const MSR_PLATFORM_INFO: u32 = 0x00CE;
    const MSR_TURBO_RATIO_LIMIT: u32 = 0x01AD;
    const MSR_P_STATE_0: u32 = 0xC001_0064;
    const MSR_FAMILY_17H_P_STATE: u32 = 0xC001_0293;
    const FAMILY_17H_THM_TCON_TEMP: u32 = 0x0005_9800;
    const FAMILY_17H_CCD_TEMP_BASE: u32 = 0x0005_9954;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Vendor {
        Unknown,
        Intel,
        Amd,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum IntelUarch {
        Legacy,
        Nehalem,
        SandyBridge,
    }

    pub fn read_cpu_hardware_snapshot(
        _brand: &str,
        physical_cores: usize,
        logical_cores: usize,
    ) -> Option<CpuHardwareSnapshot> {
        with_ring0(|ring0| {
            let vendor = cpuid_vendor();
            let physical_count = physical_cores.clamp(1, 128);
            let logical_count = logical_cores.clamp(1, 128);
            match vendor {
                Vendor::Intel => read_intel(ring0, logical_count),
                Vendor::Amd => read_amd(ring0, physical_count),
                Vendor::Unknown => None,
            }
        })
    }

    pub fn read_cpu_temperature_celsius(
        physical_cores: usize,
        logical_cores: usize,
    ) -> Option<f32> {
        with_ring0(|ring0| {
            let physical_count = physical_cores.clamp(1, 128);
            let logical_count = logical_cores.clamp(1, 128);
            match cpuid_vendor() {
                Vendor::Intel => read_intel_temperature(ring0, logical_count),
                Vendor::Amd => read_amd_temperature(ring0, physical_count),
                Vendor::Unknown => None,
            }
        })
    }

    fn cpuid_vendor() -> Vendor {
        let result = __cpuid(0);
        if result.ebx == 0x756e_6547 && result.edx == 0x4965_6e69 && result.ecx == 0x6c65_746e {
            Vendor::Intel
        } else if result.ebx == 0x6874_7541
            && result.edx == 0x6974_6e65
            && result.ecx == 0x444d_4163
        {
            Vendor::Amd
        } else {
            Vendor::Unknown
        }
    }

    fn cpuid_family_model() -> (u32, u32) {
        let eax = __cpuid(1).eax;
        let base_family = (eax >> 8) & 0xF;
        let base_model = (eax >> 4) & 0xF;
        let ext_family = (eax >> 20) & 0xFF;
        let ext_model = (eax >> 16) & 0xF;
        let family = base_family + ext_family;
        let model = base_model + (ext_model << 4);
        (family, model)
    }

    fn intel_uarch(family: u32, model: u32) -> IntelUarch {
        if family != 0x06 {
            return IntelUarch::Legacy;
        }
        match model {
            0x1A | 0x1E | 0x1F | 0x25 | 0x2C | 0x2E | 0x2F => IntelUarch::Nehalem,
            0x2A | 0x2D | 0x3A | 0x3E | 0x3C | 0x3F | 0x45 | 0x46 | 0x3D | 0x47 | 0x4F | 0x56
            | 0x36 | 0x37 | 0x4A | 0x4D | 0x5A | 0x5D | 0x4E | 0x5E | 0x55 | 0x4C | 0x8E | 0x9E
            | 0x5C | 0x5F | 0x66 | 0x7D | 0x7E | 0x6A | 0x6C | 0xA5 | 0xA6 | 0x86 | 0x8C | 0x8D => {
                IntelUarch::SandyBridge
            }
            _ if model >= 0x2A => IntelUarch::SandyBridge,
            _ if model >= 0x1A => IntelUarch::Nehalem,
            _ => IntelUarch::Legacy,
        }
    }

    /// TSC frequency is a hardware constant; cache it after the first successful
    /// measurement so we don't burn ~125 ms (5 x 25 ms) on every sample.
    static TSC_MHZ: OnceLock<Option<f64>> = OnceLock::new();

    fn estimate_tsc_frequency_mhz() -> f64 {
        TSC_MHZ
            .get_or_init(|| {
                let mut best = 0.0;
                for _ in 0..5 {
                    let start = std::time::Instant::now();
                    let tsc_start = unsafe { std::arch::x86_64::_rdtsc() };
                    while start.elapsed().as_millis() < 25 {}
                    let elapsed = start.elapsed().as_secs_f64();
                    let tsc_end = unsafe { std::arch::x86_64::_rdtsc() };
                    if elapsed > 0.0 {
                        let mhz =
                            (tsc_end.saturating_sub(tsc_start)) as f64 / elapsed / 1_000_000.0;
                        if mhz > best {
                            best = mhz;
                        }
                    }
                }
                if best > 0.0 {
                    Some(best)
                } else {
                    None
                }
            })
            .unwrap_or(0.0)
    }

    fn read_intel(
        ring0: &super::super::cpu_ring0::WinRing0,
        core_count: usize,
    ) -> Option<CpuHardwareSnapshot> {
        let (family, model) = cpuid_family_model();
        let uarch = intel_uarch(family, model);
        let tsc_mhz = estimate_tsc_frequency_mhz();
        if tsc_mhz <= 0.0 {
            return None;
        }

        let (platform_eax, _) = ring0.rdmsr(MSR_PLATFORM_INFO)?;
        let platform_multiplier = match uarch {
            IntelUarch::Legacy => {
                let (_, edx) = ring0.rdmsr(IA32_PERF_STATUS)?;
                ((edx >> 8) & 0x1F) as f64 + 0.5 * (((edx >> 14) & 1) as f64)
            }
            IntelUarch::Nehalem | IntelUarch::SandyBridge => ((platform_eax >> 8) & 0xFF) as f64,
        };
        if platform_multiplier <= 0.0 {
            return None;
        }

        let bus_mhz = tsc_mhz / platform_multiplier;
        let max_non_turbo_ratio = ((platform_eax >> 8) & 0xFF) as f64;
        let turbo_ratio = ring0
            .rdmsr(MSR_TURBO_RATIO_LIMIT)
            .map(|(eax, _)| (eax & 0xFF) as f64)
            .filter(|ratio| *ratio > 0.0)
            .unwrap_or(max_non_turbo_ratio);

        let mut core_clocks_mhz = Vec::new();
        for core in 0..core_count {
            let Some((eax, edx)) = ring0.rdmsr_on_core(IA32_PERF_STATUS, core) else {
                continue;
            };
            let multiplier = match uarch {
                IntelUarch::Nehalem => (eax & 0xFF) as f64,
                IntelUarch::SandyBridge => ((eax >> 8) & 0xFF) as f64,
                IntelUarch::Legacy => ((eax >> 8) & 0x1F) as f64 + 0.5 * (((eax >> 14) & 1) as f64),
            };
            if multiplier > 0.0 {
                core_clocks_mhz.push(multiplier * bus_mhz);
            }
            let _ = edx;
        }

        let current_mhz = if core_clocks_mhz.is_empty() {
            tsc_mhz
        } else {
            core_clocks_mhz.iter().sum::<f64>() / core_clocks_mhz.len() as f64
        };
        let max_core_mhz = core_clocks_mhz.iter().copied().fold(current_mhz, f64::max);

        let temperature_c = read_intel_temperature(ring0, core_count).unwrap_or(0.0);
        let max_ghz = ((turbo_ratio * bus_mhz).max(max_core_mhz) / 1000.0) as f32;

        Some(CpuHardwareSnapshot {
            temperature_c,
            current_frequency_ghz: (current_mhz / 1000.0) as f32,
            max_frequency_ghz: max_ghz,
        })
    }

    fn intel_tj_max_celsius(ring0: &super::super::cpu_ring0::WinRing0, core: usize) -> f32 {
        ring0
            .rdmsr_on_core(IA32_TEMPERATURE_TARGET, core)
            .map(|(eax, _)| ((eax >> 16) & 0xFF) as f32)
            .filter(|value| *value > 0.0)
            .unwrap_or(100.0)
    }

    fn intel_temperature_from_msr(eax: u32, tj_max: f32) -> Option<f32> {
        intel_msr_temperature_celsius(eax, tj_max)
    }

    fn read_intel_temperature(
        ring0: &super::super::cpu_ring0::WinRing0,
        core_count: usize,
    ) -> Option<f32> {
        let package_tj_max = intel_tj_max_celsius(ring0, 0);
        let package_temp = ring0
            .rdmsr(IA32_PACKAGE_THERM_STATUS)
            .and_then(|(eax, _)| intel_temperature_from_msr(eax, package_tj_max));

        let mut best = package_temp;
        for core in 0..core_count {
            let tj_max = intel_tj_max_celsius(ring0, core);
            let Some((eax, _)) = ring0.rdmsr_on_core(IA32_THERM_STATUS, core) else {
                continue;
            };
            if let Some(temp) = intel_temperature_from_msr(eax, tj_max) {
                best = Some(best.map_or(temp, |current| current.max(temp)));
            }
        }
        best
    }

    fn amd_bus_mhz(ring0: &super::super::cpu_ring0::WinRing0, tsc_mhz: f64) -> f64 {
        ring0
            .rdmsr(MSR_P_STATE_0)
            .and_then(|(eax, _)| {
                let dfs_id = ((eax >> 8) & 0x3F) as f64;
                let fid = (eax & 0xFF) as f64;
                if dfs_id > 0.0 {
                    let multiplier = 2.0 * fid / dfs_id;
                    (multiplier > 0.0).then_some(tsc_mhz / multiplier)
                } else {
                    None
                }
            })
            .unwrap_or(tsc_mhz)
    }

    fn read_amd(
        ring0: &super::super::cpu_ring0::WinRing0,
        physical_cores: usize,
    ) -> Option<CpuHardwareSnapshot> {
        let (family, _) = cpuid_family_model();
        let tsc_mhz = estimate_tsc_frequency_mhz();
        if tsc_mhz <= 0.0 {
            return None;
        }

        let bus_mhz = if family >= 0x17 {
            amd_bus_mhz(ring0, tsc_mhz)
        } else {
            tsc_mhz
        };

        let mut core_clocks_mhz = Vec::new();
        for core in 0..physical_cores {
            if let Some((eax, _)) = ring0.rdmsr_on_core(MSR_FAMILY_17H_P_STATE, core) {
                let dfs_id = ((eax >> 8) & 0x3F) as f64;
                let fid = (eax & 0xFF) as f64;
                if dfs_id > 0.0 {
                    core_clocks_mhz.push(2.0 * fid / dfs_id * bus_mhz);
                }
            }
        }

        let current_mhz = if core_clocks_mhz.is_empty() {
            tsc_mhz
        } else {
            core_clocks_mhz.iter().sum::<f64>() / core_clocks_mhz.len() as f64
        };
        let max_core_mhz = core_clocks_mhz.iter().copied().fold(current_mhz, f64::max);

        let temperature_c = read_amd_temperature(ring0, physical_cores).unwrap_or(0.0);
        let max_ghz = (max_core_mhz / 1000.0) as f32;

        Some(CpuHardwareSnapshot {
            temperature_c,
            current_frequency_ghz: (current_mhz / 1000.0) as f32,
            max_frequency_ghz: max_ghz,
        })
    }

    fn read_amd_temperature(
        ring0: &super::super::cpu_ring0::WinRing0,
        _physical_cores: usize,
    ) -> Option<f32> {
        let (family, model) = cpuid_family_model();
        if family < 0x17 {
            return None;
        }

        let package = ring0
            .read_smn_register(FAMILY_17H_THM_TCON_TEMP)
            .map(amd_package_temperature_celsius)
            .filter(|temp| *temp > 0.0 && *temp < 150.0);

        let mut ccd_max = None::<f32>;
        for i in 0..amd_max_ccd_count(model) {
            let address = FAMILY_17H_CCD_TEMP_BASE + (i as u32) * 4;
            let Some(value) = ring0.read_smn_register(address) else {
                continue;
            };
            if let Some(temp) = amd_ccd_temperature_celsius(value) {
                ccd_max = Some(ccd_max.map_or(temp, |current| current.max(temp)));
            }
        }

        package.or(ccd_max)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CpuHardwareSnapshot {
    pub temperature_c: f32,
    pub current_frequency_ghz: f32,
    pub max_frequency_ghz: f32,
}

#[cfg(target_os = "windows")]
pub fn read_cpu_hardware_snapshot(
    _brand: &str,
    physical_cores: usize,
    logical_cores: usize,
) -> Option<CpuHardwareSnapshot> {
    imp::read_cpu_hardware_snapshot(_brand, physical_cores, logical_cores)
}

#[cfg(target_os = "windows")]
pub fn read_cpu_temperature_celsius(physical_cores: usize, logical_cores: usize) -> Option<f32> {
    imp::read_cpu_temperature_celsius(physical_cores, logical_cores)
}

#[cfg(not(target_os = "windows"))]
pub fn read_cpu_hardware_snapshot(
    _brand: &str,
    _physical_cores: usize,
    _logical_cores: usize,
) -> Option<CpuHardwareSnapshot> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn read_cpu_temperature_celsius(_physical_cores: usize, _logical_cores: usize) -> Option<f32> {
    None
}

#[cfg(test)]
mod tests {
    use super::formulas::{
        amd_ccd_temperature_celsius, amd_core_clock_mhz, amd_max_ccd_count,
        amd_package_temperature_celsius, intel_msr_temperature_celsius, AMD_CCD_TEMP_VALID,
    };

    // OpenHardwareMonitor report: Ryzen 9 5900X, SMN 0x59800 = 0x753B0000
    #[test]
    fn amd_package_temperature_matches_ohm_5900x_report() {
        let temp = amd_package_temperature_celsius(0x753B_0000);
        assert!((temp - 68.125).abs() < 0.01, "package temp was {temp}");
    }

    // OpenHardwareMonitor report: SMN 0x59954 = 0xB8A (with valid bit set in full register)
    #[test]
    fn amd_ccd_temperature_matches_ohm_5900x_report() {
        let value = 0xB8A | AMD_CCD_TEMP_VALID;
        let temp = amd_ccd_temperature_celsius(value).unwrap();
        assert!((temp - 64.25).abs() < 0.01, "ccd temp was {temp}");
    }

    #[test]
    fn amd_ccd_temperature_requires_valid_bit() {
        assert!(amd_ccd_temperature_celsius(0x38A).is_none());
    }

    #[test]
    fn amd_max_ccd_count_for_zen3_5900x_model() {
        // CPUID family 0x19 model 0x21 -> model nibble 0x20
        assert_eq!(amd_max_ccd_count(0x21), 4);
        assert_eq!(amd_max_ccd_count(0x70), 8);
    }

    #[test]
    fn intel_msr_temperature_decodes_valid_reading() {
        // valid bit set, delta T = 32 -> 100 - 32 = 68 C
        let eax = 0x8000_0000 | (32 << 16);
        let temp = intel_msr_temperature_celsius(eax, 100.0).unwrap();
        assert!((temp - 68.0).abs() < f32::EPSILON);
    }

    #[test]
    fn intel_msr_temperature_rejects_invalid_reading() {
        assert!(intel_msr_temperature_celsius(0x0020_0000, 100.0).is_none());
    }

    #[test]
    fn amd_core_clock_formula() {
        // 2 * fid / dfs_id * bus_mhz
        let clock = amd_core_clock_mhz(36, 2, 100.0).unwrap();
        assert!((clock - 3600.0).abs() < 1e-6);
    }
}
