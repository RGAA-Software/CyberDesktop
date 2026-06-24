//! Live disk manufacturer/model probe:
//! `cargo test -p monitor-app --test disk_probe -- --ignored --nocapture`

#[test]
#[ignore = "prints live disk WMI data"]
fn probe_disk_mounts_and_models() {
    let info = monitor_app::sys_info_mgr::SysInfoManager::new().load_system_info();
    eprintln!("disks ({}):", info.disks.len());
    for disk in &info.disks {
        eprintln!(
            "  mount={:?} type={:?} manufacturer={:?}",
            disk.mount_on, disk.disk_type, disk.manufacturer
        );
    }

    let mounts: Vec<String> = info.disks.iter().map(|d| d.mount_on.clone()).collect();
    let map = monitor_app::disk_metrics_windows::read_disk_manufacturers(&mounts);
    eprintln!("read_disk_manufacturers: {map:#?}");
    eprintln!(
        "debug:\n{}",
        monitor_app::disk_metrics_windows::debug_disk_drive_fields()
    );

    assert!(
        info.disks.iter().any(|d| !d.manufacturer.is_empty()),
        "at least one disk should have manufacturer/model label"
    );
}
