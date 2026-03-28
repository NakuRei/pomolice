use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Days;

use crate::logging::{
    build_file_appender_for_test, init_logging_for_test, should_enable_stdout_layer_for,
    LoggingRuntime, LOG_FILENAME_PREFIX, LOG_FILENAME_SUFFIX, MAX_LOG_FILES,
};

static LOGGING_INIT_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
static LOGGING_RUNTIME: OnceLock<LoggingRuntime> = OnceLock::new();
static LOGGING_LOG_DIRECTORY: OnceLock<PathBuf> = OnceLock::new();

fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos()
        .to_string()
}

fn create_temp_directory(test_name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("pomolice-{test_name}-{}", unique_suffix()));
    fs::create_dir_all(&path).expect("temporary test directory should be created");
    path
}

fn managed_log_files(log_directory: &Path) -> Vec<PathBuf> {
    let mut entries = fs::read_dir(log_directory)
        .expect("log directory should be readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|file_name| {
                    file_name.starts_with(LOG_FILENAME_PREFIX)
                        && file_name.ends_with(LOG_FILENAME_SUFFIX)
                })
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries
}

fn log_directory_contents(log_directory: &Path) -> String {
    managed_log_files(log_directory)
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("log file should be readable"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_date_fragment(file_name: &str) -> Option<&str> {
    let without_prefix = file_name.strip_prefix(LOG_FILENAME_PREFIX)?;
    let without_prefix_separator = without_prefix.strip_prefix('.')?;
    let without_suffix = without_prefix_separator.strip_suffix(LOG_FILENAME_SUFFIX)?;
    without_suffix.strip_suffix('.')
}

fn wait_for_log_contents(log_directory: &Path, expected_fragments: &[&str]) -> String {
    for _ in 0..20 {
        let contents = log_directory_contents(log_directory);
        if expected_fragments
            .iter()
            .all(|expected_fragment| contents.contains(expected_fragment))
        {
            return contents;
        }

        thread::sleep(Duration::from_millis(50));
    }

    log_directory_contents(log_directory)
}

fn ensure_global_logging_for_test() -> &'static PathBuf {
    let init_mutex = LOGGING_INIT_MUTEX.get_or_init(|| Mutex::new(()));
    let _init_guard = init_mutex.lock().expect("logging init mutex should lock");

    LOGGING_LOG_DIRECTORY.get_or_init(|| {
        let log_directory = create_temp_directory("global-logging");
        let runtime = init_logging_for_test(&log_directory, false)
            .expect("global test logging should initialize successfully");
        assert!(
            LOGGING_RUNTIME.set(runtime).is_ok(),
            "logging runtime should only initialize once"
        );
        log_directory
    })
}

#[test]
fn test_init_logging_routes_tracing_and_log_records_to_file() {
    let log_directory = ensure_global_logging_for_test();
    let unique_id = unique_suffix();
    let frontend_like_message = format!("timer_sync.setup_failed.{unique_id}");
    let backend_error_message = format!("backend_error.{unique_id}");
    let dependency_log_message = format!("dependency_log.{unique_id}");

    tracing::info!(
        frontend_scope = "timer_sync",
        frontend_event = "setup_failed",
        "{frontend_like_message}"
    );
    tracing::error!(failure_code = 42, "{backend_error_message}");
    log::info!("{dependency_log_message}");

    let contents = wait_for_log_contents(
        log_directory,
        &[
            frontend_like_message.as_str(),
            backend_error_message.as_str(),
            dependency_log_message.as_str(),
        ],
    );

    assert!(contents.contains("frontend_scope"));
    assert!(contents.contains("failure_code=42"));
}

#[test]
fn test_release_like_config_disables_stdout_layer_and_keeps_file_sink() {
    let log_directory = create_temp_directory("release-like");
    assert!(!should_enable_stdout_layer_for(false));

    let mut file_appender = build_file_appender_for_test(&log_directory)
        .expect("release-like file appender should be built");
    writeln!(file_appender, "release-like-file-sink").expect("file appender should write");

    let contents = log_directory_contents(&log_directory);
    assert!(contents.contains("release-like-file-sink"));
}

#[test]
fn test_max_log_files_prunes_only_managed_log_files() {
    let log_directory = create_temp_directory("retention");
    let mut file_appender =
        build_file_appender_for_test(&log_directory).expect("file appender should build");
    writeln!(file_appender, "seed current log").expect("current log should be writable");

    let current_file = managed_log_files(&log_directory)
        .into_iter()
        .next()
        .expect("current managed log file should exist");
    let current_file_name = current_file
        .file_name()
        .and_then(|value| value.to_str())
        .expect("current managed log file name should be valid unicode")
        .to_string();
    let current_date = extract_date_fragment(&current_file_name)
        .expect("current file name should include a daily rotation date");
    let current_date = chrono::NaiveDate::parse_from_str(current_date, "%Y-%m-%d")
        .expect("current managed log file should use yyyy-mm-dd rotation dates");

    for day_offset in 1..=10 {
        let previous_date = (current_date - Days::new(day_offset))
            .format("%Y-%m-%d")
            .to_string();
        let previous_file_name = current_file_name.replacen(
            &current_date.format("%Y-%m-%d").to_string(),
            &previous_date,
            1,
        );
        fs::write(log_directory.join(previous_file_name), b"old log")
            .expect("managed historical log should be created");
    }

    let unrelated_file = log_directory.join("keep-me.txt");
    fs::write(&unrelated_file, b"unrelated").expect("unrelated file should be created");

    let mut pruned_file_appender =
        build_file_appender_for_test(&log_directory).expect("file appender should rebuild");
    writeln!(pruned_file_appender, "retention rewrite").expect("rebuilt appender should write");

    assert_eq!(managed_log_files(&log_directory).len(), MAX_LOG_FILES);
    assert!(unrelated_file.exists());
}
