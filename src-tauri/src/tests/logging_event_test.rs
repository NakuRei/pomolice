use std::io;

use log::Level;

use crate::logging_event::{
    render_event_message, BackendLogEvent, PERSISTENCE_WRITE_TRIGGER_SCHEDULER_AUTO_START,
};

#[test]
fn test_render_event_message_uses_canonical_prefix_and_deterministic_field_order() {
    let rendered = render_event_message(
        BackendLogEvent::TimerCommandUnauthorized,
        &[
            ("authorization_token", "super-secret-token".to_string()),
            ("window_label", "main\r\nwindow".to_string()),
            ("command_name", "sync=settings".to_string()),
        ],
        None,
    );

    assert_eq!(
        rendered,
        "event_id=timer.command.unauthorized command_name=sync\\=settings window_label=main\\r\\nwindow authorization_token=[REDACTED]"
    );
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

    assert_eq!(
        rendered,
        "event_id=timer.command.input_rejected command_name=sync_timer_duration_settings window_label=main argument_name=workDurationSecs error_message=invalid\\ args\\ token\\=[REDACTED]"
    );
}

#[test]
fn test_render_event_message_redacts_error_message_and_masks_high_risk_tokens() {
    let error = io::Error::other("password=abc token=xyz");
    let rendered = render_event_message(
        BackendLogEvent::SessionFullscreenBreakEnterFailed,
        &[],
        Some(&error),
    );

    assert_eq!(
        rendered,
        "event_id=session.fullscreen_break.enter_failed error_message=password\\=[REDACTED]\\ token\\=[REDACTED]"
    );
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

    assert_eq!(
        rendered,
        "event_id=storage.persistence.write_failed trigger=scheduler_auto_start_persist error_message=disk\\ full"
    );
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

    assert_eq!(
        rendered,
        "event_id=session.fullscreen_break.exit_failed step=external_break_exit_fullscreen_all_monitors error_message=window\\ exit\\ failed"
    );
}

#[test]
fn test_backend_event_catalog_metadata_matches_contract() {
    let event = BackendLogEvent::TimerCommandUnauthorized;

    assert_eq!(event.event_id(), "timer.command.unauthorized");
    assert_eq!(event.message_template(), "Timer command unauthorized");
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "authorization_token"));
}

#[test]
fn test_timer_command_input_rejected_event_catalog_metadata_matches_contract() {
    let event = BackendLogEvent::TimerCommandInputRejected;

    assert_eq!(event.event_id(), "timer.command.input_rejected");
    assert_eq!(event.message_template(), "Timer command input rejected");
    assert_eq!(event.severity(), Level::Warn);
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "command_name"));
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "window_label"));
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "argument_name"));
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "error_message"));
}
