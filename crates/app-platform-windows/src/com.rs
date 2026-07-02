//! COM threading modeled 1:1 after Files.app (`STATask` + `ThreadWithMessageQueue`).

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self};
use std::time::Duration;

use windows::core::HRESULT;
use windows::Win32::Foundation::{
    CloseHandle, HANDLE, HWND, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::System::Threading::{
    CreateThread, WaitForSingleObject, THREAD_CREATE_RUN_IMMEDIATELY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, PostMessageW,
    RegisterClassExW, TranslateMessage, UnregisterClassW, HWND_MESSAGE, MSG, WM_QUIT, WM_USER,
    WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSEXW,
};

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

/// Logs the current thread's COM apartment type for diagnostics.
pub fn log_current_apartment(label: &str) {
    use windows::Win32::System::Com::{
        CoGetApartmentType, APTTYPE, APTTYPEQUALIFIER, APTTYPE_MAINSTA, APTTYPE_MTA, APTTYPE_NA,
        APTTYPE_STA,
    };
    unsafe {
        let mut apt_type = APTTYPE::default();
        let mut apt_qualifier = APTTYPEQUALIFIER::default();
        let hr = CoGetApartmentType(&mut apt_type, &mut apt_qualifier);
        let type_name = match apt_type {
            APTTYPE_STA => "STA",
            APTTYPE_MTA => "MTA",
            APTTYPE_NA => "NA",
            APTTYPE_MAINSTA => "MAINSTA",
            _ => "UNKNOWN",
        };
        tracing::info!(
            target: "shell_menu",
            "COM apartment [{label}] hr={hr:?} type={type_name} qualifier={apt_qualifier:?}"
        );
    }
}

type Job = Box<dyn FnOnce() + Send>;

unsafe extern "system" fn sta_thread_proc(param: *mut std::ffi::c_void) -> u32 {
    if param.is_null() {
        return 0;
    }
    let closure = Box::from_raw(param as *mut Box<dyn FnOnce() + Send>);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(closure));
    0
}

/// Short-lived STA thread per call (Files `STATask`) — used for Shell **icons**.
pub fn run_sta_task<T, F>(f: F) -> T
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("cyber_desktop-sta-task".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
                let _ = OleInitialize(None);
                let out = f();
                let _ = OleUninitialize();
                out
            }));
            let _ = reply_tx.send(result);
        })
        .expect("spawn cyber_desktop-sta-task");
    match reply_rx.recv().expect("sta task reply") {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// A port of Files.app's `ThreadWithMessageQueue` that uses a raw Win32 worker thread.
///
/// Files runs each Shell context-menu session on a dedicated background `Thread` with
/// `ApartmentState.STA`, which consumes a `BlockingCollection` of jobs
/// (`foreach msg in GetConsumingEnumerable() { msg.payload(); }`) and runs each synchronously.
/// There is deliberately **no Win32 message loop** — Shell verbs are direct COM calls. On dispose
/// it `CompleteAdding()`s the queue and `Join()`s the thread.
///
/// We mirror the Files behavior (`OleInitialize`, mpsc job loop, bounded wait on drop) but we
/// intentionally spawn a **raw Win32 thread** instead of a `std::thread`. The reason is that a
/// wedged Shell extension can leave a worker thread in a state where `std::thread::join` blocks
/// process exit and can even hold the loader lock, keeping the test/application process alive.
/// A raw thread is invisible to the Rust runtime; dropping the queue sender and closing the
/// thread handle abandons the worker without stalling the caller.
#[derive(Clone, Copy)]
struct SendHandle(HANDLE);
unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}

pub struct ThreadWithMessageQueue {
    name: &'static str,
    job_tx: Option<Sender<Job>>,
    /// Raw OS thread handle. We intentionally use a Win32 thread instead of
    /// `std::thread` so that, if the worker wedges inside a Shell extension, the
    /// process is **not** kept alive by a leaked/joined Rust thread when the test
    /// harness or application exits.
    handle: Option<SendHandle>,
}

/// Max time `Drop` waits for the worker to finish its queue before abandoning a wedged thread.
const JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// Max time to wait for a freshly spawned pump thread to signal readiness. Thread start
/// requires the loader lock; a wedged Shell extension holding it blocks new threads forever.
const PUMP_READY_TIMEOUT: Duration = Duration::from_secs(3);

impl ThreadWithMessageQueue {
    pub fn new(name: &'static str) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        tracing::info!(target: "shell_menu", "STA[{name}]: spawning owning thread (Files ThreadWithMessageQueue)");

        let body: Box<dyn FnOnce() + Send> = Box::new(move || unsafe {
            // Files STATask: OleInitialize (implies CoInitializeEx STA). No message loop.
            let ole = OleInitialize(None);
            if ole.is_err() {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }
            tracing::info!(target: "shell_menu", "STA[{name}]: started; OleInitialize hr={ole:?}");
            // Consume jobs until the sender is dropped (Files: BlockingCollection.CompleteAdding).
            for job in job_rx.iter() {
                job();
            }
            if ole.is_ok() {
                OleUninitialize();
            }
            tracing::info!(target: "shell_menu", "STA[{name}]: queue completed; OleUninitialize and exit");
        });

        // Double-box so the trait-object fat pointer can be passed through a single
        // `*mut c_void`.
        let param = Box::into_raw(Box::new(body)) as *mut std::ffi::c_void;

        let handle = unsafe {
            CreateThread(
                None,
                0,
                Some(sta_thread_proc),
                Some(param as *const _), // cast to *const core::ffi::c_void
                THREAD_CREATE_RUN_IMMEDIATELY,
                None,
            )
        }
        .expect("spawn STA thread");

        Self {
            name,
            job_tx: Some(job_tx),
            handle: Some(SendHandle(handle)),
        }
    }

    /// Posts a job to the owning STA thread and gives up after `timeout`, returning `None`.
    ///
    /// A `None` result means the job is still running on the STA thread (e.g. a Shell
    /// extension deadlocked inside `QueryContextMenu`). The job keeps running, so the caller
    /// MUST abandon this thread (drop/forget it) instead of posting more work to it.
    pub fn post_with_timeout<F, T>(&self, f: F, timeout: Duration) -> Option<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let name = self.name;
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        tracing::info!(target: "shell_menu", "STA[{name}]: dispatching timed job (timeout={timeout:?})");
        self.dispatch(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = reply_tx.send(result);
        });
        match reply_rx.recv_timeout(timeout) {
            Ok(Ok(value)) => Some(value),
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(RecvTimeoutError::Timeout) => {
                tracing::warn!(target: "shell_menu", "STA[{name}]: TIMEOUT waiting for reply (job still running on STA thread)");
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                tracing::warn!(target: "shell_menu", "STA[{name}]: reply channel disconnected");
                None
            }
        }
    }

    /// Queue work without waiting for completion (caller waits on its own channel).
    fn dispatch<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(tx) = self.job_tx.as_ref() {
            let _ = tx.send(Box::new(f));
        }
    }

    /// Abandon a wedged worker without joining. Intentionally does **not** call
    /// `TerminateThread` — killing a thread inside Shell extension / loader code can
    /// crash the entire process (seen on Windows Server).
    pub fn abandon_wedged(mut self) {
        let name = self.name;
        self.job_tx.take();
        let _ = self.handle.take();
        std::mem::forget(self);
        tracing::warn!(target: "shell_menu", "STA[{name}]: wedged worker abandoned");
    }
}

impl Drop for ThreadWithMessageQueue {
    fn drop(&mut self) {
        // Files Dispose(): CompleteAdding() then Join(). Dropping the sender ends the consume loop
        // after the current job drains; then we wait on the raw OS handle — bounded, so a thread
        // wedged inside a Shell extension is abandoned rather than blocking the caller forever.
        // Because we spawned a Win32 thread (not a `std::thread`), abandoning it does not keep the
        // process alive when the test harness or application exits.
        self.job_tx.take();
        let Some(SendHandle(handle)) = self.handle.take() else {
            return;
        };
        let timeout_ms = JOIN_TIMEOUT.as_millis().min(u32::MAX as u128) as u32;
        let result = unsafe { WaitForSingleObject(handle, timeout_ms) };
        unsafe {
            let _ = CloseHandle(handle);
        }
        if result != WAIT_OBJECT_0 {
            tracing::warn!(
                target: "shell_menu",
                "STA[{}]: worker did not finish within {JOIN_TIMEOUT:?}; abandoning wedged thread",
                self.name
            );
        }
    }
}

/// A STA thread with a real Win32 message pump, required for Shell extensions that
/// SendMessage/marshal COM calls during `QueryContextMenu`.
///
/// Jobs are posted as window messages to a message-only window on the pump thread, mirroring
/// WinForms/WPF Dispatcher behavior.
const WM_USER_JOB: u32 = WM_USER + 1;

/// Per-thread state for the pumping STA worker.
struct PumpState {
    hwnd: HWND,
}

unsafe impl Send for PumpState {}

pub struct ThreadWithMessageQueueWithPump {
    name: &'static str,
    hwnd: HWND,
    job_tx: Option<Sender<Job>>,
    handle: Option<SendHandle>,
    degraded: bool,
}

// HWND is a raw pointer, but this type only uses it to post messages to the thread that owns it.
// The owning thread is the only one that calls GetMessage/DispatchMessage on this window.
unsafe impl Send for ThreadWithMessageQueueWithPump {}

impl ThreadWithMessageQueueWithPump {
    pub fn new(name: &'static str) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (ready_tx, ready_rx) = mpsc::channel::<PumpState>();
        tracing::info!(target: "shell_menu", "STA-PUMP[{name}]: spawning owning thread");

        let body: Box<dyn FnOnce() + Send> = Box::new(move || unsafe {
            let ole = OleInitialize(None);
            if ole.is_err() {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            }

            // Register a window class and create a message-only window so jobs arrive as window
            // messages (mirrors WinForms/WPF Dispatcher.Invoke/BeginInvoke behavior).
            let hinstance =
                windows::Win32::System::LibraryLoader::GetModuleHandleW(None).unwrap_or_default();
            let class_name: Vec<u16> = format!("CyberDesktopPump_{}\0", name)
                .encode_utf16()
                .collect();
            let wndclass = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(pump_wndproc),
                hInstance: hinstance.into(),
                lpszClassName: windows::core::PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            let atom = RegisterClassExW(&wndclass);
            let hwnd = if atom != 0 {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    windows::core::PCWSTR(class_name.as_ptr()),
                    windows::core::PCWSTR(class_name.as_ptr()),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    HWND_MESSAGE,
                    None,
                    hinstance,
                    None,
                )
                .unwrap_or_default()
            } else {
                Default::default()
            };

            tracing::info!(target: "shell_menu", "STA-PUMP[{name}]: started hwnd={hwnd:?}");
            let _ = ready_tx.send(PumpState { hwnd });

            // Classic message pump. WM_USER_JOB is handled inside the WNDPROC so that
            // SendMessage calls from Shell extensions re-enter the same window procedure.
            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            // Drain any channel jobs posted during shutdown.
            while let Ok(job) = job_rx.try_recv() {
                job();
            }

            let _ = DestroyWindow(hwnd);
            if atom != 0 {
                let _ = UnregisterClassW(windows::core::PCWSTR(class_name.as_ptr()), hinstance);
            }
            if ole.is_ok() {
                let _ = OleUninitialize();
            }
            tracing::info!(target: "shell_menu", "STA-PUMP[{name}]: queue completed; exit");
        });

        let param = Box::into_raw(Box::new(body)) as *mut std::ffi::c_void;
        let handle = unsafe {
            CreateThread(
                None,
                0,
                Some(sta_thread_proc),
                Some(param as *const _),
                THREAD_CREATE_RUN_IMMEDIATELY,
                None,
            )
        }
        .expect("spawn STA pump thread");

        // Bounded: if a previously wedged Shell extension holds the loader lock, the new
        // thread never reaches its entry point and an unbounded recv would freeze the
        // caller forever. Degrade to a dead queue instead — post_with_timeout returns None.
        let state = match ready_rx.recv_timeout(PUMP_READY_TIMEOUT) {
            Ok(state) => state,
            Err(_) => {
                tracing::warn!(
                    target: "shell_menu",
                    "STA-PUMP[{name}]: worker did not start within {PUMP_READY_TIMEOUT:?} \
                     (loader lock likely held by a wedged extension); operating degraded"
                );
                PumpState {
                    hwnd: HWND::default(),
                }
            }
        };

        Self {
            name,
            hwnd: state.hwnd,
            job_tx: Some(job_tx),
            handle: Some(SendHandle(handle)),
            degraded: state.hwnd.is_invalid(),
        }
    }

    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    pub fn post_with_timeout<F, T>(&self, f: F, timeout: Duration) -> Option<T>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let name = self.name;
        if self.hwnd.is_invalid() {
            tracing::warn!(target: "shell_menu", "STA-PUMP[{name}]: no pump window (degraded); dropping job");
            return None;
        }
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        tracing::info!(target: "shell_menu", "STA-PUMP[{name}]: dispatching timed job (timeout={timeout:?})");

        let job: Job = Box::new(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = reply_tx.send(result);
        });
        let job_ptr = Box::into_raw(Box::new(job)) as usize;

        unsafe {
            let _ = PostMessageW(self.hwnd, WM_USER_JOB, WPARAM(job_ptr), LPARAM(0));
        }

        match reply_rx.recv_timeout(timeout) {
            Ok(Ok(value)) => Some(value),
            Ok(Err(payload)) => std::panic::resume_unwind(payload),
            Err(RecvTimeoutError::Timeout) => {
                tracing::warn!(target: "shell_menu", "STA-PUMP[{name}]: TIMEOUT waiting for reply");
                None
            }
            Err(RecvTimeoutError::Disconnected) => {
                tracing::warn!(target: "shell_menu", "STA-PUMP[{name}]: reply channel disconnected");
                None
            }
        }
    }

    /// Abandon a wedged worker without joining. Intentionally does **not** call
    /// `TerminateThread` — killing a thread inside Shell extension / loader code can
    /// crash the entire process (seen on Windows Server).
    pub fn abandon_wedged(mut self) {
        let name = self.name;
        self.job_tx.take();
        let _ = self.handle.take();
        std::mem::forget(self);
        tracing::warn!(target: "shell_menu", "STA-PUMP[{name}]: wedged worker abandoned");
    }
}

impl Drop for ThreadWithMessageQueueWithPump {
    fn drop(&mut self) {
        self.job_tx.take();
        if !self.hwnd.is_invalid() {
            unsafe {
                let _ = PostMessageW(self.hwnd, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        let Some(SendHandle(handle)) = self.handle.take() else {
            return;
        };
        let timeout_ms = JOIN_TIMEOUT.as_millis().min(u32::MAX as u128) as u32;
        let result = unsafe { WaitForSingleObject(handle, timeout_ms) };
        unsafe {
            let _ = CloseHandle(handle);
        }
        if result != WAIT_OBJECT_0 {
            tracing::warn!(
                target: "shell_menu",
                "STA-PUMP[{}]: worker did not finish within {JOIN_TIMEOUT:?}; abandoning wedged thread",
                self.name
            );
        }
    }
}

unsafe extern "system" fn pump_wndproc(
    _hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    _lparam: LPARAM,
) -> LRESULT {
    if msg == WM_USER_JOB {
        if wparam.0 == 0 {
            return LRESULT(0);
        }
        let job = Box::from_raw(wparam.0 as *mut Job);
        job();
        return LRESULT(0);
    }
    DefWindowProcW(_hwnd, msg, wparam, _lparam)
}

/// Terminate the process immediately, skipping DLL `DllMain(DETACH)` callbacks and Rust
/// destructors. `std::process::exit` → `ExitProcess` acquires the loader lock; when a Shell
/// extension thread has wedged while holding it, normal exit deadlocks and the process
/// lingers as a zombie. Flushes stdio first so diagnostic output is not lost.
pub fn hard_exit_process(code: u32) -> ! {
    use std::io::Write;
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();
    unsafe {
        use windows::Win32::System::Threading::{GetCurrentProcess, TerminateProcess};
        let _ = TerminateProcess(GetCurrentProcess(), code);
    }
    // TerminateProcess does not return on success; this is unreachable in practice.
    std::process::exit(code as i32)
}

/// Ensures COM is initialized on the **current** thread (MTA). Use on dedicated worker threads (e.g. audio).
pub fn ensure_com_multithreaded() {
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
    unsafe {
        let hr = CoInitializeEx(None, COINIT_MULTITHREADED);
        if hr.is_ok() || hr == RPC_E_CHANGED_MODE {
            Ok(())
        } else {
            Err(anyhow::anyhow!("CoInitializeEx(MTA) failed: {hr:?}"))
        }
    }
    .ok();
}

/// Ensures COM is initialized on the **current** thread (STA).
pub fn ensure_com_apartment() -> anyhow::Result<()> {
    unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_ok() || hr == RPC_E_CHANGED_MODE {
            Ok(())
        } else {
            anyhow::bail!("CoInitializeEx failed: {hr:?}");
        }
    }
}
