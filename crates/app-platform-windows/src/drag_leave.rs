//! One-shot WM_MOUSELEAVE watch for native drag-out (no per-frame GPUI listener registration).

use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TrackMouseEvent, TME_LEAVE, TRACKMOUSEEVENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetAncestor, SetWindowsHookExW, UnhookWindowsHookEx, GA_ROOT, HC_ACTION,
    HHOOK, MSG, WH_CALLWNDPROC,
};

use crate::drag_out::root_window_hwnd;

static TARGET_HWND: AtomicIsize = AtomicIsize::new(0);
static HOOK: Mutex<isize> = Mutex::new(0);
static LEAVE_PENDING: AtomicBool = AtomicBool::new(false);

fn disarm_hook() {
    TARGET_HWND.store(0, Ordering::SeqCst);
    if let Ok(mut hook) = HOOK.lock() {
        if *hook != 0 {
            unsafe {
                let _ = UnhookWindowsHookEx(HHOOK(*hook as _));
            }
            *hook = 0;
        }
    }
}

unsafe fn hwnd_matches_target(msg_hwnd: HWND, target: isize) -> bool {
    if target == 0 {
        return false;
    }
    let msg = msg_hwnd.0 as isize;
    if msg == target {
        return true;
    }
    let root = GetAncestor(msg_hwnd, GA_ROOT);
    !root.0.is_null() && root.0 as isize == target
}

unsafe fn track_leave(hwnd: isize) {
    let _ = TrackMouseEvent(&mut TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: HWND(hwnd as _),
        dwHoverTime: 0,
    });
}

unsafe extern "system" fn call_wnd_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let msg = &*(lparam.0 as *const MSG);
        let target = TARGET_HWND.load(Ordering::SeqCst);
        if msg.message == WM_MOUSELEAVE && hwnd_matches_target(msg.hwnd, target) {
            LEAVE_PENDING.store(true, Ordering::SeqCst);
            track_leave(target);
        }
    }
    let hook = HOOK.lock().map(|h| *h).unwrap_or(0);
    CallNextHookEx(
        if hook != 0 {
            HHOOK(hook as _)
        } else {
            HHOOK::default()
        },
        code,
        wparam,
        lparam,
    )
}

/// (Re)arms WM_MOUSELEAVE for `hwnd` — safe to call every drag_move while dragging.
pub fn arm_native_drag_leave(hwnd: isize) {
    let root = root_window_hwnd(hwnd);
    if root == 0 {
        return;
    }

    TARGET_HWND.store(root, Ordering::SeqCst);

    if let Ok(mut hook) = HOOK.lock() {
        if *hook == 0 {
            unsafe {
                if let Ok(h) = SetWindowsHookExW(WH_CALLWNDPROC, Some(call_wnd_proc), None, 0) {
                    *hook = h.0 as isize;
                }
            }
        }
    }

    unsafe {
        track_leave(root);
    }
}

/// Cancels any pending native drag leave watch.
pub fn disarm_native_drag_leave() {
    disarm_hook();
    LEAVE_PENDING.store(false, Ordering::SeqCst);
}

/// Returns true once after the pointer left the armed window (polled from the UI frame loop).
pub fn take_native_drag_leave_pending() -> bool {
    LEAVE_PENDING.swap(false, Ordering::SeqCst)
}
