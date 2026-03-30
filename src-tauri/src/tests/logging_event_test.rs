use std::io;

use crate::logging_event::{render_event_message, BackendLogEvent};

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
fn test_backend_event_catalog_metadata_matches_contract() {
    let event = BackendLogEvent::TimerCommandUnauthorized;

    assert_eq!(event.event_id(), "timer.command.unauthorized");
    assert_eq!(event.message_template(), "Timer command unauthorized");
    assert!(event
        .allowed_fields()
        .iter()
        .any(|field| field.name == "authorization_token"));
}
