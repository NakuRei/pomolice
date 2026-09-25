use log::{Level, LevelFilter, Metadata};
use tauri_plugin_log::{RotationStrategy, TimezoneStrategy, WEBVIEW_TARGET};

use crate::external_adapters::logging::{
    default_log_level, max_log_file_size_bytes, resolve_log_target_specs, rotation_strategy,
    should_log, timezone_strategy, LogTargetSpec,
};

fn metadata(target: &'static str) -> Metadata<'static> {
    Metadata::builder()
        .level(Level::Info)
        .target(target)
        .build()
}

fn assert_log_target_specs_include_only(
    actual_specs: &[LogTargetSpec],
    expected_specs: &[LogTargetSpec],
) {
    assert_eq!(
        actual_specs.len(),
        expected_specs.len(),
        "logging targets should not include unexpected destinations"
    );
    for expected_spec in expected_specs {
        assert!(
            actual_specs.contains(expected_spec),
            "logging targets should include {expected_spec:?}"
        );
    }
}

#[test]
fn test_resolve_log_target_specs_returns_dev_stdout_webview_logdir() {
    let specs = resolve_log_target_specs(true, None);

    assert_log_target_specs_include_only(
        &specs,
        &[
            LogTargetSpec::Stdout,
            LogTargetSpec::Webview,
            LogTargetSpec::LogDir,
        ],
    );
}

#[test]
fn test_resolve_log_target_specs_returns_prod_logdir_only() {
    let specs = resolve_log_target_specs(false, None);

    assert_log_target_specs_include_only(&specs, &[LogTargetSpec::LogDir]);
}

#[test]
fn test_resolve_log_target_specs_uses_folder_for_prod_when_directory_is_provided() {
    let log_directory = std::env::temp_dir().join("pomolice-logging-config-test");
    let specs = resolve_log_target_specs(false, Some(log_directory.as_path()));

    assert_log_target_specs_include_only(
        &specs,
        &[LogTargetSpec::Folder {
            path: log_directory,
        }],
    );
}

#[test]
fn test_resolve_log_target_specs_uses_folder_for_dev_when_directory_is_provided() {
    let log_directory = std::env::temp_dir().join("pomolice-logging-config-test-dev");
    let specs = resolve_log_target_specs(true, Some(log_directory.as_path()));

    assert_log_target_specs_include_only(
        &specs,
        &[
            LogTargetSpec::Stdout,
            LogTargetSpec::Webview,
            LogTargetSpec::Folder {
                path: log_directory,
            },
        ],
    );
}

#[test]
fn test_default_logging_policy_keeps_operational_logs_bounded_and_readable() {
    assert_eq!(default_log_level(), LevelFilter::Info);
    assert!(matches!(timezone_strategy(), TimezoneStrategy::UseLocal));
    assert!(max_log_file_size_bytes() > 0);
    assert!(matches!(
        rotation_strategy(),
        RotationStrategy::KeepSome(count) if count > 0
    ));
}

#[test]
fn test_should_log_allows_app_targets_and_filters_noisy_dependencies() {
    assert!(should_log(&metadata("pomolice_lib::timer_core")));
    assert!(should_log(&metadata(WEBVIEW_TARGET)));
    assert!(!should_log(&metadata("rdev::listen")));
    assert!(!should_log(&metadata("hyper::client")));
}
