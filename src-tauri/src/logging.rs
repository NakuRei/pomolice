use std::fs;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{InitError, RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

pub(crate) const LOG_FILENAME_PREFIX: &str = "pomolice";
pub(crate) const LOG_FILENAME_SUFFIX: &str = "log";
pub(crate) const MAX_LOG_FILES: usize = 8;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LoggingInitError {
    #[error("failed to resolve app log directory: {0}")]
    LogDirResolution(String),
    #[error("failed to build rolling file appender: {0}")]
    AppenderInit(#[from] InitError),
    #[error("failed to initialize global tracing subscriber: {0}")]
    SubscriberInit(String),
}

pub(crate) struct LoggingRuntime {
    _file_guard: WorkerGuard,
}

fn init_log_tracer() {
    let _ = tracing_log::LogTracer::init();
}

fn create_env_filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

fn build_rolling_file_appender(
    log_directory: &Path,
) -> Result<RollingFileAppender, LoggingInitError> {
    Ok(tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILENAME_PREFIX)
        .filename_suffix(LOG_FILENAME_SUFFIX)
        .max_log_files(MAX_LOG_FILES)
        .build(log_directory)?)
}

fn should_enable_stdout_layer() -> bool {
    should_enable_stdout_layer_for(cfg!(debug_assertions))
}

pub(crate) fn should_enable_stdout_layer_for(debug_assertions_enabled: bool) -> bool {
    debug_assertions_enabled
}

fn resolve_log_directory<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, LoggingInitError> {
    let log_directory = app
        .path()
        .app_log_dir()
        .map_err(|error| LoggingInitError::LogDirResolution(error.to_string()))?;

    fs::create_dir_all(&log_directory)
        .map_err(|error| LoggingInitError::LogDirResolution(error.to_string()))?;

    Ok(log_directory)
}

fn init_logging_with_directory(
    log_directory: &Path,
    stdout_enabled: bool,
) -> Result<LoggingRuntime, LoggingInitError> {
    let file_appender = build_rolling_file_appender(log_directory)?;
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    init_log_tracer();

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_file(false)
        .with_line_number(false)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_writer(file_writer);

    let stdout_layer = stdout_enabled.then(|| {
        tracing_subscriber::fmt::layer()
            .with_file(false)
            .with_line_number(false)
            .with_target(true)
            .with_thread_ids(false)
            .with_thread_names(false)
            .with_writer(std::io::stdout)
    });

    let subscriber = Registry::default()
        .with(create_env_filter())
        .with(file_layer)
        .with(stdout_layer);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|error| LoggingInitError::SubscriberInit(error.to_string()))?;

    Ok(LoggingRuntime {
        _file_guard: file_guard,
    })
}

pub(crate) fn init_logging<R: tauri::Runtime>(
    app: &AppHandle<R>,
) -> Result<LoggingRuntime, LoggingInitError> {
    let log_directory = resolve_log_directory(app)?;
    init_logging_with_directory(&log_directory, should_enable_stdout_layer())
}

pub(crate) fn build_log_plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_log::Builder::new().skip_logger().build()
}

#[cfg(test)]
pub(crate) fn build_file_appender_for_test(
    log_directory: &Path,
) -> Result<RollingFileAppender, LoggingInitError> {
    build_rolling_file_appender(log_directory)
}

#[cfg(test)]
pub(crate) fn init_logging_for_test(
    log_directory: &Path,
    stdout_enabled: bool,
) -> Result<LoggingRuntime, LoggingInitError> {
    init_logging_with_directory(log_directory, stdout_enabled)
}
