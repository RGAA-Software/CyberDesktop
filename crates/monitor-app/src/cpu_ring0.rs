//! WinRing0 driver access (ported from OpenHardwareMonitor `Ring0.cs` / `KernelDriver.cs`).

use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Services::{
    CloseServiceHandle, CreateServiceW, DeleteService, OpenSCManagerW, OpenServiceW, StartServiceW,
    SC_MANAGER_ALL_ACCESS, SERVICE_ALL_ACCESS, SERVICE_DEMAND_START, SERVICE_ERROR_NORMAL,
    SERVICE_KERNEL_DRIVER,
};
use windows::Win32::System::SystemInformation::{
    GetLogicalProcessorInformation, RelationProcessorCore, SYSTEM_LOGICAL_PROCESSOR_INFORMATION,
};
use windows::Win32::System::Threading::{GetCurrentThread, SetThreadAffinityMask};

const DRIVER_ID: &str = "WinRing0_1_2_0";
const OLS_TYPE: u32 = 40_000;
const METHOD_BUFFERED: u32 = 0;
const FILE_ANY_ACCESS: u32 = 0;
const FILE_READ_ACCESS: u32 = 1;
const FILE_WRITE_ACCESS: u32 = 2;

const fn ctl_code(device_type: u32, function: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | METHOD_BUFFERED
}

const IOCTL_OLS_READ_MSR: u32 = ctl_code(OLS_TYPE, 0x821, FILE_ANY_ACCESS);
const IOCTL_OLS_READ_PCI_CONFIG: u32 = ctl_code(OLS_TYPE, 0x851, FILE_READ_ACCESS);
const IOCTL_OLS_WRITE_PCI_CONFIG: u32 = ctl_code(OLS_TYPE, 0x852, FILE_WRITE_ACCESS);

static RING0: OnceLock<Mutex<Option<WinRing0>>> = OnceLock::new();
static PHYSICAL_CORE_AFFINITY: OnceLock<Vec<usize>> = OnceLock::new();

fn physical_core_affinity_masks() -> &'static [usize] {
    PHYSICAL_CORE_AFFINITY.get_or_init(|| {
        let mut required = 0u32;
        unsafe {
            let _ = GetLogicalProcessorInformation(None, &mut required);
        }
        let entry_size = std::mem::size_of::<SYSTEM_LOGICAL_PROCESSOR_INFORMATION>();
        let mut buffer =
            vec![SYSTEM_LOGICAL_PROCESSOR_INFORMATION::default(); required as usize / entry_size + 16];
        let mut returned = required;
        let ok = unsafe {
            GetLogicalProcessorInformation(Some(buffer.as_mut_ptr()), &mut returned).is_ok()
        };
        if !ok {
            return Vec::new();
        }
        let count = returned as usize / entry_size;
        let mut masks = Vec::new();
        for info in &buffer[..count] {
            if info.Relationship != RelationProcessorCore {
                continue;
            }
            let mask = info.ProcessorMask as usize;
            if mask != 0 {
                let lowest = 1usize << mask.trailing_zeros();
                masks.push(lowest);
            }
        }
        masks
    })
}

fn ring0_slot() -> &'static Mutex<Option<WinRing0>> {
    RING0.get_or_init(|| Mutex::new(None))
}

pub struct WinRing0 {
    handle: HANDLE,
}

// HANDLE is thread-safe to use from the owning process.
unsafe impl Send for WinRing0 {}
unsafe impl Sync for WinRing0 {}

impl Drop for WinRing0 {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

pub fn with_ring0<F, T>(f: F) -> Option<T>
where
    F: FnOnce(&WinRing0) -> Option<T>,
{
    let mut guard = ring0_slot().lock().ok()?;
    if guard.is_none() {
        *guard = WinRing0::open().ok();
    }
    let ring0 = guard.as_ref()?;
    f(ring0)
}

impl WinRing0 {
    fn open() -> Result<Self, ()> {
        if let Ok(handle) = open_device_handle() {
            return Ok(Self { handle });
        }

        let driver_path = extract_driver()?;
        install_and_start_service(&driver_path)?;
        let _ = fs::remove_file(&driver_path);

        let handle = open_device_handle()?;
        Ok(Self { handle })
    }

    pub fn rdmsr(&self, index: u32) -> Option<(u32, u32)> {
        let mut output = 0u64;
        let mut bytes_returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_OLS_READ_MSR,
                Some(&index as *const u32 as *const _),
                std::mem::size_of::<u32>() as u32,
                Some(&mut output as *mut u64 as *mut _),
                std::mem::size_of::<u64>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        }
        .is_ok();
        if !ok {
            return None;
        }
        let eax = output as u32;
        let edx = (output >> 32) as u32;
        Some((eax, edx))
    }

    pub fn rdmsr_on_core(&self, index: u32, core_index: usize) -> Option<(u32, u32)> {
        let affinity = physical_core_affinity_masks()
            .get(core_index)
            .copied()
            .unwrap_or(1usize << (core_index.min(usize::BITS as usize - 1)));
        let _guard = CoreAffinity::new(affinity)?;
        self.rdmsr(index)
    }

    pub fn read_pci_config(&self, pci_address: u32, reg_address: u32) -> Option<u32> {
        if reg_address & 3 != 0 {
            return None;
        }
        #[repr(C, packed)]
        struct Input {
            pci_address: u32,
            reg_address: u32,
        }
        let input = Input {
            pci_address,
            reg_address,
        };
        let mut value = 0u32;
        let mut bytes_returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_OLS_READ_PCI_CONFIG,
                Some(&input as *const Input as *const _),
                std::mem::size_of::<Input>() as u32,
                Some(&mut value as *mut u32 as *mut _),
                std::mem::size_of::<u32>() as u32,
                Some(&mut bytes_returned),
                None,
            )
        }
        .is_ok();
        ok.then_some(value)
    }

    pub fn write_pci_config(&self, pci_address: u32, reg_address: u32, value: u32) -> bool {
        if reg_address & 3 != 0 {
            return false;
        }
        #[repr(C, packed)]
        struct Input {
            pci_address: u32,
            reg_address: u32,
            value: u32,
        }
        let input = Input {
            pci_address,
            reg_address,
            value,
        };
        let mut bytes_returned = 0u32;
        unsafe {
            DeviceIoControl(
                self.handle,
                IOCTL_OLS_WRITE_PCI_CONFIG,
                Some(&input as *const Input as *const _),
                std::mem::size_of::<Input>() as u32,
                None,
                0,
                Some(&mut bytes_returned),
                None,
            )
        }
        .is_ok()
    }

    pub fn read_smn_register(&self, address: u32) -> Option<u32> {
        if !self.write_pci_config(0, 0x60, address) {
            return None;
        }
        self.read_pci_config(0, 0x64)
    }
}

struct CoreAffinity {
    previous: usize,
}

impl CoreAffinity {
    fn new(affinity_mask: usize) -> Option<Self> {
        if affinity_mask == 0 {
            return None;
        }
        let previous = unsafe { SetThreadAffinityMask(GetCurrentThread(), affinity_mask) };
        if previous == 0 {
            return None;
        }
        Some(Self {
            previous: previous as usize,
        })
    }
}

impl Drop for CoreAffinity {
    fn drop(&mut self) {
        unsafe {
            let _ = SetThreadAffinityMask(GetCurrentThread(), self.previous);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn open_device_handle() -> Result<HANDLE, ()> {
    let path = wide(&format!(r"\\.\{}", DRIVER_ID));
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path.as_ptr()),
            (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_NONE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .map_err(|_| ())?;
    if handle == INVALID_HANDLE_VALUE {
        return Err(());
    }
    Ok(handle)
}

fn extract_driver() -> Result<PathBuf, ()> {
    let mut path = std::env::temp_dir();
    path.push("cyber_monitor_WinRing0x64.sys");
    let bytes = include_bytes!("../assets/WinRing0x64.sys");
    let mut file = fs::File::create(&path).map_err(|_| ())?;
    file.write_all(bytes).map_err(|_| ())?;
    file.sync_all().map_err(|_| ())?;
    Ok(path)
}

fn install_and_start_service(driver_path: &PathBuf) -> Result<(), ()> {
    unsafe {
        let manager = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS)
            .map_err(|_| ())?;
        let service_name = wide(DRIVER_ID);
        let driver_wide = wide(&driver_path.to_string_lossy());

        let service = match CreateServiceW(
            manager,
            PCWSTR(service_name.as_ptr()),
            PCWSTR(service_name.as_ptr()),
            SERVICE_ALL_ACCESS,
            SERVICE_KERNEL_DRIVER,
            SERVICE_DEMAND_START,
            SERVICE_ERROR_NORMAL,
            PCWSTR(driver_wide.as_ptr()),
            PCWSTR::null(),
            None,
            PCWSTR::null(),
            PCWSTR::null(),
            PCWSTR::null(),
        ) {
            Ok(handle) => handle,
            Err(_) => OpenServiceW(
                manager,
                PCWSTR(service_name.as_ptr()),
                SERVICE_ALL_ACCESS,
            )
            .map_err(|_| ())?,
        };

        let _ = StartServiceW(service, None);
        let _ = CloseServiceHandle(service);
        let _ = CloseServiceHandle(manager);
        Ok(())
    }
}

#[cfg(test)]
mod ioctl_tests {
    const OLS_TYPE: u32 = 40_000;
    const METHOD_BUFFERED: u32 = 0;

    const fn ctl_code(device_type: u32, function: u32, access: u32) -> u32 {
        (device_type << 16) | (access << 14) | (function << 2) | METHOD_BUFFERED
    }

    #[test]
    fn winring0_ioctl_codes_match_ohm() {
        assert_eq!(ctl_code(OLS_TYPE, 0x821, 0), 0x9C40_2084); // READ_MSR
        assert_eq!(ctl_code(OLS_TYPE, 0x851, 1), 0x9C40_6144); // READ_PCI
        assert_eq!(ctl_code(OLS_TYPE, 0x852, 2), 0x9C40_A148); // WRITE_PCI
    }
}

#[allow(dead_code)]
pub fn uninstall_driver_service() {
    unsafe {
        let manager = OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS);
        if let Ok(manager) = manager {
            let service_name = wide(DRIVER_ID);
            if let Ok(service) =
                OpenServiceW(manager, PCWSTR(service_name.as_ptr()), SERVICE_ALL_ACCESS)
            {
                let _ = DeleteService(service);
                let _ = CloseServiceHandle(service);
            }
            let _ = CloseServiceHandle(manager);
        }
    }
}
