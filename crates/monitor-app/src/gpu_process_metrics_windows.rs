//! Per-process GPU utilization and dedicated VRAM via Windows PDH counters
//! (`GPU Engine`, `GPU Process Memory`) — same source as Task Manager.

use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProcessGpuUsage {
    pub gpu_index: u32,
    pub gpu_name: String,
    pub usage_percent: f32,
    pub dedicated_bytes: u64,
}

pub struct GpuProcessMetricsCollector {
    query: isize,
    engine_counter: isize,
    memory_counter: isize,
    warmed_up: bool,
}

impl Drop for GpuProcessMetricsCollector {
    fn drop(&mut self) {
        unsafe {
            use windows::Win32::System::Performance::PdhCloseQuery;
            if self.query != 0 {
                let _ = PdhCloseQuery(self.query);
            }
        }
    }
}

impl GpuProcessMetricsCollector {
    pub fn open() -> Option<Self> {
        unsafe {
            use windows::core::PCWSTR;
            use windows::Win32::System::Performance::{
                PdhAddEnglishCounterW, PdhCloseQuery, PdhOpenQueryW,
            };

            let mut query = 0isize;
            if PdhOpenQueryW(PCWSTR::null(), 0, &mut query) != 0 {
                return None;
            }

            let mut engine_counter = 0isize;
            let engine_path: Vec<u16> = "\\GPU Engine(*)\\Utilization Percentage"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            if PdhAddEnglishCounterW(query, PCWSTR(engine_path.as_ptr()), 0, &mut engine_counter)
                != 0
            {
                let _ = PdhCloseQuery(query);
                return None;
            }

            let mut memory_counter = 0isize;
            let memory_path: Vec<u16> = "\\GPU Process Memory(*)\\Dedicated Usage"
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            if PdhAddEnglishCounterW(query, PCWSTR(memory_path.as_ptr()), 0, &mut memory_counter)
                != 0
            {
                memory_counter = 0;
            }

            Some(Self {
                query,
                engine_counter,
                memory_counter,
                warmed_up: false,
            })
        }
    }

    pub fn sample(&mut self, gpu_names: &[String]) -> HashMap<u32, ProcessGpuUsage> {
        unsafe {
            use windows::Win32::System::Performance::PdhCollectQueryData;
            if PdhCollectQueryData(self.query) != 0 {
                return HashMap::new();
            }
            if !self.warmed_up {
                self.warmed_up = true;
                return HashMap::new();
            }
        }

        let mut by_pid_phys: HashMap<(u32, u32), f32> = HashMap::new();
        let engine_samples = unsafe { read_counter_array(self.engine_counter) };
        for (instance, value) in engine_samples {
            let Some((pid, phys, engtype)) = parse_gpu_engine_instance(&instance) else {
                continue;
            };
            if !engine_counts_toward_usage(engtype) {
                continue;
            }
            if value.is_finite() && value > 0.0 {
                let entry = by_pid_phys.entry((pid, phys)).or_insert(0.0);
                *entry = (*entry + value as f32).min(100.0);
            }
        }

        let mut dedicated_by_pid: HashMap<u32, u64> = HashMap::new();
        if self.memory_counter != 0 {
            let memory_samples = unsafe { read_counter_array(self.memory_counter) };
            for (instance, value) in memory_samples {
                let Some(pid) = parse_gpu_memory_instance(&instance) else {
                    continue;
                };
                if value.is_finite() && value > 0.0 {
                    let bytes = value.round() as u64;
                    dedicated_by_pid
                        .entry(pid)
                        .and_modify(|total| *total = total.saturating_add(bytes))
                        .or_insert(bytes);
                }
            }
        }

        let mut result: HashMap<u32, ProcessGpuUsage> = HashMap::new();
        for ((pid, phys), usage) in by_pid_phys {
            if usage <= 0.0 {
                continue;
            }
            let gpu_name = gpu_label(gpu_names, phys);
            match result.get_mut(&pid) {
                Some(existing) if existing.usage_percent >= usage => {}
                Some(existing) => {
                    existing.gpu_index = phys;
                    existing.gpu_name = gpu_name;
                    existing.usage_percent = usage;
                }
                None => {
                    result.insert(
                        pid,
                        ProcessGpuUsage {
                            gpu_index: phys,
                            gpu_name,
                            usage_percent: usage,
                            dedicated_bytes: dedicated_by_pid.get(&pid).copied().unwrap_or(0),
                        },
                    );
                }
            }
        }

        for (pid, bytes) in dedicated_by_pid {
            result
                .entry(pid)
                .and_modify(|usage| {
                    if usage.dedicated_bytes == 0 {
                        usage.dedicated_bytes = bytes;
                    }
                })
                .or_insert_with(|| ProcessGpuUsage {
                    dedicated_bytes: bytes,
                    ..Default::default()
                });
        }

        result
    }
}

fn gpu_label(gpu_names: &[String], phys_index: u32) -> String {
    gpu_names
        .get(phys_index as usize)
        .filter(|name| !name.is_empty())
        .cloned()
        .unwrap_or_else(|| format!("GPU {}", phys_index))
}

fn engine_counts_toward_usage(engtype: &str) -> bool {
    matches!(
        engtype,
        "3D" | "Copy"
            | "Compute_0"
            | "Compute_1"
            | "VideoDecode"
            | "VideoEncode"
            | "Graphics"
            | "GDI"
            | "Render"
            | "VideoProcessing"
    ) || engtype.starts_with("Compute")
}

/// `pid_1388_luid_0x00000000_0x00011372_phys_0_eng_8_engtype_3D`
pub fn parse_gpu_engine_instance(instance: &str) -> Option<(u32, u32, &str)> {
    let rest = instance.strip_prefix("pid_")?;
    let (pid_str, rest) = rest.split_once('_')?;
    let pid = pid_str.parse().ok()?;
    let phys_part = rest.split("_phys_").nth(1)?;
    let phys_str = phys_part.split('_').next()?;
    let phys = phys_str.parse().ok()?;
    let engtype_part = instance.split("engtype_").nth(1)?;
    let engtype = engtype_part.split('_').next().unwrap_or(engtype_part);
    Some((pid, phys, engtype))
}

/// `pid_1388_luid_0x00000000_0x00011372` or engine-style instance names.
pub fn parse_gpu_memory_instance(instance: &str) -> Option<u32> {
    let rest = instance.strip_prefix("pid_")?;
    let pid_str = rest.split('_').next()?;
    pid_str.parse().ok()
}

unsafe fn read_counter_array(counter: isize) -> Vec<(String, f64)> {
    use windows::Win32::System::Performance::{
        PdhGetFormattedCounterArrayW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_MORE_DATA,
    };

    if counter == 0 {
        return Vec::new();
    }

    let mut buffer_size = 0u32;
    let mut item_count = 0u32;
    let status = PdhGetFormattedCounterArrayW(
        counter,
        PDH_FMT_DOUBLE,
        &mut buffer_size,
        &mut item_count,
        None,
    );
    if status != PDH_MORE_DATA || buffer_size == 0 {
        return Vec::new();
    }

    let mut buffer = vec![0u8; buffer_size as usize];
    let status = PdhGetFormattedCounterArrayW(
        counter,
        PDH_FMT_DOUBLE,
        &mut buffer_size,
        &mut item_count,
        Some(buffer.as_mut_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>()),
    );
    if status != 0 {
        return Vec::new();
    }

    let items = std::slice::from_raw_parts(
        buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
        item_count as usize,
    );

    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let name = pwstr_to_string(item.szName);
        let status = item.FmtValue.CStatus;
        if status != 0 && status != 1 {
            continue;
        }
        let value = item.FmtValue.Anonymous.doubleValue;
        if name.is_empty() {
            continue;
        }
        out.push((name, value));
    }
    out
}

unsafe fn pwstr_to_string(ptr: windows::core::PWSTR) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.0.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr.0, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_gpu_engine_instance_fields() {
        let instance = "pid_1388_luid_0x00000000_0x00011372_phys_0_eng_8_engtype_3D";
        assert_eq!(parse_gpu_engine_instance(instance), Some((1388, 0, "3D")));
    }

    #[test]
    fn parse_gpu_memory_instance_pid() {
        let instance = "pid_912_luid_0x00000000_0x00010F5C";
        assert_eq!(parse_gpu_memory_instance(instance), Some(912));
    }

    #[test]
    fn engine_usage_filter() {
        assert!(engine_counts_toward_usage("3D"));
        assert!(engine_counts_toward_usage("VideoDecode"));
        assert!(!engine_counts_toward_usage("Security"));
    }
}
