//! CPU platform details via OS queries and CPUID (virtualization, cache).

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::arch::x86_64::__cpuid_count;

/// Human-readable hardware virtualization state (matches Windows Task Manager).
pub fn read_cpu_virtualization_label() -> String {
    virtualization_label_impl()
}

#[cfg(target_os = "windows")]
fn virtualization_label_impl() -> String {
    read_windows_virtualization_label()
}

#[cfg(not(target_os = "windows"))]
fn virtualization_label_impl() -> String {
    #[cfg(target_os = "linux")]
    if let Some(label) = read_linux_virtualization_label() {
        return label;
    }
    match cpu_virtualization_feature() {
        CpuVirtFeature::IntelVmx | CpuVirtFeature::AmdSvm => "已启用".to_string(),
        CpuVirtFeature::None => "不支持".to_string(),
    }
}

pub fn read_cpu_cache_label() -> String {
    #[cfg(target_os = "windows")]
    if let Some((l1, l2, l3)) = read_windows_cache_sizes_kb() {
        return format_cache_levels_kb(l1, l2, l3);
    }

    #[cfg(target_os = "linux")]
    if let Some((l1, l2, l3)) = read_linux_cache_sizes_kb() {
        return format_cache_levels_kb(l1, l2, l3);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let (l1, l2, l3) = read_cache_sizes_cpuid_kb();
        if l1 > 0 || l2 > 0 || l3 > 0 {
            return format_cache_levels_kb(l1, l2, l3);
        }
    }

    "暂不可用".to_string()
}

/// Diagnostic dump for `cache_probe` integration test.
pub fn debug_cache_signals() -> String {
    #[cfg(target_os = "windows")]
    {
        let cache_memory = read_windows_cache_memory_sizes_kb();
        let glpi = read_windows_cache_from_glpi_kb();
        let processor = read_windows_processor_cache_sizes_kb();
        let cpuid = read_cache_sizes_cpuid_kb();
        let packages = query_logical_processor_information()
            .map(|buffer| logical_processor_package_count(&buffer))
            .unwrap_or(1);
        format!(
            "cache_memory={cache_memory:?}\nglpi={glpi:?}\nprocessor_fallback={processor:?}\ncpuid={cpuid:?}\ncores={}\npackages={packages}\nlabel={}",
            physical_core_count(),
            read_cpu_cache_label()
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        let cpuid = read_cache_sizes_cpuid_kb();
        format!("cpuid={cpuid:?}\nlabel={}", read_cpu_cache_label())
    }
}

pub fn format_cpu_frequency_range(base_ghz: f32, max_ghz: f32) -> String {
    if base_ghz <= 0.0 && max_ghz <= 0.0 {
        return "暂不可用".to_string();
    }
    if base_ghz <= 0.0 {
        return format!("{max_ghz:.2} GHz");
    }
    if max_ghz <= 0.0 || (max_ghz - base_ghz).abs() < 0.01 {
        return format!("{base_ghz:.2} GHz");
    }
    format!("{base_ghz:.2} / {max_ghz:.2} GHz")
}

fn format_cache_levels_kb(l1_kb: u64, l2_kb: u64, l3_kb: u64) -> String {
    format!(
        "L1 {} / L2 {} / L3 {}",
        format_cache_kb(l1_kb),
        format_cache_kb(l2_kb),
        format_cache_kb(l3_kb)
    )
}

/// Matches Task Manager style: KB for L1, one-decimal MB when needed.
fn format_cache_kb(kb: u64) -> String {
    if kb == 0 {
        return "—".to_string();
    }
    if kb >= 1024 {
        let mb = kb as f64 / 1024.0;
        if (mb - mb.round()).abs() < 0.05 {
            format!("{} MB", mb.round() as u64)
        } else {
            format!("{mb:.1} MB")
        }
    } else {
        format!("{kb} KB")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CpuVirtFeature {
    None,
    IntelVmx,
    AmdSvm,
}

fn cpu_virtualization_feature() -> CpuVirtFeature {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        use std::arch::x86_64::__cpuid;

        let vendor = __cpuid(0);
        let is_intel = vendor.ebx == 0x756e_6547;
        let is_amd = vendor.ebx == 0x6874_7541;

        if is_intel {
            let features = __cpuid(1);
            if features.ecx & (1 << 5) != 0 {
                return CpuVirtFeature::IntelVmx;
            }
        }
        if is_amd {
            let max_leaf = vendor.eax;
            if max_leaf >= 0x8000_0001 {
                let features = __cpuid(0x8000_0001);
                if features.ecx & (1 << 2) != 0 {
                    return CpuVirtFeature::AmdSvm;
                }
            }
        }
    }
    CpuVirtFeature::None
}

/// WMI / Task Manager aligned cache sizes in kilobytes.
#[cfg(target_os = "windows")]
fn read_windows_cache_sizes_kb() -> Option<(u64, u64, u64)> {
    if let Some(sizes) = read_windows_cache_memory_sizes_kb() {
        return Some(sizes);
    }
    // No COM: same totals Task Manager shows, works when WMI is busy in GUI thread.
    if let Some(sizes) = read_windows_cache_from_glpi_kb() {
        return Some(sizes);
    }
    read_windows_processor_cache_sizes_kb()
}

#[cfg(target_os = "windows")]
fn read_windows_cache_memory_sizes_kb() -> Option<(u64, u64, u64)> {
    use std::collections::HashMap;

    use wmi::{COMLibrary, Variant, WMIConnection};

    fn variant_as_u64(value: Option<&Variant>) -> Option<u64> {
        match value? {
            Variant::UI1(v) => Some(*v as u64),
            Variant::I1(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI2(v) => Some(*v as u64),
            Variant::I2(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI4(v) => Some(*v as u64),
            Variant::I4(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI8(v) => Some(*v),
            Variant::I8(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            _ => None,
        }
    }

    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query("SELECT Level, MaxCacheSize FROM Win32_CacheMemory")
        .ok()?;

    let mut l1 = 0u64;
    let mut l2 = 0u64;
    let mut l3 = 0u64;
    for row in &rows {
        let level = variant_as_u64(row.get("Level"))?;
        let kb = variant_as_u64(row.get("MaxCacheSize"))?;
        match level {
            // One WMI row per system-wide L1/L2 total.
            3 => l1 = l1.max(kb),
            4 => l2 = l2.max(kb),
            // One WMI row per physical L3 (dual-socket → sum).
            5 => l3 += kb,
            _ => {}
        }
    }

    if l1 == 0 && l2 == 0 && l3 == 0 {
        None
    } else {
        Some((l1, l2, l3))
    }
}

/// Fallback when WMI is unavailable. L2/L3 from `Win32_Processor`; L1 from GLPI or CPUID.
#[cfg(target_os = "windows")]
fn read_windows_processor_cache_sizes_kb() -> Option<(u64, u64, u64)> {
    use std::collections::HashMap;

    use wmi::{COMLibrary, Variant, WMIConnection};

    fn variant_as_u64(value: Option<&Variant>) -> Option<u64> {
        match value? {
            Variant::UI1(v) => Some(*v as u64),
            Variant::I1(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI2(v) => Some(*v as u64),
            Variant::I2(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI4(v) => Some(*v as u64),
            Variant::I4(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            Variant::UI8(v) => Some(*v),
            Variant::I8(v) => Some(if *v > 0 { *v as u64 } else { 0 }),
            _ => None,
        }
    }

    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query("SELECT L2CacheSize, L3CacheSize FROM Win32_Processor")
        .ok()?;
    let mut l2 = 0u64;
    let mut l3 = 0u64;
    for row in &rows {
        if let Some(kb) = variant_as_u64(row.get("L2CacheSize")) {
            l2 += kb;
        }
        if let Some(kb) = variant_as_u64(row.get("L3CacheSize")) {
            l3 += kb;
        }
    }
    if l2 == 0 && l3 == 0 {
        return None;
    }

    let l1 = read_windows_cache_from_glpi_kb()
        .map(|(l1, _, _)| l1)
        .filter(|kb| *kb > 0)
        .or_else(|| {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            {
                let (l1_per_core, _, _) = read_cache_sizes_cpuid_per_core_kb();
                let cores = physical_core_count().max(1) as u64;
                let l1 = l1_per_core * cores;
                if l1 > 0 {
                    Some(l1)
                } else {
                    None
                }
            }
            #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
            {
                None
            }
        })?;

    Some((l1, l2, l3))
}

/// Sum per-core L1/L2; L3 is per CPU package (sum when multi-socket). No COM/WMI required.
#[cfg(target_os = "windows")]
fn read_windows_cache_from_glpi_kb() -> Option<(u64, u64, u64)> {
    use windows::Win32::System::SystemInformation::RelationCache;

    let buffer = query_logical_processor_information()?;
    let package_count = logical_processor_package_count(&buffer);
    let mut l1 = 0u64;
    let mut l2 = 0u64;
    let mut l3_sizes = Vec::new();

    for info in &buffer {
        if info.Relationship != RelationCache {
            continue;
        }
        let cache = unsafe { info.Anonymous.Cache };
        let kb = cache.Size as u64 / 1024;
        match cache.Level {
            1 => l1 += kb,
            2 => l2 += kb,
            3 => l3_sizes.push(kb),
            _ => {}
        }
    }

    let l3 = aggregate_l3_cache_kb(l3_sizes, package_count);

    if l1 == 0 && l2 == 0 && l3 == 0 {
        None
    } else {
        Some((l1, l2, l3))
    }
}

#[cfg(target_os = "windows")]
fn logical_processor_package_count(
    buffer: &[windows::Win32::System::SystemInformation::SYSTEM_LOGICAL_PROCESSOR_INFORMATION],
) -> usize {
    use windows::Win32::System::SystemInformation::RelationProcessorPackage;

    buffer
        .iter()
        .filter(|info| info.Relationship == RelationProcessorPackage)
        .count()
        .max(1)
}

/// L3 is shared within a package. Multi-socket systems have one L3 per package (sum).
/// Single-socket may repeat the same L3 size per group (max).
fn aggregate_l3_cache_kb(sizes_kb: Vec<u64>, package_count: usize) -> u64 {
    let sizes: Vec<u64> = sizes_kb.into_iter().filter(|kb| *kb > 0).collect();
    if sizes.is_empty() {
        return 0;
    }
    if package_count > 1 {
        sizes.iter().sum()
    } else {
        sizes.iter().copied().max().unwrap_or(0)
    }
}

#[cfg(target_os = "windows")]
fn query_logical_processor_information(
) -> Option<Vec<windows::Win32::System::SystemInformation::SYSTEM_LOGICAL_PROCESSOR_INFORMATION>> {
    use windows::Win32::System::SystemInformation::{
        GetLogicalProcessorInformation, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
    };

    let mut required = 0u32;
    unsafe {
        let _ = GetLogicalProcessorInformation(None, &mut required);
    }
    let entry_size = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
    let mut buffer =
        vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); required as usize / entry_size + 16];
    let mut returned = required;
    let ok =
        unsafe { GetLogicalProcessorInformation(Some(buffer.as_mut_ptr()), &mut returned).is_ok() };
    if !ok {
        return None;
    }
    let count = returned as usize / entry_size;
    buffer.truncate(count);
    Some(buffer)
}

#[cfg(target_os = "linux")]
fn read_linux_cache_sizes_kb() -> Option<(u64, u64, u64)> {
    use std::collections::HashMap;

    let mut by_level: HashMap<u8, u64> = HashMap::new();
    let cpu0_cache = std::path::Path::new("/sys/devices/system/cpu/cpu0/cache");
    let entries = std::fs::read_dir(cpu0_cache).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let level = std::fs::read_to_string(path.join("level"))
            .ok()?
            .trim()
            .parse::<u8>()
            .ok()?;
        let size = std::fs::read_to_string(path.join("size")).ok()?;
        let kb = parse_linux_cache_size_kb(size.trim())?;
        by_level
            .entry(level)
            .and_modify(|current| *current = (*current).max(kb))
            .or_insert(kb);
    }

    let l1 = *by_level.get(&1)?;
    let l2 = by_level.get(&2).copied().unwrap_or(0);
    let l3 = by_level.get(&3).copied().unwrap_or(0);
    Some((l1, l2, l3))
}

#[cfg(target_os = "linux")]
fn parse_linux_cache_size_kb(raw: &str) -> Option<u64> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(k) = raw.strip_suffix('K') {
        return k.parse().ok();
    }
    if let Some(m) = raw.strip_suffix('M') {
        return m.parse::<u64>().ok().map(|mb| mb * 1024);
    }
    raw.parse().ok()
}

/// CPUID fallback: per-core L1/L2 scaled by physical cores; L3 per package.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn read_cache_sizes_cpuid_kb() -> (u64, u64, u64) {
    let (l1_per_core, l2_per_core, l3_per_package) = read_cache_sizes_cpuid_per_core_kb();
    let cores = physical_core_count().max(1) as u64;
    #[cfg(target_os = "windows")]
    let packages = query_logical_processor_information()
        .map(|buffer| logical_processor_package_count(&buffer))
        .unwrap_or(1) as u64;
    #[cfg(not(target_os = "windows"))]
    let packages = 1u64;
    (
        l1_per_core * cores,
        l2_per_core * cores,
        l3_per_package * packages,
    )
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn read_cache_sizes_cpuid_kb() -> (u64, u64, u64) {
    (0, 0, 0)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn read_cache_sizes_cpuid_per_core_kb() -> (u64, u64, u64) {
    let mut l1_data = 0u64;
    let mut l1_instr = 0u64;
    let mut l1_unified = 0u64;
    let mut l2 = 0u64;
    let mut l3 = 0u64;

    for subleaf in 0u32..64 {
        let info = __cpuid_count(4, subleaf);
        let cache_type = info.eax & 0xFF;
        if cache_type == 0 {
            break;
        }
        let level = (info.eax >> 5) & 0x7;
        let kb = cache_descriptor_kb(&info);
        match level {
            1 => match cache_type {
                1 => l1_data = l1_data.max(kb),
                2 => l1_instr = l1_instr.max(kb),
                3 => l1_unified = l1_unified.max(kb),
                _ => {}
            },
            2 => l2 = l2.max(kb),
            3 => l3 = l3.max(kb),
            _ => {}
        }
    }

    let l1_per_core = if l1_unified > 0 {
        l1_unified
    } else {
        l1_data + l1_instr
    };
    (l1_per_core, l2, l3)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn cache_descriptor_kb(info: &std::arch::x86_64::CpuidResult) -> u64 {
    let ways = ((info.ebx >> 22) & 0x3FF) as u64 + 1;
    let partitions = ((info.ebx >> 12) & 0x3FF) as u64 + 1;
    let line_size = (info.ebx & 0xFFF) as u64 + 1;
    let sets = info.ecx as u64 + 1;
    let bytes = ways * partitions * line_size * sets;
    bytes / 1024
}

fn physical_core_count() -> usize {
    #[cfg(target_os = "windows")]
    {
        if let Some(cores) = read_windows_physical_core_count() {
            return cores;
        }
    }
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

#[cfg(target_os = "windows")]
fn read_windows_physical_core_count() -> Option<usize> {
    use windows::Win32::System::SystemInformation::RelationProcessorCore;

    let buffer = query_logical_processor_information()?;
    let cores = buffer
        .iter()
        .filter(|info| info.Relationship == RelationProcessorCore)
        .count();
    if cores > 0 {
        Some(cores)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CpuVirtStatus {
    Enabled,
    Disabled,
    NotCapable,
}

/// Mirrors ProcessHacker `PhGetVirtualStatus` → Task Manager wording.
#[cfg(target_os = "windows")]
fn read_windows_virtualization_status() -> CpuVirtStatus {
    if is_hypervisor_active_cpuid() {
        // Host Hyper-V or guest VM: Task Manager shows Enabled on Hyper-V hosts.
        return CpuVirtStatus::Enabled;
    }

    let cpu_feature = cpu_virtualization_feature();
    if cpu_feature == CpuVirtFeature::None {
        return CpuVirtStatus::NotCapable;
    }

    if is_processor_virt_firmware_enabled() {
        return CpuVirtStatus::Enabled;
    }

    // PhVirtualStatusDisabledWithHyperV: firmware flag off but SLAT+NX active (Hyper-V owns VT).
    if is_processor_slat_enabled() && is_processor_nx_enabled() {
        return CpuVirtStatus::Enabled;
    }

    // WMI hints (best effort; may fail when COM is busy).
    if read_windows_hypervisor_present() == Some(true)
        || read_windows_virtualization_firmware_enabled() == Some(true)
    {
        return CpuVirtStatus::Enabled;
    }
    if read_windows_virtualization_firmware_enabled() == Some(false) {
        return CpuVirtStatus::Disabled;
    }

    CpuVirtStatus::Disabled
}

#[cfg(target_os = "windows")]
fn read_windows_virtualization_label() -> String {
    match read_windows_virtualization_status() {
        CpuVirtStatus::Enabled => "已启用".to_string(),
        CpuVirtStatus::Disabled => "未启用".to_string(),
        CpuVirtStatus::NotCapable => "不支持".to_string(),
    }
}

/// Diagnostic dump for `virt_probe` integration test.
#[cfg(target_os = "windows")]
pub fn debug_virtualization_signals() -> String {
    format!(
        "status={:?}\npf_virt_firmware={}\nhypervisor_cpuid={}\nms_hypervisor={}\nslat={}\nnx={}\nhypervisor_wmi={:?}\nfirmware_wmi={:?}\ncpu_feature={:?}\nlabel={}",
        read_windows_virtualization_status(),
        is_processor_virt_firmware_enabled(),
        is_hypervisor_active_cpuid(),
        is_microsoft_hypervisor_cpuid(),
        is_processor_slat_enabled(),
        is_processor_nx_enabled(),
        read_windows_hypervisor_present(),
        read_windows_virtualization_firmware_enabled(),
        cpu_virtualization_feature(),
        read_cpu_virtualization_label(),
    )
}

#[cfg(not(target_os = "windows"))]
pub fn debug_virtualization_signals() -> String {
    format!(
        "cpu_feature={:?}\nlabel={}",
        cpu_virtualization_feature(),
        read_cpu_virtualization_label()
    )
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn is_hypervisor_active_cpuid() -> bool {
    use std::arch::x86_64::__cpuid;

    (__cpuid(1).ecx & (1 << 31)) != 0
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn is_hypervisor_active_cpuid() -> bool {
    false
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn is_microsoft_hypervisor_cpuid() -> bool {
    use std::arch::x86_64::__cpuid;

    if !is_hypervisor_active_cpuid() {
        return false;
    }
    let info = __cpuid(0x4000_0000);
    // "Microsoft Hv" — ProcessHacker PhVCpuIsMicrosoftHyperV
    info.ebx == 0x7263_694D && info.ecx == 0x666F_736F && info.edx == 0x7648_2074
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn is_microsoft_hypervisor_cpuid() -> bool {
    false
}

#[cfg(target_os = "windows")]
fn is_processor_slat_enabled() -> bool {
    use windows::Win32::System::Threading::{
        IsProcessorFeaturePresent, PF_SECOND_LEVEL_ADDRESS_TRANSLATION,
    };

    unsafe { IsProcessorFeaturePresent(PF_SECOND_LEVEL_ADDRESS_TRANSLATION).as_bool() }
}

#[cfg(target_os = "windows")]
fn is_processor_nx_enabled() -> bool {
    use windows::Win32::System::Threading::{IsProcessorFeaturePresent, PF_NX_ENABLED};

    unsafe { IsProcessorFeaturePresent(PF_NX_ENABLED).as_bool() }
}

#[cfg(target_os = "windows")]
fn is_processor_virt_firmware_enabled() -> bool {
    use windows::Win32::System::Threading::{IsProcessorFeaturePresent, PF_VIRT_FIRMWARE_ENABLED};

    unsafe { IsProcessorFeaturePresent(PF_VIRT_FIRMWARE_ENABLED).as_bool() }
}

#[cfg(target_os = "windows")]
fn read_windows_hypervisor_present() -> Option<bool> {
    use std::collections::HashMap;

    use wmi::{COMLibrary, Variant, WMIConnection};

    fn variant_as_bool(value: Option<&Variant>) -> Option<bool> {
        match value? {
            Variant::Bool(v) => Some(*v),
            Variant::UI1(v) => Some(*v != 0),
            Variant::I1(v) => Some(*v != 0),
            Variant::UI2(v) => Some(*v != 0),
            Variant::I2(v) => Some(*v != 0),
            Variant::UI4(v) => Some(*v != 0),
            Variant::I4(v) => Some(*v != 0),
            _ => None,
        }
    }

    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query("SELECT HypervisorPresent FROM Win32_ComputerSystem")
        .ok()?;
    rows.first()
        .and_then(|row| variant_as_bool(row.get("HypervisorPresent")))
}

#[cfg(target_os = "windows")]
fn read_windows_virtualization_firmware_enabled() -> Option<bool> {
    use std::collections::HashMap;

    use wmi::{COMLibrary, Variant, WMIConnection};

    fn variant_as_bool(value: Option<&Variant>) -> Option<bool> {
        match value? {
            Variant::Bool(v) => Some(*v),
            Variant::UI1(v) => Some(*v != 0),
            Variant::I1(v) => Some(*v != 0),
            Variant::UI2(v) => Some(*v != 0),
            Variant::I2(v) => Some(*v != 0),
            Variant::UI4(v) => Some(*v != 0),
            Variant::I4(v) => Some(*v != 0),
            _ => None,
        }
    }

    let com = COMLibrary::new().ok()?;
    let wmi = WMIConnection::new(com).ok()?;
    let rows: Vec<HashMap<String, Variant>> = wmi
        .raw_query("SELECT VirtualizationFirmwareEnabled FROM Win32_Processor")
        .ok()?;
    rows.first()
        .and_then(|row| variant_as_bool(row.get("VirtualizationFirmwareEnabled")))
}

#[cfg(target_os = "linux")]
fn read_linux_virtualization_label() -> Option<String> {
    let contents = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    let flags = contents
        .lines()
        .find(|line| line.starts_with("flags"))
        .unwrap_or("");
    let has_vmx = flags.split_whitespace().any(|f| f == "vmx");
    let has_svm = flags.split_whitespace().any(|f| f == "svm");
    if !has_vmx && !has_svm {
        return Some("不支持".to_string());
    }
    if std::path::Path::new("/dev/kvm").exists() {
        return Some("已启用".to_string());
    }
    Some("未启用".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_cpu_frequency_range_merges_base_and_max() {
        assert_eq!(format_cpu_frequency_range(2.8, 5.2), "2.80 / 5.20 GHz");
        assert_eq!(format_cpu_frequency_range(0.0, 5.2), "5.20 GHz");
    }

    #[test]
    fn format_cache_kb_matches_task_manager_style() {
        assert_eq!(format_cache_kb(640), "640 KB");
        assert_eq!(format_cache_kb(2560), "2.5 MB");
        assert_eq!(format_cache_kb(20480), "20 MB");
    }

    #[test]
    fn format_cache_levels_kb_dual_socket_24c() {
        assert_eq!(
            format_cache_levels_kb(1536, 6144, 61440),
            "L1 1.5 MB / L2 6 MB / L3 60 MB"
        );
    }

    #[test]
    fn aggregate_l3_single_socket_uses_max() {
        assert_eq!(aggregate_l3_cache_kb(vec![20480, 20480, 20480], 1), 20480);
    }

    #[test]
    fn aggregate_l3_dual_socket_sums_packages() {
        assert_eq!(aggregate_l3_cache_kb(vec![30720, 30720], 2), 61440);
    }

    #[test]
    fn aggregate_l3_dual_socket_ignores_zero_entries() {
        assert_eq!(aggregate_l3_cache_kb(vec![30720, 0, 30720], 2), 61440);
    }

    #[test]
    fn format_cache_levels_kb_i9_10900_wmi_values() {
        assert_eq!(
            format_cache_levels_kb(640, 2560, 20480),
            "L1 640 KB / L2 2.5 MB / L3 20 MB"
        );
    }

    #[test]
    fn cpuid_per_core_scaled_to_i9_10900_totals() {
        let cores = 10u64;
        let (l1_pc, l2_pc, l3) = (64, 256, 20480);
        assert_eq!(
            format_cache_levels_kb(l1_pc * cores, l2_pc * cores, l3),
            "L1 640 KB / L2 2.5 MB / L3 20 MB"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn glpi_cache_matches_task_manager_on_intel_desktop() {
        let sizes = read_windows_cache_from_glpi_kb().expect("GLPI cache");
        let label = format_cache_levels_kb(sizes.0, sizes.1, sizes.2);
        eprintln!("glpi label: {label} sizes={sizes:?}");
        assert_eq!(label, "L1 640 KB / L2 2.5 MB / L3 20 MB");
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    #[test]
    fn cpuid_per_core_cache_when_visible() {
        let (l1_pc, l2_pc, l3) = read_cache_sizes_cpuid_per_core_kb();
        if l1_pc == 0 {
            eprintln!("skip: L1 hidden under hypervisor (l2_pc={l2_pc}, l3={l3})");
            return;
        }
        let cores = physical_core_count().max(1) as u64;
        let label = format_cache_levels_kb(l1_pc * cores, l2_pc * cores, l3);
        eprintln!("cpuid scaled label: {label}");
        assert_eq!(label, "L1 640 KB / L2 2.5 MB / L3 20 MB");
    }
}
