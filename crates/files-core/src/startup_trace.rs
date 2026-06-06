use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static PROCESS_START: OnceLock<Instant> = OnceLock::new();
static LAST_STEP: Mutex<Option<Instant>> = Mutex::new(None);

/// Marks process start; call once at the very beginning of `main`.
pub fn mark_process_start() {
    let start = Instant::now();
    let _ = PROCESS_START.set(start);
    *LAST_STEP.lock().expect("startup trace mutex poisoned") = Some(start);
    tracing::info!(
        target: "startup",
        step = "process_start",
        total_ms = 0.0,
        delta_ms = 0.0
    );
}

/// Logs a startup milestone with elapsed time since process start and since the previous step.
pub fn log_startup_step(step: &'static str) {
    let now = Instant::now();
    let total_ms = PROCESS_START
        .get()
        .map(|start| now.duration_since(*start).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    let delta_ms = {
        let mut last = LAST_STEP.lock().expect("startup trace mutex poisoned");
        let delta = last
            .map(|prev| now.duration_since(prev).as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        *last = Some(now);
        delta
    };
    tracing::info!(target: "startup", step, total_ms, delta_ms);
}

/// Runs `f` and logs how long the block took, plus cumulative startup time.
pub fn time_startup_step<T>(step: &'static str, f: impl FnOnce() -> T) -> T {
    let block_start = Instant::now();
    let result = f();
    let block_ms = block_start.elapsed().as_secs_f64() * 1000.0;
    let now = Instant::now();
    let total_ms = PROCESS_START
        .get()
        .map(|start| now.duration_since(*start).as_secs_f64() * 1000.0)
        .unwrap_or(0.0);
    *LAST_STEP.lock().expect("startup trace mutex poisoned") = Some(now);
    tracing::info!(target: "startup", step, block_ms, total_ms);
    result
}
