//! COM threading modeled after Files.app (`STATask` + `ThreadWithMessageQueue`).

use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};

use windows::core::HRESULT;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

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

/// Long-lived STA worker with a job queue (Files `ThreadWithMessageQueue`) — used for **context menus**.
pub struct StaMessageThread {
    job_tx: SyncSender<Box<dyn FnOnce() + Send>>,
    join: Option<JoinHandle<()>>,
}

impl StaMessageThread {
    pub fn new(name: &'static str) -> Self {
        let (job_tx, job_rx) = mpsc::sync_channel::<Box<dyn FnOnce() + Send>>(128);
        let join = thread::Builder::new()
            .name(name.into())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                for job in job_rx {
                    job();
                }
                let _ = CoUninitialize();
            })
            .expect("spawn STA message thread");
        Self {
            job_tx,
            join: Some(join),
        }
    }

    pub fn post<F, T>(&self, f: F) -> T
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        self.dispatch(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = reply_tx.send(result);
        });
        match reply_rx.recv().expect("STA thread job reply") {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Queue work without waiting for completion (caller waits on its own channel).
    pub fn dispatch<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.job_tx
            .send(Box::new(f))
            .expect("STA thread job queue full");
    }
}

impl Drop for StaMessageThread {
    fn drop(&mut self) {
        // Do not join: the STA thread may be blocked inside Shell COM (Files uses background threads).
        if let Some(join) = self.join.take() {
            std::mem::forget(join);
        }
    }
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
