use gpui::{Bounds, Pixels, Window};
use raw_window_handle::RawWindowHandle;

#[cfg(windows)]
use windows::core::w;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, MoveWindow, ShowWindow, SW_HIDE, SW_SHOW, WS_BORDER, WS_CHILD,
    WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
};

#[derive(Debug)]
pub struct NativeVideoSurface {
    #[cfg(windows)]
    hwnd: HWND,
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
            let right =
                ((f32::from(bounds.origin.x) + f32::from(bounds.size.width)) * scale).round()
                    as i32;
            let bottom =
                ((f32::from(bounds.origin.y) + f32::from(bounds.size.height)) * scale).round()
                    as i32;

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
