//! Extended physical / virtual memory metrics (TaskExplorer / GetPerformanceInfo).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WindowsMemMetrics {
    pub committed: u64,
    pub commit_peak: u64,
    pub commit_limit: u64,
    pub physical_used: u64,
    pub system_cache: u64,
    pub kernel_ws: u64,
    pub kernel_paged: u64,
    pub kernel_nonpaged: u64,
    pub hw_reserved: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

pub fn read_windows_mem_metrics() -> Option<WindowsMemMetrics> {
    use windows::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};
    use windows::Win32::System::SystemInformation::{
        GetPhysicallyInstalledSystemMemory, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };

    let mut perf = PERFORMANCE_INFORMATION::default();
    perf.cb = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    unsafe { GetPerformanceInfo(&mut perf, perf.cb).ok()? };

    let page = perf.PageSize as u64;
    let committed = perf.CommitTotal as u64 * page;
    let commit_peak = perf.CommitPeak as u64 * page;
    let commit_limit = perf.CommitLimit as u64 * page;
    let physical_used = perf.PhysicalTotal.saturating_sub(perf.PhysicalAvailable) as u64 * page;
    let system_cache = perf.SystemCache as u64 * page;
    let kernel_ws = perf.KernelTotal as u64 * page;
    let kernel_paged = perf.KernelPaged as u64 * page;
    let kernel_nonpaged = perf.KernelNonpaged as u64 * page;

    let mut mem_status = MEMORYSTATUSEX::default();
    mem_status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
    unsafe { GlobalMemoryStatusEx(&mut mem_status).ok()? };

    let swap_total = mem_status.ullTotalPageFile;
    let swap_used = mem_status
        .ullTotalPageFile
        .saturating_sub(mem_status.ullAvailPageFile);

    let mut installed_kb = 0u64;
    let hw_reserved = if unsafe { GetPhysicallyInstalledSystemMemory(&mut installed_kb).is_ok() } {
        installed_kb
            .saturating_mul(1024)
            .saturating_sub(mem_status.ullTotalPhys)
    } else {
        0
    };

    Some(WindowsMemMetrics {
        committed,
        commit_peak,
        commit_limit,
        physical_used,
        system_cache,
        kernel_ws,
        kernel_paged,
        kernel_nonpaged,
        hw_reserved,
        swap_total,
        swap_used,
    })
}
