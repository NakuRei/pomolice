use log::Level;

use crate::log_redaction::{redact_exception_message, RedactionClass};

pub(crate) const PERSISTENCE_WRITE_TRIGGER_RUN_EVENT_EXIT: &str = "run_event_exit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendLogEvent {
    AppStartupCompleted,
    AppStateCreated,
    AppStateRestoredFromPersistence,
    AppStateUsingDefaults,
    AppStateInitializationStarted,
    AppStateInitializationCompleted,
    AppWindowHiddenToTray,
    AppTrayInitialized,
    AppTraySetupFailed,
    AppTrayMenuExitSelected,
    AppTrayIconUpdateFailed,
    AppTrayTooltipUpdateFailed,
    AppTrayIdleAppIconMissing,
    TimerCommandInvoked,
    TimerCommandUnauthorized,
    TimerCommandInputRejected,
    ToolingTrayIconFixtureExported,
    ToolingTrayIconFixtureExportCompleted,
    TimerStartRejected,
    TimerStartRateLimited,
    TimerTransitionRejected,
    TimerWorkSessionStarted,
    TimerTransitionCompleted,
    SettingsSyncApplied,
    SettingsSyncFailed,
    StoragePersistenceWriteStarted,
    StoragePersistenceWriteFailed,
    StoragePersistenceWriteCompleted,
    StoragePersistencePathUnavailable,
    StoragePersistenceReadStarted,
    StoragePersistenceReadFailed,
    StoragePersistenceNoStateFile,
    StoragePersistenceLoaded,
    StoragePersistenceParseFailed,
    StoragePersistenceSchemaVersionUnsupported,
    StoragePersistenceRestoreNormalized,
    SchedulerCrossedEndBoundary,
    SchedulerMarkedStopPending,
    SchedulerCrossedStartBoundary,
    SchedulerStartedWorkFromBoundary,
    SchedulerSkippedAutoStartDueToRejection,
    InputMonitoringStarted,
    InputMonitoringAlreadyRunning,
    InputMonitoringListenerStarted,
    InputMonitoringListenerStoppedCleanly,
    InputMonitoringShutdownRequestedAfterError,
    InputMonitoringListenerFailed,
    InputMonitoringAlreadyStopped,
    InputMonitoringStopping,
    SessionFullscreenBreakEnterStarted,
    SessionFullscreenBreakEntered,
    SessionFullscreenBreakEnterFailed,
    SessionFullscreenBreakExitStarted,
    SessionFullscreenBreakExited,
    SessionFullscreenBreakExitFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackendLogFieldSpec {
    pub name: &'static str,
    pub redaction_classification: RedactionClass,
}

impl BackendLogFieldSpec {
    const fn new(name: &'static str, redaction_classification: RedactionClass) -> Self {
        Self {
            name,
            redaction_classification,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BackendLogEventSpec {
    pub event_id: &'static str,
    pub severity: Level,
    pub message_template: &'static str,
    pub allowed_fields: &'static [BackendLogFieldSpec],
}

macro_rules! fields {
    ($(($name:literal, $classification:ident)),* $(,)?) => {
        {
            const ALLOWED_FIELDS: &[BackendLogFieldSpec] = &[
                $(BackendLogFieldSpec::new($name, RedactionClass::$classification)),*
            ];
            ALLOWED_FIELDS
        }
    };
}

macro_rules! spec {
    ($event_id:literal, $severity:ident, $message_template:literal, [$($field:literal => $classification:ident),* $(,)?]) => {
        BackendLogEventSpec {
            event_id: $event_id,
            severity: Level::$severity,
            message_template: $message_template,
            allowed_fields: fields![$(($field, $classification)),*],
        }
    };
}

include!("logging_event_specs.incl.rs");

impl BackendLogEvent {
    pub(crate) fn spec(self) -> BackendLogEventSpec {
        backend_log_event_spec_match!(self)
    }

    #[cfg(test)]
    pub(crate) fn event_id(self) -> &'static str {
        self.spec().event_id
    }

    pub(crate) fn severity(self) -> Level {
        self.spec().severity
    }

    #[cfg(test)]
    pub(crate) fn allowed_fields(self) -> &'static [BackendLogFieldSpec] {
        self.spec().allowed_fields
    }
}

pub fn log_info_event(event: BackendLogEvent, fields: &[(&str, String)]) {
    debug_assert_eq!(event.severity(), Level::Info);
    log::info!("{}", render_event_message(event, fields, None));
}

pub fn log_warn_event(event: BackendLogEvent, fields: &[(&str, String)]) {
    debug_assert_eq!(event.severity(), Level::Warn);
    log::warn!("{}", render_event_message(event, fields, None));
}

pub fn log_error_event(
    event: BackendLogEvent,
    fields: &[(&str, String)],
    error: Option<&dyn std::error::Error>,
) {
    debug_assert_eq!(event.severity(), Level::Error);
    log::error!("{}", render_event_message(event, fields, error));
}

pub(crate) fn log_persistence_write_failure(trigger: &'static str, error: &dyn std::error::Error) {
    log_error_event(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[("trigger", trigger.to_string())],
        Some(error),
    );
}

#[cfg(test)]
pub(crate) fn render_persistence_write_failure_message(
    trigger: &'static str,
    error: &dyn std::error::Error,
) -> String {
    render_event_message(
        BackendLogEvent::StoragePersistenceWriteFailed,
        &[("trigger", trigger.to_string())],
        Some(error),
    )
}

pub(crate) fn render_event_message(
    event: BackendLogEvent,
    fields: &[(&str, String)],
    error: Option<&dyn std::error::Error>,
) -> String {
    let spec = event.spec();
    let mut rendered_fields = Vec::new();

    for field_spec in spec.allowed_fields {
        if field_spec.name == "error_message" {
            if let Some(error) = error {
                rendered_fields.push(format!(
                    "{}={}",
                    field_spec.name,
                    redact_exception_message(&error.to_string())
                ));
                continue;
            }

            if let Some((_, value)) = fields.iter().find(|(name, _)| *name == field_spec.name) {
                rendered_fields.push(format!(
                    "{}={}",
                    field_spec.name,
                    crate::log_redaction::redact_field_value(RedactionClass::Sensitive, value,)
                ));
            }
            continue;
        }

        if let Some((_, value)) = fields.iter().find(|(name, _)| *name == field_spec.name) {
            let rendered_value = match field_spec.redaction_classification {
                RedactionClass::Public => {
                    crate::log_redaction::redact_field_value(RedactionClass::Public, value)
                }
                RedactionClass::Sensitive => {
                    crate::log_redaction::redact_field_value(RedactionClass::Sensitive, value)
                }
                RedactionClass::Secret => {
                    crate::log_redaction::redact_field_value(RedactionClass::Secret, value)
                }
            };
            rendered_fields.push(format!("{}={rendered_value}", field_spec.name));
        }
    }

    if let Some(error) = error {
        if !spec
            .allowed_fields
            .iter()
            .any(|field| field.name == "error_message")
        {
            rendered_fields.push(format!(
                "error_message={}",
                redact_exception_message(&error.to_string())
            ));
        }
    }

    if rendered_fields.is_empty() {
        format!("event_id={}", spec.event_id)
    } else {
        format!("event_id={} {}", spec.event_id, rendered_fields.join(" "))
    }
}
