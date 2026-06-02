/// stderr logging for Info pane audio (flush immediately for hang diagnosis).
#[macro_export]
macro_rules! audio_log {
    ($($t:tt)*) => {{
        tracing::debug!(target: "audio", "{}", format!($($t)*));
    }};
}

pub(crate) use audio_log;
