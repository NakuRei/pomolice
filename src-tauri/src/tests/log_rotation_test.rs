use std::path::PathBuf;

use tauri_plugin_log::RotationStrategy;

use crate::logging::{build_log_builder_for_directory, resolve_log_target_specs, LogTargetSpec};

fn unique_log_directory(test_name: &str) -> PathBuf {
    let timestamp_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();

    std::env::temp_dir().join(format!(
        "pomolice-rotation-{test_name}-{pid}-{timestamp_nanos}",
        pid = std::process::id(),
    ))
}

#[test]
fn test_rotation_helper_uses_temp_directory_folder_target_and_fixed_rotation_policy() {
    let log_directory = unique_log_directory("config");
    let builder = build_log_builder_for_directory(&log_directory, false);
    let specs = resolve_log_target_specs(false, Some(log_directory.as_path()));

    assert_eq!(
        specs,
        vec![LogTargetSpec::Folder {
            path: log_directory.clone(),
        }]
    );
    assert!(matches!(
        crate::logging::rotation_strategy(),
        RotationStrategy::KeepSome(5)
    ));

    let _builder = builder;
}
