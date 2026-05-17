use serde_json::json;
use tauri::ipc::InvokeError;

use crate::commands::input_rejection::{
    extract_invoke_error_message, observe_command_arg_result,
    observe_command_arg_result_with_logger, CommandInputRejectionContext,
};
use crate::logging_event::BackendLogEvent;

fn rejection_context() -> CommandInputRejectionContext {
    CommandInputRejectionContext {
        command_name: "start_work",
        argument_name: "work_duration_secs",
        window_label: "main".to_string(),
    }
}

#[test]
fn test_observe_command_arg_result_success_wraps_inner_value() {
    let observed = observe_command_arg_result(Ok(42_u32), rejection_context())
        .expect("valid command argument should be accepted");

    assert_eq!(observed.into_inner(), 42);
}

#[test]
fn test_observe_command_arg_result_rejection_logs_event_once() {
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
    assert_eq!(returned_error.0, json!("password=abc token=xyz"));
    assert_eq!(captured_logs.len(), 1);

    let (event, fields) = &captured_logs[0];
    assert_eq!(*event, BackendLogEvent::TimerCommandInputRejected);
    assert_eq!(
        fields,
        &vec![
            ("command_name".to_string(), "start_work".to_string()),
            ("window_label".to_string(), "main".to_string()),
            (
                "argument_name".to_string(),
                "work_duration_secs".to_string()
            ),
            (
                "error_message".to_string(),
                "password=abc token=xyz".to_string()
            ),
        ]
    );
}

#[test]
fn test_extract_invoke_error_message_preserves_string_values() {
    let invoke_error = InvokeError(json!("invalid args"));

    assert_eq!(extract_invoke_error_message(&invoke_error), "invalid args");
}

#[test]
fn test_extract_invoke_error_message_serializes_non_string_values() {
    let invoke_error = InvokeError(json!({
        "code": "invalid_args",
        "argument": "work_duration_secs",
    }));

    assert_eq!(
        extract_invoke_error_message(&invoke_error),
        "{\"argument\":\"work_duration_secs\",\"code\":\"invalid_args\"}"
    );
}
