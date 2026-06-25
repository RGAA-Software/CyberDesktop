//! COM threading modeled 1:1 after Files.app (`STATask` + `ThreadWithMessageQueue`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::core::HRESULT;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x80010106u32 as i32);

type Job = Box<dyn FnOnce() + Send>;

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

/// A 1:1 port of Files.app's `ThreadWithMessageQueue`.
///
/// Files runs each Shell context-menu session on a dedicated background `Thread` with
/// `ApartmentState.STA`, which consumes a `BlockingCollection` of jobs
/// (`foreach msg in GetConsumingEnumerable() { msg.payload(); }`) and runs each synchronously.
/// There is deliberately **no Win32 message loop** — Shell verbs are direct COM calls. On dispose
/// it `CompleteAdding()`s the queue and `Join()`s the thread.
///
/// We mirror that exactly: `OleInitialize` on the thread, a blocking job-consume loop fed by an
/// mpsc channel, and a `Drop` that drops the sender (== `CompleteAdding`) then joins. The only
/// addition is that the join is *bounded*: if the worker is wedged inside a misbehaving Shell
/// extension we abandon (leak) it rather than block forever (Files relies on warm-up to avoid
/// ever hitting that case; we keep a safety net).
pub struct ThreadWithMessageQueue {
    name: &'static str,
    job_tx: Option<Sender<Job>>,
    finished: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

/// Max time `Drop` waits for the worker to finish its queue before abandoning a wedged thread.
const JOIN_TIMEOUT: Duration = Duration::from_secs(2);

impl ThreadWithMessageQueue {
    pub fn new(name: &'static str) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let finished = Arc::new(AtomicBool::new(false));
        let finished_worker = finished.clone();
        tracing::info!(target: "shell_menu", "STA[{name}]: spawning owning thread (Files ThreadWithMessageQueue)");
        let join = thread::Builder::new()
            .name(name.into())
            .spawn(move || unsafe {
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
                finished_worker.store(true, Ordering::SeqCst);
                tracing::info!(target: "shell_menu", "STA[{name}]: queue completed; OleUninitialize and exit");
            })
            .expect("spawn STA thread");
        Self {
            name,
            job_tx: Some(job_tx),
            finished,
            join: Some(join),
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
}

impl Drop for ThreadWithMessageQueue {
    fn drop(&mut self) {
        // Files Dispose(): CompleteAdding() then Join(). Dropping the sender ends the consume loop
        // after the current job drains; then we join — but bounded, so a thread wedged inside a
        // Shell extension is abandoned (leaked) rather than blocking the caller forever.
        self.job_tx.take();
        let Some(join) = self.join.take() else {
            return;
        };
        let deadline = Instant::now() + JOIN_TIMEOUT;
        while !self.finished.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                tracing::warn!(
                    target: "shell_menu",
                    "STA[{}]: worker did not finish within {JOIN_TIMEOUT:?}; abandoning (leaking) wedged thread",
                    self.name
                );
                std::mem::forget(join);
                return;
            }
            thread::sleep(Duration::from_millis(5));
        }
        let _ = join.join();
    }
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
