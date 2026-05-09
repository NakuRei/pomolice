#[cfg(test)]
mod startup_restore_tests {
    use crate::session_pipeline::{
        build_session_completion_effect_plan, build_startup_resume_effect_plan, BreakModeChange,
        PersistenceRequirement, PipelineEmitEvent, TimerTaskRestart,
    };
    use crate::types::{SessionTransitionOutcome, SessionType, TimerStatus};

    /// Mirrors the `restart_timer_task` derivation in `lib_handle_startup_restore`
    /// so these tests break if the call-site logic diverges from the plan.
    fn startup_restart_timer_task(outcome: &SessionTransitionOutcome) -> bool {
        outcome.next_status != TimerStatus::Idle
    }

    #[test]
    fn test_startup_resume_action_keeps_post_manage_work_to_emit_and_restart_only() {
        let effect_plan = build_startup_resume_effect_plan();

        // The timer state is already restored before `app.manage(app_state)`.
        // Startup handling should only emit the first tick and restart the task.
        assert_eq!(effect_plan.emit_order, &[PipelineEmitEvent::TimerTick]);
        assert_eq!(effect_plan.break_mode_change, BreakModeChange::None);
        assert_eq!(
            effect_plan.persistence_requirement,
            PersistenceRequirement::Skip
        );
        assert_eq!(effect_plan.timer_task_restart, TimerTaskRestart::Restart);
        assert!(!effect_plan
            .emit_order
            .contains(&PipelineEmitEvent::AutoRunStopped));
    }

    #[test]
    fn test_startup_expired_work_to_break_effect_plan_enters_break_and_restarts_timer() {
        let transition_outcome = SessionTransitionOutcome::work_to_break(SessionType::ShortBreak);
        let restart = startup_restart_timer_task(&transition_outcome);
        let effect_plan = build_session_completion_effect_plan(&transition_outcome, restart);

        assert_eq!(
            effect_plan.emit_order,
            &[
                PipelineEmitEvent::SessionCompleted,
                PipelineEmitEvent::TimerTick,
            ]
        );
        assert_eq!(effect_plan.break_mode_change, BreakModeChange::Enter);
        assert_eq!(
            effect_plan.persistence_requirement,
            PersistenceRequirement::Persist
        );
        assert_eq!(effect_plan.timer_task_restart, TimerTaskRestart::Restart);
    }

    #[test]
    fn test_startup_expired_work_scheduled_stop_effect_plan_skips_break_and_emits_auto_run_stopped()
    {
        let transition_outcome =
            SessionTransitionOutcome::scheduled_stop_executed(SessionType::Work);
        let restart = startup_restart_timer_task(&transition_outcome);
        let effect_plan = build_session_completion_effect_plan(&transition_outcome, restart);

        assert_eq!(
            effect_plan.emit_order,
            &[
                PipelineEmitEvent::SessionCompleted,
                PipelineEmitEvent::AutoRunStopped,
                PipelineEmitEvent::TimerTick,
            ]
        );
        assert_eq!(effect_plan.break_mode_change, BreakModeChange::None);
        assert_eq!(
            effect_plan.persistence_requirement,
            PersistenceRequirement::Persist
        );
        assert_eq!(effect_plan.timer_task_restart, TimerTaskRestart::NoRestart);
    }
}
