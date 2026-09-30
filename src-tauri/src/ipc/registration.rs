macro_rules! with_commands {
    ($register:ident) => {
        $register![
            application::is_tray_available,
            application::has_input_monitor_failed,
            application::get_emergency_shortcut_status,
            application::exit_without_tray,
            timer::start_work,
            timer::resolve_post_break_decision,
            timer::set_away_during_break,
            timer::emergency_stop,
            timer::update_settings,
            timer::get_full_state,
            timer::resolve_boot_auto_start_prompt,
            timer::retry_failed_timer_effects,
        ]
    };
}

pub(crate) use with_commands;
