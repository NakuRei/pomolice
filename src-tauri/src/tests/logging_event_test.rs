use std::io;

use log::Level;

use crate::log_redaction::RedactionClass;
use crate::logging_event::{
    render_event_message, BackendLogEvent, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
};

#[test]
fn test_render_event_message_preserves_timer_command_unauthorized_log_contract() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandUnauthorized,
        &[
            ("authorization_token", "super-secret-token".to_string()),
            ("window_label", "main_window".to_string()),
            ("command_name", "sync_settings".to_string()),
        ],
        None,
    );

    assert_rendered_event_identity(&rendered, "timer.command.unauthorized");
    assert_rendered_field_names(
        &rendered,
        &["command_name", "window_label", "authorization_token"],
    );
    assert!(rendered.contains("sync_settings"));
    assert!(rendered.contains("main_window"));
    assert!(!rendered.contains("super-secret-token"));
}

#[test]
fn test_render_event_message_neutralizes_log_line_injection_without_losing_public_context() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandUnauthorized,
        &[
            ("authorization_token", "super-secret-token".to_string()),
            ("window_label", "main\r\nwindow".to_string()),
            ("command_name", "sync=settings".to_string()),
        ],
        None,
    );

    assert_rendered_event_identity(&rendered, "timer.command.unauthorized");
    assert_rendered_field_names(&rendered, &["command_name", "window_label"]);
    assert!(rendered.contains("sync"));
    assert!(rendered.contains("settings"));
    assert!(rendered.contains("main"));
    assert!(rendered.contains("window"));
    assert!(!rendered.contains('\r'));
    assert!(!rendered.contains('\n'));
}

#[test]
fn test_render_event_message_prevents_public_field_structure_injection() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandInvoked,
        &[
            (
                "command_name",
                "sync settings \"quoted\" fake=value \\\r\n\tend".to_string(),
            ),
            (
                "window_label",
                "main window \"label\" shadow=value \\\r\n\tend".to_string(),
            ),
        ],
        None,
    );

    assert_rendered_log_fields(&rendered, &["event_id", "command_name", "window_label"]);
}

#[test]
fn test_render_event_message_redacts_timer_command_input_rejected_error() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandInputRejected,
        &[
            ("error_message", "invalid args token=abc".to_string()),
            ("argument_name", "workDurationSecs".to_string()),
            ("window_label", "main".to_string()),
            ("command_name", "sync_timer_duration_settings".to_string()),
        ],
        None,
    );

    assert_rendered_event_identity(&rendered, "timer.command.input_rejected");
    assert_rendered_field_names(
        &rendered,
        &[
            "command_name",
            "window_label",
            "argument_name",
            "error_message",
        ],
    );
    assert!(rendered.contains("sync_timer_duration_settings"));
    assert!(rendered.contains("main"));
    assert!(rendered.contains("workDurationSecs"));
    assert!(rendered.contains("invalid"));
    assert!(rendered.contains("args"));
    assert!(!rendered.contains("token=abc"));
}

#[test]
fn test_render_event_message_prevents_sensitive_field_structure_injection() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandInputRejected,
        &[
            (
                "error_message",
                "invalid args \"quoted\" fake=value \\\r\n\tpassword=secret".to_string(),
            ),
            ("argument_name", "workDurationSecs".to_string()),
            ("window_label", "main".to_string()),
            ("command_name", "sync_timer_duration_settings".to_string()),
        ],
        None,
    );

    assert_rendered_log_fields(
        &rendered,
        &[
            "event_id",
            "command_name",
            "window_label",
            "argument_name",
            "error_message",
        ],
    );
    assert!(!rendered.contains("password=secret"));
}

#[test]
fn test_render_event_message_redacts_error_message_and_masks_high_risk_tokens() {
    let error = io::Error::other("password=abc token=xyz");
    let rendered = render_event_message(
        BackendLogEvent::SessionFullscreenBreakEnterFailed,
        &[],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "session.fullscreen_break.enter_failed");
    assert_rendered_field_names(&rendered, &["error_message"]);
    assert!(!rendered.contains("password=abc"));
    assert!(!rendered.contains("token=xyz"));
}

#[test]
fn test_render_event_message_redacts_authorization_bearer_tokens_from_error_object() {
    let cases = [
        ("request failed: Authorization: Bearer abc123", "abc123"),
        ("request failed: authorization: bearer def456", "def456"),
        ("request failed: AUTHORIZATION: BEARER ghi789", "ghi789"),
        ("request failed: Bearer jkl012", "jkl012"),
    ];

    for (message, token) in cases {
        let error = io::Error::other(message);
        let rendered = render_event_message(
            BackendLogEvent::SessionFullscreenBreakEnterFailed,
            &[],
            Some(&error),
        );

        assert_rendered_event_identity(&rendered, "session.fullscreen_break.enter_failed");
        assert_rendered_log_fields(&rendered, &["event_id", "error_message"]);
        assert!(rendered.contains("[REDACTED]"));
        assert!(rendered.contains("request"));
        assert!(rendered.contains("failed"));
        assert!(!rendered.contains(token));
    }
}

#[test]
fn test_render_event_message_redacts_authorization_bearer_tokens_from_error_field() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandInputRejected,
        &[
            (
                "error_message",
                "request failed: Authorization: Bearer abc123".to_string(),
            ),
            ("argument_name", "authorizationHeader".to_string()),
            ("window_label", "main".to_string()),
            ("command_name", "sync_timer_duration_settings".to_string()),
        ],
        None,
    );

    assert_rendered_log_fields(
        &rendered,
        &[
            "event_id",
            "command_name",
            "window_label",
            "argument_name",
            "error_message",
        ],
    );
    assert!(rendered.contains("[REDACTED]"));
    assert!(rendered.contains("request"));
    assert!(rendered.contains("failed"));
    assert!(!rendered.contains("abc123"));
}

#[test]
fn test_render_event_message_prevents_error_object_structure_injection() {
    let error = io::Error::other("failed \"quoted\" fake=value \\\r\n\twith backslash \\");
    let rendered = render_event_message(
        BackendLogEvent::SessionFullscreenBreakEnterFailed,
        &[("step", "enter fullscreen".to_string())],
        Some(&error),
    );

    assert_rendered_log_fields(&rendered, &["event_id", "step", "error_message"]);
}

#[test]
fn test_render_event_message_redacts_secret_fields_before_structure_encoding() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandUnauthorized,
        &[
            (
                "authorization_token",
                "top-secret \"quoted\" fake=value \\\r\n\tend".to_string(),
            ),
            ("window_label", "main_window".to_string()),
            ("command_name", "sync_settings".to_string()),
        ],
        None,
    );

    assert_rendered_log_fields(
        &rendered,
        &[
            "event_id",
            "command_name",
            "window_label",
            "authorization_token",
        ],
    );
    assert!(rendered.contains("authorization_token=[REDACTED]"));
    assert!(!rendered.contains("top-secret"));
    assert!(!rendered.contains("fake"));
}

#[test]
fn test_render_event_message_truncates_long_error_message() {
    let long_message = "a".repeat(10_000);
    let error = io::Error::other(long_message.clone());
    let rendered = render_event_message(
        BackendLogEvent::SessionFullscreenBreakEnterFailed,
        &[],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "session.fullscreen_break.enter_failed");
    assert_rendered_field_names(&rendered, &["error_message"]);
    assert!(rendered.len() < long_message.len());
    assert!(rendered.contains("...[truncated]"));
}

#[test]
fn test_render_event_message_includes_persistence_trigger_context() {
    let error = io::Error::other("disk full");
    let rendered = render_event_message(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[(
            "trigger",
            PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START.to_string(),
        )],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "storage.persistence.write_failed");
    assert_rendered_field_names(&rendered, &["trigger", "error_message"]);
    assert!(rendered.contains(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START));
    assert!(rendered.contains("disk"));
    assert!(rendered.contains("full"));
}

#[test]
fn test_render_persistence_write_failed_redacts_home_paths_from_path_and_error_message() {
    let error = io::Error::other(
        "failed to open /Users/alice/Library/Application Support/pomolice/timer_state.json and /home/alice/.config/pomolice/timer_state.json",
    );
    let rendered = render_event_message(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[
            (
                "trigger",
                PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START.to_string(),
            ),
            (
                "path",
                r"C:\Users\Alice\AppData\Local\pomolice\timer_state.json".to_string(),
            ),
        ],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "storage.persistence.write_failed");
    assert!(rendered.contains(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START));
    assert!(!rendered.contains("Alice"));
    assert!(!rendered.contains("alice"));
    assert!(!rendered.contains(r"C:\Users\Alice"));
    assert!(!rendered.contains("/Users/alice"));
    assert!(!rendered.contains("/home/alice"));
}

#[test]
fn test_render_persistence_write_failed_redacts_home_paths_with_spaces() {
    let error = io::Error::other(
        "failed to open /Users/Jane Doe/Library/Application Support/pomolice/timer_state.json and /home/Jane Doe/.config/pomolice/timer_state.json",
    );
    let rendered = render_event_message(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[
            (
                "trigger",
                PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START.to_string(),
            ),
            (
                "path",
                r"C:\Users\Jane Doe\AppData\Local\pomolice\timer_state.json".to_string(),
            ),
        ],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "storage.persistence.write_failed");
    assert!(rendered.contains(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START));
    assert!(!rendered.contains("Jane"));
    assert!(!rendered.contains("Doe"));
    assert!(!rendered.contains("Jane Doe"));
    assert!(!rendered.contains(r"C:\Users\Jane Doe"));
    assert!(!rendered.contains("/Users/Jane Doe"));
    assert!(!rendered.contains("/home/Jane Doe"));
}

#[test]
fn test_render_persistence_write_failed_redacts_profile_path_suffix_username() {
    let error = io::Error::other(
        "failed to open /Users/alice/Documents/alice-notes.txt and /home/alice/docs/alice-notes.txt",
    );
    let rendered = render_event_message(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[
            (
                "trigger",
                PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START.to_string(),
            ),
            (
                "path",
                r"C:\Users\Alice\Documents\Alice-notes.txt".to_string(),
            ),
        ],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "storage.persistence.write_failed");
    assert!(rendered.contains(PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START));
    assert!(!rendered.contains("Alice"));
    assert!(!rendered.contains("alice"));
    assert!(!rendered.contains(r"C:\Users\Alice"));
    assert!(!rendered.contains("/Users/alice"));
    assert!(!rendered.contains("/home/alice"));
}

#[test]
fn test_render_event_message_includes_break_step_context() {
    let error = io::Error::other("window exit failed");
    let rendered = render_event_message(
        BackendLogEvent::SessionFullscreenBreakExitFailed,
        &[(
            "step",
            "external_break_exit_fullscreen_all_monitors".to_string(),
        )],
        Some(&error),
    );

    assert_rendered_event_identity(&rendered, "session.fullscreen_break.exit_failed");
    assert_rendered_field_names(&rendered, &["step", "error_message"]);
    assert!(rendered.contains("external_break_exit_fullscreen_all_monitors"));
    assert!(rendered.contains("window"));
    assert!(rendered.contains("exit"));
    assert!(rendered.contains("failed"));
}

#[test]
fn test_backend_event_catalog_metadata_matches_contract() {
    let event = BackendLogEvent::TimerCommandUnauthorized;

    assert_eq!(event.event_id(), "timer.command.unauthorized");
    assert_eq!(event.severity(), Level::Warn);
    assert_field_contracts(
        event,
        &[
            ("command_name", RedactionClass::Public),
            ("window_label", RedactionClass::Public),
            ("authorization_token", RedactionClass::Secret),
        ],
    );
}

#[test]
fn test_timer_command_input_rejected_event_catalog_metadata_matches_contract() {
    let event = BackendLogEvent::TimerCommandInputRejected;

    assert_eq!(event.event_id(), "timer.command.input_rejected");
    assert_eq!(event.severity(), Level::Warn);
    assert_field_contracts(
        event,
        &[
            ("command_name", RedactionClass::Public),
            ("window_label", RedactionClass::Public),
            ("argument_name", RedactionClass::Public),
            ("error_message", RedactionClass::Sensitive),
        ],
    );
}

fn assert_rendered_event_identity(rendered: &str, event_id: &str) {
    assert!(
        rendered.contains(event_id),
        "rendered log should identify event {event_id}: {rendered}"
    );
}

fn assert_rendered_field_names(rendered: &str, field_names: &[&str]) {
    for field_name in field_names {
        assert!(
            rendered.contains(field_name),
            "rendered log should include field {field_name}: {rendered}"
        );
    }
}

fn assert_rendered_log_fields(rendered: &str, expected_fields: &[&str]) {
    assert!(
        !rendered.contains('\r') && !rendered.contains('\n') && !rendered.contains('\t'),
        "rendered log should not contain raw line or tab separators: {rendered}"
    );
    assert!(
        find_unescaped_character(rendered, '"').is_none(),
        "rendered log should not contain unescaped quotes: {rendered}"
    );

    let fields = split_log_fields(rendered);
    let actual_fields = fields
        .iter()
        .map(|field| rendered_log_field_name(field, rendered))
        .collect::<Vec<_>>();

    assert_eq!(actual_fields, expected_fields);
}

fn split_log_fields(rendered: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    let mut field_start = 0;
    let mut escaped = false;

    for (index, character) in rendered.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == ' ' {
            fields.push(&rendered[field_start..index]);
            field_start = index + character.len_utf8();
        }
    }

    fields.push(&rendered[field_start..]);
    fields
}

fn rendered_log_field_name(field: &str, rendered: &str) -> String {
    let separator = find_unescaped_character(field, '=').unwrap_or_else(|| {
        panic!("rendered log field should contain '=': {field:?} in {rendered}")
    });
    let value = &field[separator + 1..];

    assert!(
        find_unescaped_character(value, '=').is_none(),
        "rendered log field value should not contain unescaped '=': {field:?} in {rendered}"
    );

    let field_name = &field[..separator];
    assert!(
        !field_name.is_empty(),
        "rendered log field name should not be empty: {field:?} in {rendered}"
    );
    field_name.to_string()
}

fn find_unescaped_character(value: &str, target: char) -> Option<usize> {
    let mut escaped = false;

    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }

        if character == '\\' {
            escaped = true;
            continue;
        }

        if character == target {
            return Some(index);
        }
    }

    None
}

fn assert_field_contracts(event: BackendLogEvent, expected_fields: &[(&str, RedactionClass)]) {
    let actual_fields = event.allowed_fields();

    assert_eq!(actual_fields.len(), expected_fields.len());
    for (field_name, redaction_classification) in expected_fields {
        assert!(
            actual_fields.iter().any(|field| {
                field.name == *field_name
                    && field.redaction_classification == *redaction_classification
            }),
            "{field_name} should be classified as {redaction_classification:?}"
        );
    }
}
