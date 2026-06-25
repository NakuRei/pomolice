use serde_json::json;
use tauri::ipc::InvokeError;

use crate::commands::input_rejection::{
    observe_command_arg_result, observe_command_arg_result_with_logger,
    CommandInputRejectionContext,
};
use crate::logging_event::BackendLogEvent;

fn rejection_context() -> CommandInputRejectionContext {
    CommandInputRejectionContext {
        command_name: "start_work",
        argument_name: "work_duration_secs",
        window_label: "main".to_string(),
    }
}

fn assert_public_command_input_error(value: &serde_json::Value) {
    let response = value
        .as_object()
        .expect("command rejection should be a JSON object");

    assert_eq!(
        response
            .get("code")
            .expect("command rejection should include code")
            .as_str(),
        Some("invalid_command_input")
    );
    let message = response
        .get("message")
        .expect("command rejection should include message")
        .as_str()
        .expect("command rejection message should be text");
    assert!(!message.is_empty());
    assert_public_error_excludes_retry_affordance(response);
}

fn assert_public_error_excludes_retry_affordance(
    response: &serde_json::Map<String, serde_json::Value>,
) {
    for key in response.keys() {
        let normalized = key.to_ascii_lowercase();
        assert!(
            !normalized.contains("retry") && !normalized.contains("transient"),
            "backend public command error encoded retry eligibility in field: {key}"
        );
    }
}

fn assert_value_excludes(value: &serde_json::Value, diagnostic_fragments: &[&str]) {
    let serialized = value.to_string();

    for fragment in diagnostic_fragments {
        assert!(
            !serialized.contains(fragment),
            "command rejection leaked diagnostic fragment: {fragment}"
        );
    }
}

fn assert_log_field(fields: &[(String, String)], expected_name: &str, expected_value: &str) {
    let actual_value = fields
        .iter()
        .find_map(|(name, value)| (name == expected_name).then_some(value.as_str()));

    assert_eq!(
        actual_value,
        Some(expected_value),
        "expected log field {expected_name}={expected_value}, got {fields:?}"
    );
}

#[test]
fn test_observe_command_arg_result_success_wraps_inner_value() {
    let observed = observe_command_arg_result(Ok(42_u32), rejection_context())
        .expect("valid command argument should be accepted");

    assert_eq!(observed.into_inner(), 42);
}

#[test]
fn test_invalid_command_argument_logs_rejection_context_and_returns_public_error() {
    let invoke_error = InvokeError(json!("password=abc token=xyz"));
    let mut captured_logs = Vec::new();

    let result = observe_command_arg_result_with_logger::<u32, _>(
        Err(invoke_error),
        rejection_context(),
        |event, fields| {
            captured_logs.push((
                event,
                fields
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), value.clone()))
                    .collect::<Vec<_>>(),
            ));
        },
    );

    let Err(returned_error) = result else {
        panic!("invalid command argument should be rejected");
    };
    assert_public_command_input_error(&returned_error.0);
    assert_value_excludes(&returned_error.0, &["password=abc", "token=xyz"]);
    assert_eq!(captured_logs.len(), 1);

    let (event, fields) = &captured_logs[0];
    assert_eq!(*event, BackendLogEvent::TimerCommandInputRejected);
    assert_log_field(fields, "command_name", "start_work");
    assert_log_field(fields, "window_label", "main");
    assert_log_field(fields, "argument_name", "work_duration_secs");
    assert_log_field(fields, "error_message", "password=abc token=xyz");
}

#[test]
fn test_structured_command_argument_rejection_logs_diagnostic_context_and_returns_public_error() {
    let invoke_error = InvokeError(json!({
        "code": "invalid_args",
        "argument": "work_duration_secs",
        "message": "invalid type: string \"abc\", expected u64",
    }));
    let mut captured_logs = Vec::new();

    let result = observe_command_arg_result_with_logger::<u32, _>(
        Err(invoke_error),
        rejection_context(),
        |event, fields| {
            captured_logs.push((
                event,
                fields
                    .iter()
                    .map(|(name, value)| ((*name).to_string(), value.clone()))
                    .collect::<Vec<_>>(),
            ));
        },
    );

    let Err(returned_error) = result else {
        panic!("invalid command argument should be rejected");
    };
    assert_public_command_input_error(&returned_error.0);
    assert_value_excludes(
        &returned_error.0,
        &[
            "invalid_args",
            "work_duration_secs",
            "invalid type",
            "expected u64",
        ],
    );
    assert_eq!(captured_logs.len(), 1);

    let (event, fields) = &captured_logs[0];
    assert_eq!(*event, BackendLogEvent::TimerCommandInputRejected);
    assert_log_field(fields, "command_name", "start_work");
    assert_log_field(fields, "window_label", "main");
    assert_log_field(fields, "argument_name", "work_duration_secs");

    let Some(error_message) = fields
        .iter()
        .find_map(|(name, value)| (name == "error_message").then_some(value.as_str()))
    else {
        panic!("command rejection log should include diagnostic error_message");
    };
    assert!(error_message.contains("invalid_args"));
    assert!(error_message.contains("work_duration_secs"));
}

#[test]
fn test_command_argument_rejection_returns_public_error_without_framework_details() {
    let invoke_error = InvokeError(json!({
        "code": "invalid_args",
        "command": "start_work",
        "argument": "work_duration_secs",
        "path": "C:\\Users\\nak\\secret\\timer_state.json",
        "message": "missing required key work_duration_secs",
    }));

    let result = observe_command_arg_result::<u32>(Err(invoke_error), rejection_context());

    let Err(returned_error) = result else {
        panic!("invalid command argument should be rejected");
    };
    assert_public_command_input_error(&returned_error.0);

    let returned_error = returned_error.0.to_string();
    assert!(!returned_error.contains("invalid_args"));
    assert!(!returned_error.contains("start_work"));
    assert!(!returned_error.contains("work_duration_secs"));
    assert!(!returned_error.contains("C:\\Users\\nak"));
    assert!(!returned_error.contains("timer_state.json"));
    assert!(!returned_error.contains("missing required key"));
}

#[test]
fn test_command_argument_rejections_exclude_representative_diagnostics() {
    let rejection_values = [
        json!("invalid args at C:\\Users\\nak\\secret\\timer_state.json"),
        json!("thread 'main' panicked at src\\commands\\timer.rs:123: invalid command input"),
        json!({
            "code": "invalid_args",
            "message": "invalid type: string \"abc\", expected u64",
            "command": "start_work",
            "argument": "work_duration_secs",
        }),
        json!("Window control error: Failed to enter fullscreen"),
    ];

    for rejection in rejection_values {
        let result =
            observe_command_arg_result::<u32>(Err(InvokeError(rejection)), rejection_context());

        let Err(returned_error) = result else {
            panic!("invalid command argument should be rejected");
        };

        assert_public_command_input_error(&returned_error.0);
        assert_value_excludes(
            &returned_error.0,
            &[
                "C:\\Users\\nak",
                "timer_state.json",
                "thread 'main' panicked",
                "src\\commands\\timer.rs",
                "invalid type",
                "expected u64",
                "start_work",
                "work_duration_secs",
                "Failed to enter fullscreen",
            ],
        );
    }
}
