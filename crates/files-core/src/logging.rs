use std::path::PathBuf;
use std::sync::OnceLock;

use rolling_file::{BasicRollingFileAppender, RollingConditionBasic};
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_log::LogTracer;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

use crate::config::config_path;

const LOG_FILE_SIZE_BYTES: u64 = 32 * 1024 * 1024;
const LOG_FILE_COUNT_TOTAL: usize = 7;

static LOG_GUARDS: OnceLock<TracingGuards> = OnceLock::new();

struct TracingGuards {
    _file_guard: WorkerGuard,
}

fn log_dir() -> Option<PathBuf> {
    let config_path = config_path()?;
    Some(config_path.parent()?.join("logs"))
}

fn historical_log_file_count() -> usize {
    LOG_FILE_COUNT_TOTAL.saturating_sub(1)
}

fn default_env_filter() -> EnvFilter {
    EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy()
}

pub fn init_tracing(app_name: &str) {
    let _ = LogTracer::init();

    let env_filter = default_env_filter();
    let terminal_layer = fmt::layer()
        .with_ansi(true)
        .with_target(true)
        .with_thread_names(true)
        .with_file(false)
        .with_line_number(false)
        .with_writer(std::io::stderr);

    if let Some((file_writer, file_guard)) = build_file_writer(app_name) {
        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_writer(file_writer);
        let subscriber = Registry::default()
            .with(env_filter)
            .with(terminal_layer)
            .with(file_layer);
        let _ = tracing::subscriber::set_global_default(subscriber);
        let _ = LOG_GUARDS.set(TracingGuards {
            _file_guard: file_guard,
        });
        return;
    }

    let subscriber = Registry::default().with(env_filter).with(terminal_layer);
    let _ = tracing::subscriber::set_global_default(subscriber);
}

fn build_file_writer(app_name: &str) -> Option<(NonBlocking, WorkerGuard)> {
    let log_dir = log_dir()?;
    std::fs::create_dir_all(&log_dir).ok()?;

    let rolling = BasicRollingFileAppender::new(
        log_dir.join(format!("{app_name}.log")),
        RollingConditionBasic::new().max_size(LOG_FILE_SIZE_BYTES),
        historical_log_file_count(),
    )
    .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(rolling);

    Some((writer, guard))
}
