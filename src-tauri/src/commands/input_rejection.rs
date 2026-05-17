use tauri::ipc::{CommandArg, CommandItem, InvokeError};

use crate::logging_event::{log_warn_event, BackendLogEvent};

pub(crate) struct CommandInputRejectionContext {
    pub(crate) command_name: &'static str,
    pub(crate) argument_name: &'static str,
    pub(crate) window_label: String,
}

pub(crate) struct ObservedCommandArg<T> {
    value: T,
}

impl<T> ObservedCommandArg<T> {
    pub(crate) fn into_inner(self) -> T {
        self.value
    }
}

impl<'de, T, R> CommandArg<'de, R> for ObservedCommandArg<T>
where
    T: CommandArg<'de, R>,
    R: tauri::Runtime,
{
    fn from_command(command: CommandItem<'de, R>) -> Result<Self, InvokeError> {
        let context = CommandInputRejectionContext {
            command_name: command.name,
            argument_name: command.key,
            window_label: command.message.webview_ref().label().to_string(),
        };

        observe_command_arg_result(T::from_command(command), context)
    }
}

pub(crate) fn observe_command_arg_result<T>(
    result: Result<T, InvokeError>,
    context: CommandInputRejectionContext,
) -> Result<ObservedCommandArg<T>, InvokeError> {
    observe_command_arg_result_with_logger(result, context, log_warn_event)
}

pub(crate) fn observe_command_arg_result_with_logger<T, F>(
    result: Result<T, InvokeError>,
    context: CommandInputRejectionContext,
    mut log_rejection: F,
) -> Result<ObservedCommandArg<T>, InvokeError>
where
    F: FnMut(BackendLogEvent, &[(&str, String)]),
{
    match result {
        Ok(value) => Ok(ObservedCommandArg { value }),
        Err(error) => {
            let error_message = extract_invoke_error_message(&error);
            log_rejection(
                BackendLogEvent::TimerCommandInputRejected,
                &[
                    ("command_name", context.command_name.to_string()),
                    ("window_label", context.window_label),
                    ("argument_name", context.argument_name.to_string()),
                    ("error_message", error_message),
                ],
            );
            Err(error)
        }
    }
}

pub(crate) fn extract_invoke_error_message(error: &InvokeError) -> String {
    match &error.0 {
        serde_json::Value::String(message) => message.clone(),
        value => value.to_string(),
    }
}
