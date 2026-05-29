//! Telemetry no-op when `collab-runtime` is disabled.

#[macro_export]
macro_rules! collab_telemetry {
    ($($t:tt)*) => {
        let _ = ();
    };
}
