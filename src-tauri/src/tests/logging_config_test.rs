use log::{Level, LevelFilter, Metadata};
use tauri_plugin_log::{RotationStrategy, TimezoneStrategy, WEBVIEW_TARGET};

use crate::logging::{
    default_log_level, max_log_file_size_bytes, module_log_overrides, resolve_log_target_specs,
    rotation_strategy, should_log, timezone_strategy, LogTargetSpec,
};

fn metadata(target: &'static str) -> Metadata<'static> {
    Metadata::builder()
        .level(Level::Info)
        .target(target)
        .build()
}

#[test]
fn test_resolve_log_target_specs_returns_dev_stdout_webview_logdir() {
    let specs = resolve_log_target_specs(true, None);

    assert_eq!(
        specs,
        vec![
            LogTargetSpec::Stdout,
            LogTargetSpec::Webview,
            LogTargetSpec::LogDir,
        ]
    );
}

#[test]
fn test_resolve_log_target_specs_returns_prod_logdir_only() {
    let specs = resolve_log_target_specs(false, None);

    assert_eq!(specs, vec![LogTargetSpec::LogDir]);
}

#[test]
fn test_resolve_log_target_specs_uses_folder_when_directory_is_provided() {
    let log_directory = std::env::temp_dir().join("pomolice-logging-config-test");
    let specs = resolve_log_target_specs(false, Some(log_directory.as_path()));

    assert_eq!(
        specs,
        vec![LogTargetSpec::Folder {
            path: log_directory,
        }]
    );
}

#[test]
fn test_default_log_level_and_rotation_policy_are_fixed() {
    assert_eq!(default_log_level(), LevelFilter::Info);
    assert_eq!(max_log_file_size_bytes(), 32 * 1024);
    assert!(matches!(rotation_strategy(), RotationStrategy::KeepSome(5)));
    assert!(matches!(timezone_strategy(), TimezoneStrategy::UseLocal));
}

#[test]
fn test_module_log_overrides_include_expected_noisy_dependencies() {
    let overrides = module_log_overrides();

    assert!(overrides.contains(&("pomolice_lib::managers::input_monitor", LevelFilter::Debug)));
    assert!(overrides.contains(&("pomolice_lib::scheduler", LevelFilter::Debug)));
    assert!(overrides.contains(&("rdev", LevelFilter::Warn)));
}

#[test]
fn test_should_log_allows_app_targets_and_filters_noisy_dependencies() {
    assert!(should_log(&metadata("pomolice_lib::scheduler")));
    assert!(should_log(&metadata(WEBVIEW_TARGET)));
    assert!(!should_log(&metadata("rdev::listen")));
    assert!(!should_log(&metadata("hyper::client")));
}

#[test]
fn test_build_log_plugin_for_directory_is_available_for_wry_runtime() {
    let _factory = crate::logging::build_log_plugin_for_directory::<tauri::Wry>;
}

#[test]
fn test_build_log_plugin_for_directory_function_is_callable_for_wry_runtime() {
    let _factory = crate::logging::build_log_plugin_for_directory::<tauri::Wry>;
}
