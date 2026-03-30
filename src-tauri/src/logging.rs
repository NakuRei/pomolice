use std::path::{Path, PathBuf};

use chrono::{Local, Utc};
use tauri::{plugin::TauriPlugin, Runtime};
use tauri_plugin_log::{
    log::{LevelFilter, Metadata},
    RotationStrategy, Target, TargetKind, TimezoneStrategy, WEBVIEW_TARGET,
};

const DEFAULT_MAX_FILE_SIZE_BYTES: u128 = 32 * 1024;
const DEFAULT_ROTATION_KEEP_COUNT: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LogTargetSpec {
    Stdout,
    Webview,
    LogDir,
    Folder { path: PathBuf },
}

pub(crate) fn build_log_plugin<R: Runtime>() -> TauriPlugin<R> {
    build_log_builder(cfg!(debug_assertions), None).build()
}

#[cfg(test)]
pub(crate) fn build_log_plugin_for_directory<R: Runtime>(
    log_directory: &Path,
    is_dev: bool,
) -> TauriPlugin<R> {
    build_log_builder(is_dev, Some(log_directory)).build()
}

pub(crate) fn build_log_builder(
    is_dev: bool,
    log_directory: Option<&Path>,
) -> tauri_plugin_log::Builder {
    let mut builder = tauri_plugin_log::Builder::new()
        .targets(resolve_log_targets(is_dev, log_directory))
        .level(default_log_level())
        .max_file_size(max_log_file_size_bytes())
        .rotation_strategy(rotation_strategy())
        .timezone_strategy(timezone_strategy())
        .filter(should_log)
        .format(|out, message, record| {
            let timestamp_strategy = timezone_strategy();
            out.finish(format_args!(
                "{} [{}][{}] {}",
                render_timestamp(&timestamp_strategy),
                record.target(),
                record.level(),
                message
            ));
        });

    for (module, level) in module_log_overrides() {
        builder = builder.level_for(*module, *level);
    }

    builder
}

#[cfg(test)]
pub(crate) fn build_log_builder_for_directory(
    log_directory: &Path,
    is_dev: bool,
) -> tauri_plugin_log::Builder {
    build_log_builder(is_dev, Some(log_directory))
}

pub(crate) fn resolve_log_target_specs(
    is_dev: bool,
    log_directory: Option<&Path>,
) -> Vec<LogTargetSpec> {
    let mut specs = Vec::new();
    if is_dev {
        specs.push(LogTargetSpec::Stdout);
        specs.push(LogTargetSpec::Webview);
    }

    match log_directory {
        Some(path) => specs.push(LogTargetSpec::Folder {
            path: path.to_path_buf(),
        }),
        None => specs.push(LogTargetSpec::LogDir),
    }

    specs
}

pub(crate) fn resolve_log_targets(is_dev: bool, log_directory: Option<&Path>) -> Vec<Target> {
    resolve_log_target_specs(is_dev, log_directory)
        .into_iter()
        .map(|spec| match spec {
            LogTargetSpec::Stdout => Target::new(TargetKind::Stdout),
            LogTargetSpec::Webview => Target::new(TargetKind::Webview),
            LogTargetSpec::LogDir => Target::new(TargetKind::LogDir { file_name: None }),
            LogTargetSpec::Folder { path } => Target::new(TargetKind::Folder {
                path,
                file_name: None,
            }),
        })
        .collect()
}

pub(crate) fn default_log_level() -> LevelFilter {
    LevelFilter::Info
}

pub(crate) fn module_log_overrides() -> &'static [(&'static str, LevelFilter)] {
    &[
        ("pomolice_lib::managers::input_monitor", LevelFilter::Debug),
        ("pomolice_lib::scheduler", LevelFilter::Debug),
        ("rdev", LevelFilter::Warn),
    ]
}

pub(crate) fn max_log_file_size_bytes() -> u128 {
    DEFAULT_MAX_FILE_SIZE_BYTES
}

pub(crate) fn rotation_strategy() -> RotationStrategy {
    RotationStrategy::KeepSome(DEFAULT_ROTATION_KEEP_COUNT)
}

pub(crate) fn timezone_strategy() -> TimezoneStrategy {
    TimezoneStrategy::UseLocal
}

pub(crate) fn should_log(metadata: &Metadata<'_>) -> bool {
    let target = metadata.target();
    if target == WEBVIEW_TARGET || target.starts_with("pomolice") {
        return true;
    }

    ![
        "rdev",
        "mio",
        "wry",
        "tao",
        "hyper",
        "reqwest",
        "tokio",
        "tray_icon",
    ]
    .iter()
    .any(|noisy_target_prefix| target.starts_with(noisy_target_prefix))
}

fn render_timestamp(timezone_strategy: &TimezoneStrategy) -> String {
    match timezone_strategy {
        TimezoneStrategy::UseUtc => Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        TimezoneStrategy::UseLocal => Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string(),
    }
}
