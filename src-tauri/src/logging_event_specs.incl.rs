macro_rules! backend_log_event_spec_match {
    ($event:expr) => {
        match $event {
            BackendLogEvent::AppStartupCompleted => spec!(
                "app.startup.completed",
                Info,
                "Application startup completed",
                [
                    "startup_phase" => Public,
                    "has_persisted_state" => Public,
                    "restored_state_status" => Public,
                ]
            ),
            BackendLogEvent::AppStateCreated => {
                spec!("app.state.created", Info, "Application state created", [])
            }
            BackendLogEvent::AppStateRestoredFromPersistence => spec!(
                "app.state.restored_from_persistence",
                Info,
                "Persisted application state restored",
                [
                    "state_status" => Public,
                    "saved_at" => Public,
                ]
            ),
            BackendLogEvent::AppStateUsingDefaults => spec!(
                "app.state.using_defaults",
                Info,
                "Application state initialized from defaults",
                []
            ),
            BackendLogEvent::AppStateInitializationStarted => spec!(
                "app.state.initialization.started",
                Info,
                "Application state initialization started",
                []
            ),
            BackendLogEvent::AppStateInitializationCompleted => spec!(
                "app.state.initialization.completed",
                Info,
                "Application state initialization completed",
                [
                    "is_running" => Public,
                ]
            ),
            BackendLogEvent::AppWindowHiddenToTray => spec!(
                "app.window.hidden_to_tray",
                Info,
                "Main window hidden to tray",
                [
                    "first_notification" => Public,
                ]
            ),
            BackendLogEvent::AppTrayInitialized => {
                spec!("tray.initialized", Info, "System tray icon initialized", [])
            }
            BackendLogEvent::AppTraySetupFailed => spec!(
                "tray.setup_failed",
                Error,
                "System tray setup failed",
                [
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::AppTrayMenuExitSelected => spec!(
                "tray.menu.exit_selected",
                Info,
                "Tray exit menu item selected",
                []
            ),
            BackendLogEvent::AppTrayPhantomMenuEventIgnored => spec!(
                "tray.menu.phantom_event_ignored",
                Warn,
                "Ignored phantom tray menu event",
                []
            ),
            BackendLogEvent::AppTrayIconUpdateFailed => spec!(
                "tray.icon.update_failed",
                Warn,
                "Failed to update tray icon",
                [
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::AppTrayTooltipUpdateFailed => spec!(
                "tray.tooltip.update_failed",
                Warn,
                "Failed to update tray tooltip",
                [
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::AppTrayIdleAppIconMissing => spec!(
                "tray.idle_icon_missing",
                Warn,
                "Idle app icon was not initialized",
                []
            ),
            BackendLogEvent::TimerCommandInvoked => spec!(
                "timer.command.invoked",
                Info,
                "Timer command invoked",
                [
                    "command_name" => Public,
                    "window_label" => Public,
                ]
            ),
            BackendLogEvent::TimerCommandUnauthorized => spec!(
                "timer.command.unauthorized",
                Warn,
                "Timer command unauthorized",
                [
                    "command_name" => Public,
                    "window_label" => Public,
                    "authorization_token" => Secret,
                ]
            ),
            BackendLogEvent::TimerCommandInputRejected => spec!(
                "timer.command.input_rejected",
                Warn,
                "Timer command input rejected",
                [
                    "command_name" => Public,
                    "window_label" => Public,
                    "argument_name" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::ToolingTrayIconFixtureExported => spec!(
                "tooling.tray_icon.fixture_exported",
                Info,
                "Tray icon fixture exported",
                [
                    "path" => Public,
                ]
            ),
            BackendLogEvent::ToolingTrayIconFixtureExportCompleted => spec!(
                "tooling.tray_icon.fixture_export_completed",
                Info,
                "Tray icon fixture export completed",
                [
                    "output_directory" => Public,
                ]
            ),
            BackendLogEvent::TimerStartRejected => spec!(
                "timer.start.rejected",
                Warn,
                "Timer start rejected",
                [
                    "reason" => Public,
                    "current_state" => Public,
                ]
            ),
            BackendLogEvent::TimerStartRateLimited => spec!(
                "timer.start.rate_limited",
                Warn,
                "Timer start was rate limited",
                [
                    "remaining_ms" => Public,
                ]
            ),
            BackendLogEvent::TimerTransitionRejected => spec!(
                "timer.transition.rejected",
                Warn,
                "Timer transition was rejected",
                [
                    "action" => Public,
                    "current_state" => Public,
                    "pending" => Public,
                ]
            ),
            BackendLogEvent::TimerWorkSessionStarted => spec!(
                "timer.work_session.started",
                Info,
                "Work session started",
                [
                    "duration_secs" => Public,
                    "work_start_origin" => Public,
                ]
            ),
            BackendLogEvent::TimerTransitionCompleted => spec!(
                "timer.transition.completed",
                Info,
                "Timer transition completed",
                [
                    "transition_stage" => Public,
                    "transition_reason" => Public,
                    "policy" => Public,
                    "post_break_decision_pending" => Public,
                    "schedule_stop_state" => Public,
                    "action" => Public,
                    "remaining_seconds" => Public,
                    "completed_session_type" => Public,
                    "next_status" => Public,
                    "next_session_type" => Public,
                ]
            ),
            BackendLogEvent::SettingsSyncApplied => spec!(
                "settings.sync.applied",
                Info,
                "Settings sync applied",
                [
                    "timer_status" => Public,
                ]
            ),
            BackendLogEvent::SettingsSyncFailed => spec!(
                "settings.sync.failed",
                Warn,
                "Settings sync failed",
                [
                    "timer_status" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::StoragePersistenceWriteStarted => spec!(
                "storage.persistence.write_started",
                Info,
                "Persistence write started",
                [
                    "path" => Public,
                ]
            ),
            BackendLogEvent::StoragePersistenceWriteFailed => spec!(
                "storage.persistence.write_failed",
                Error,
                "Persistence write failed",
                [
                    "trigger" => Public,
                    "path" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::StoragePersistenceWriteCompleted => spec!(
                "storage.persistence.write_completed",
                Info,
                "Persistence write completed",
                [
                    "path" => Public,
                ]
            ),
            BackendLogEvent::StoragePersistencePathUnavailable => spec!(
                "storage.persistence.path_unavailable",
                Warn,
                "Persistence path was unavailable",
                []
            ),
            BackendLogEvent::StoragePersistenceReadStarted => spec!(
                "storage.persistence.read_started",
                Info,
                "Persistence read started",
                [
                    "path" => Public,
                ]
            ),
            BackendLogEvent::StoragePersistenceReadFailed => spec!(
                "storage.persistence.read_failed",
                Error,
                "Persistence read failed",
                [
                    "path" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::StoragePersistenceNoStateFile => spec!(
                "storage.persistence.no_state_file",
                Info,
                "Persistence state file missing",
                [
                    "path" => Public,
                ]
            ),
            BackendLogEvent::StoragePersistenceLoaded => spec!(
                "storage.persistence.loaded",
                Info,
                "Persistence state loaded",
                []
            ),
            BackendLogEvent::StoragePersistenceParseFailed => spec!(
                "storage.persistence.parse_failed",
                Warn,
                "Persistence parse failed",
                [
                    "path" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::StoragePersistenceSchemaVersionUnsupported => spec!(
                "storage.persistence.schema_version_unsupported",
                Warn,
                "Unsupported persisted schema version",
                [
                    "schema_version" => Public,
                    "expected_schema_version" => Public,
                ]
            ),
            BackendLogEvent::StoragePersistenceRestoreNormalized => spec!(
                "storage.persistence.restore_normalized",
                Warn,
                "Persisted state normalized on restore",
                [
                    "status" => Public,
                    "session_type" => Public,
                    "remaining_seconds" => Public,
                    "completed_pomodoros" => Public,
                ]
            ),
            BackendLogEvent::SchedulerStarted => spec!(
                "scheduler.started",
                Info,
                "Weekly scheduler started",
                [
                    "previous_local" => Public,
                    "blocked_until_idle" => Public,
                ]
            ),
            BackendLogEvent::SchedulerReceivedCancellation => spec!(
                "scheduler.received_cancellation",
                Info,
                "Weekly scheduler received cancellation",
                []
            ),
            BackendLogEvent::SchedulerStopped => {
                spec!("scheduler.stopped", Info, "Weekly scheduler stopped", [])
            }
            BackendLogEvent::SchedulerResumedStartBoundaryEvaluation => spec!(
                "scheduler.resumed_start_boundary_evaluation",
                Info,
                "Weekly scheduler resumed start-boundary evaluation",
                []
            ),
            BackendLogEvent::SchedulerConfigRevisionChanged => spec!(
                "scheduler.config_revision_changed",
                Info,
                "Weekly scheduler detected config revision change",
                [
                    "config_revision" => Public,
                ]
            ),
            BackendLogEvent::SchedulerCrossedEndBoundary => spec!(
                "scheduler.crossed_end_boundary",
                Info,
                "Weekly scheduler crossed end boundary",
                [
                    "boundary_id" => Public,
                ]
            ),
            BackendLogEvent::SchedulerMarkedStopPending => spec!(
                "scheduler.marked_stop_pending",
                Info,
                "Weekly scheduler marked scheduled stop as pending",
                [
                    "boundary_id" => Public,
                ]
            ),
            BackendLogEvent::SchedulerCrossedStartBoundary => spec!(
                "scheduler.crossed_start_boundary",
                Info,
                "Weekly scheduler crossed start boundary",
                [
                    "boundary_id" => Public,
                ]
            ),
            BackendLogEvent::SchedulerStartedWorkFromBoundary => spec!(
                "scheduler.started_work_from_boundary",
                Info,
                "Weekly scheduler started a work session from a start boundary",
                [
                    "boundary_id" => Public,
                ]
            ),
            BackendLogEvent::SchedulerSkippedAutoStartDueToRejection => spec!(
                "scheduler.skipped_auto_start_due_to_rejection",
                Info,
                "Weekly scheduler skipped auto-start after start_work rejection",
                [
                    "boundary_id" => Public,
                ]
            ),
            BackendLogEvent::InputMonitoringStarted => spec!(
                "input.monitoring.started",
                Info,
                "Global input monitoring started",
                []
            ),
            BackendLogEvent::InputMonitoringAlreadyRunning => spec!(
                "input.monitoring.already_running",
                Info,
                "Input monitoring already running",
                []
            ),
            BackendLogEvent::InputMonitoringListenerStarted => spec!(
                "input.monitoring.listener_started",
                Info,
                "Input event listener started",
                []
            ),
            BackendLogEvent::InputMonitoringListenerStoppedCleanly => spec!(
                "input.monitoring.listener_stopped_cleanly",
                Info,
                "Input event listener stopped cleanly",
                []
            ),
            BackendLogEvent::InputMonitoringShutdownRequestedAfterError => spec!(
                "input.monitoring.shutdown_requested_after_error",
                Info,
                "Input monitoring stopped after listener error",
                []
            ),
            BackendLogEvent::InputMonitoringListenerFailed => spec!(
                "input.monitoring.listener_failed",
                Error,
                "Input monitoring listener failed",
                [
                    "backoff" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::InputMonitoringAlreadyStopped => spec!(
                "input.monitoring.already_stopped",
                Info,
                "Input monitoring already stopped",
                []
            ),
            BackendLogEvent::InputMonitoringStopping => spec!(
                "input.monitoring.stopping",
                Info,
                "Stopping global input monitoring",
                []
            ),
            BackendLogEvent::SessionFullscreenBreakEnterStarted => spec!(
                "session.fullscreen_break.enter_started",
                Info,
                "Fullscreen break mode entry started",
                []
            ),
            BackendLogEvent::SessionFullscreenBreakEntered => spec!(
                "session.fullscreen_break.entered",
                Info,
                "Fullscreen break mode entered",
                []
            ),
            BackendLogEvent::SessionFullscreenBreakEnterFailed => spec!(
                "session.fullscreen_break.enter_failed",
                Error,
                "Fullscreen break mode entry failed",
                [
                    "step" => Public,
                    "error_message" => Sensitive,
                ]
            ),
            BackendLogEvent::SessionFullscreenBreakExitStarted => spec!(
                "session.fullscreen_break.exit_started",
                Info,
                "Fullscreen break mode exit started",
                []
            ),
            BackendLogEvent::SessionFullscreenBreakExited => spec!(
                "session.fullscreen_break.exited",
                Info,
                "Fullscreen break mode exited",
                []
            ),
            BackendLogEvent::SessionFullscreenBreakExitFailed => spec!(
                "session.fullscreen_break.exit_failed",
                Error,
                "Fullscreen break mode exit failed",
                [
                    "step" => Public,
                    "error_message" => Sensitive,
                ]
            ),
        }
    };
}
