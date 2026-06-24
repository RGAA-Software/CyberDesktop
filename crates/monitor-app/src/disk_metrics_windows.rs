//! Physical disk identity via WMI (`Win32_DiskDrive` Model / Manufacturer).

use std::collections::BTreeMap;

/// Maps logical drive id (`C:`) to a human-readable disk label (model preferred).
pub fn read_disk_manufacturers(mount_points: &[String]) -> BTreeMap<String, String> {
    let mounts = mount_points.to_vec();
    // WMI uses MTA; the UI thread is often already STA — query on a fresh worker thread.
    std::thread::spawn(move || read_disk_manufacturers_on_thread(&mounts))
        .join()
        .unwrap_or_default()
}

fn read_disk_manufacturers_on_thread(mount_points: &[String]) -> BTreeMap<String, String> {
    use std::collections::HashSet;

    use wmi::{COMLibrary, WMIConnection};

    let Ok(com) = COMLibrary::new() else {
        return BTreeMap::new();
    };
    let Ok(wmi) = WMIConnection::new(com) else {
        return BTreeMap::new();
    };

    let drive_models = load_physical_drive_models(&wmi);
    let partition_to_drive = load_partition_to_drive_map(&wmi);
    let logical_to_partition = load_logical_to_partition_map(&wmi);

    let mut seen = HashSet::new();
    let mut result = BTreeMap::new();

    for mount in mount_points {
        let device_id = normalize_device_id(mount);
        if device_id.is_empty() || !seen.insert(device_id.clone()) {
            continue;
        }

        let Some(partition) = logical_to_partition.get(&device_id) else {
            continue;
        };
        let Some(physical_drive) = partition_to_drive.get(partition) else {
            continue;
        };
        let Some((manufacturer, model)) = drive_models.get(physical_drive) else {
            continue;
        };
        let label = resolve_disk_label(manufacturer, model);
        if !label.is_empty() {
            result.insert(device_id, label);
        }
    }

    result
}

fn load_physical_drive_models(
    wmi: &wmi::WMIConnection,
) -> BTreeMap<String, (String, String)> {
    use wmi::Variant;

    let Ok(rows): Result<Vec<std::collections::HashMap<String, Variant>>, _> = wmi.raw_query(
        "SELECT DeviceID, Manufacturer, Model FROM Win32_DiskDrive",
    ) else {
        return BTreeMap::new();
    };

    rows.into_iter()
        .filter_map(|row| {
            let device_id = variant_as_string(row.get("DeviceID"))?;
            let drive_key = physical_drive_key(&device_id)?;
            let manufacturer = variant_as_string(row.get("Manufacturer")).unwrap_or_default();
            let model = variant_as_string(row.get("Model")).unwrap_or_default();
            Some((drive_key, (manufacturer, model)))
        })
        .collect()
}

fn load_partition_to_drive_map(wmi: &wmi::WMIConnection) -> BTreeMap<String, String> {
    load_wmi_association(wmi, "SELECT Antecedent, Dependent FROM Win32_DiskDriveToDiskPartition")
        .into_iter()
        .filter_map(|(antecedent, dependent)| {
            let drive = physical_drive_key(&normalize_wmi_device_id(&antecedent)?)?;
            let partition = normalize_wmi_device_id(&dependent)?;
            Some((partition, drive))
        })
        .collect()
}

fn load_logical_to_partition_map(wmi: &wmi::WMIConnection) -> BTreeMap<String, String> {
    load_wmi_association(wmi, "SELECT Antecedent, Dependent FROM Win32_LogicalDiskToPartition")
        .into_iter()
        .filter_map(|(antecedent, dependent)| {
            let (logical_path, partition_path) = if is_logical_disk_path(&antecedent) {
                (antecedent, dependent)
            } else {
                (dependent, antecedent)
            };
            let device_id = normalize_device_id(&wmi_path_value(&logical_path, "DeviceID")?);
            let partition = normalize_wmi_device_id(&partition_path)?;
            Some((device_id, partition))
        })
        .collect()
}

fn load_wmi_association(
    wmi: &wmi::WMIConnection,
    query: &str,
) -> Vec<(String, String)> {
    use wmi::Variant;

    let Ok(rows): Result<Vec<std::collections::HashMap<String, Variant>>, _> =
        wmi.raw_query(query)
    else {
        return Vec::new();
    };

    rows.into_iter()
        .filter_map(|row| {
            let antecedent = variant_as_string(row.get("Antecedent"))?;
            let dependent = variant_as_string(row.get("Dependent"))?;
            Some((antecedent, dependent))
        })
        .collect()
}

/// Debug dump for live probes (`disk_probe` integration test).
pub fn debug_disk_drive_fields() -> String {
    std::thread::spawn(|| debug_disk_drive_fields_on_thread())
        .join()
        .unwrap_or_else(|_| "disk WMI worker thread panicked".to_string())
}

fn debug_disk_drive_fields_on_thread() -> String {
    use std::fmt::Write;

    use wmi::{COMLibrary, WMIConnection};

    let mut out = String::new();

    let Ok(com) = COMLibrary::new() else {
        return "COM init failed".to_string();
    };
    let Ok(wmi) = WMIConnection::new(com) else {
        return "WMI connect failed".to_string();
    };

    let models = load_physical_drive_models(&wmi);
    let _ = writeln!(out, "Win32_DiskDrive ({}):", models.len());
    for (device_id, (manufacturer, model)) in &models {
        let _ = writeln!(
            out,
            "  {device_id} | mfg={manufacturer:?} model={model:?} label={}",
            resolve_disk_label(manufacturer, model),
        );
    }

    let logical_to_partition = load_logical_to_partition_map(&wmi);
    let _ = writeln!(out, "LogicalDiskToPartition ({}):", logical_to_partition.len());
    for (logical, partition) in &logical_to_partition {
        let _ = writeln!(out, "  {logical} -> {partition}");
    }

    let partition_to_drive = load_partition_to_drive_map(&wmi);
    let _ = writeln!(out, "DiskDriveToDiskPartition ({}):", partition_to_drive.len());
    for (partition, drive) in &partition_to_drive {
        let _ = writeln!(out, "  {partition} -> {drive}");
    }

    out
}

pub fn normalize_device_id(mount: &str) -> String {
    let trimmed = mount.trim().trim_end_matches('\\');
    if trimmed.len() >= 2 && trimmed.as_bytes()[1] == b':' {
        let letter = trimmed.chars().next().unwrap_or('?').to_ascii_uppercase();
        format!("{letter}:")
    } else {
        trimmed.to_string()
    }
}

pub fn resolve_disk_label(manufacturer: &str, model: &str) -> String {
    let manufacturer = clean_text(manufacturer);
    let model = clean_text(model);

    if !model.is_empty() {
        return model;
    }
    if is_generic_manufacturer(&manufacturer) {
        String::new()
    } else {
        manufacturer
    }
}

pub fn wmi_path_value(path: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = path.find(&needle)? + needle.len();
    let rest = &path[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn normalize_wmi_device_id(raw: &str) -> Option<String> {
    wmi_path_value(raw, "DeviceID").or_else(|| {
        let trimmed = clean_text(raw);
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn is_logical_disk_path(path: &str) -> bool {
    path.contains("Win32_LogicalDisk")
        || wmi_path_value(path, "DeviceID")
            .is_some_and(|id| id.len() >= 2 && id.as_bytes()[1] == b':')
}

fn physical_drive_key(raw: &str) -> Option<String> {
    let upper = raw.to_ascii_uppercase();
    let pos = upper.find("PHYSICALDRIVE")?;
    let tail = &raw[pos..];
    let end = tail.find('\\').unwrap_or(tail.len());
    Some(tail[..end].to_ascii_uppercase())
}

fn variant_as_string(value: Option<&wmi::Variant>) -> Option<String> {
    match value? {
        wmi::Variant::String(s) => {
            let cleaned = clean_text(s);
            if cleaned.is_empty() {
                None
            } else {
                Some(cleaned)
            }
        }
        _ => None,
    }
}

fn clean_text(raw: &str) -> String {
    raw.trim().trim_end_matches('.').trim().to_string()
}

fn is_generic_manufacturer(raw: &str) -> bool {
    let lower = raw.trim().to_lowercase();
    lower.is_empty()
        || lower.contains("standard disk drives")
        || lower == "(standard disk drives)"
        || lower == "microsoft"
        || lower == "unknown"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_device_id_from_mount_paths() {
        assert_eq!(normalize_device_id("C:"), "C:");
        assert_eq!(normalize_device_id("c:\\"), "C:");
        assert_eq!(normalize_device_id("D:\\"), "D:");
    }

    #[test]
    fn wmi_path_value_extracts_device_id() {
        assert_eq!(
            wmi_path_value(
                r#"\\DESKTOP\root\cimv2:Win32_LogicalDisk.DeviceID="C:""#,
                "DeviceID"
            ),
            Some("C:".to_string())
        );
    }

    #[test]
    fn resolve_disk_label_prefers_model_over_generic_manufacturer() {
        assert_eq!(
            resolve_disk_label("(Standard disk drives)", "KINGSTON SA400S37240G"),
            "KINGSTON SA400S37240G"
        );
        assert_eq!(
            resolve_disk_label("(Standard disk drives)", "ZHITAI PC005 Active 1TB"),
            "ZHITAI PC005 Active 1TB"
        );
    }

    #[test]
    fn resolve_disk_label_uses_real_manufacturer_when_model_missing() {
        assert_eq!(resolve_disk_label("Samsung", ""), "Samsung");
    }

    #[test]
    fn physical_drive_key_normalizes_wmi_escapes() {
        assert_eq!(
            physical_drive_key(r"\\.\PHYSICALDRIVE1"),
            Some("PHYSICALDRIVE1".to_string())
        );
        assert_eq!(
            physical_drive_key("\\\\.\\PHYSICALDRIVE1"),
            Some("PHYSICALDRIVE1".to_string())
        );
    }

    /// Caller thread may already use STA COM (gpui / shell); disk WMI must still work.
    #[test]
    #[cfg(target_os = "windows")]
    fn read_disk_manufacturers_works_when_caller_thread_is_sta() {
        use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        let mounts: Vec<String> = sysinfo::Disks::new_with_refreshed_list()
            .iter()
            .filter_map(|disk| disk.mount_point().to_str().map(str::to_string))
            .collect();
        if mounts.is_empty() {
            return;
        }

        let labels = read_disk_manufacturers(&mounts);
        assert!(
            !labels.is_empty(),
            "expected at least one disk label after STA COM init on caller thread"
        );
    }
}
