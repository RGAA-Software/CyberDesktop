use gpui::{Bounds, Pixels, Window};
use raw_window_handle::RawWindowHandle;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, MoveWindow, ShowWindow, MA_NOACTIVATE, SW_HIDE, SW_SHOW,
    WM_MOUSEACTIVATE, WS_BORDER, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};

#[cfg(windows)]
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};

#[derive(Debug)]
pub struct NativeVideoSurface {
    #[cfg(windows)]
    hwnd: HWND,
}

#[cfg(windows)]
unsafe extern "system" fn video_surface_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> LRESULT {
    // Prevent the video surface from stealing focus on mouse clicks,
    // so keyboard events (e.g. Escape to exit fullscreen) keep routing
    // to the parent GPUI window.
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    DefSubclassProc(hwnd, msg, wparam, lparam)
}

impl NativeVideoSurface {
    #[cfg(windows)]
    pub fn new(parent_hwnd: isize) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                Default::default(),
                w!("STATIC"),
                w!("Cyber Media Player Video"),
                WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_BORDER,
                0,
                0,
                1,
                1,
                HWND(parent_hwnd as _),
                None,
                None,
                None,
            )
        }?;

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetWindowSubclass(hwnd, Some(video_surface_subclass_proc), 0, 0);
        }

        Ok(Self { hwnd })
    }

    #[cfg(not(windows))]
    pub fn new(_parent_hwnd: isize) -> anyhow::Result<Self> {
        anyhow::bail!("video embedding is only supported on Windows")
    }

    #[cfg(windows)]
    pub fn hwnd(&self) -> isize {
        self.hwnd.0 as isize
    }

    #[cfg(not(windows))]
    pub fn hwnd(&self) -> isize {
        0
    }

    pub fn set_visible(&self, visible: bool) {
        #[cfg(windows)]
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
        }
    }

    pub fn set_bounds(&self, window: &Window, bounds: Bounds<Pixels>) {
        #[cfg(windows)]
        {
            let scale = window.scale_factor();
            let left = (f32::from(bounds.origin.x) * scale).round() as i32;
            let top = (f32::from(bounds.origin.y) * scale).round() as i32;
            let right = ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale)
                .round() as i32;
            let bottom = ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * scale)
                .round() as i32;

            if right <= left || bottom <= top {
                self.set_visible(false);
                return;
            }

            unsafe {
                let _ = MoveWindow(self.hwnd, left, top, right - left, bottom - top, true);
            }
            self.set_visible(true);
        }
    }
}

impl Drop for NativeVideoSurface {
    fn drop(&mut self) {
        #[cfg(windows)]
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

pub fn window_hwnd(window: &Window) -> Option<isize> {
    let handle = raw_window_handle::HasWindowHandle::window_handle(window).ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(window) => Some(window.hwnd.get() as isize),
        _ => None,
    }
}
