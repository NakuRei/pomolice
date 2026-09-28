// Tauri commands module
// All commands use dependency injection via State

pub(crate) mod application;
pub(crate) mod error_response;
pub(crate) mod input_rejection;
pub(crate) mod timer;

pub(crate) fn invoke_handler<R: tauri::Runtime>(
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
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
}

#[cfg(test)]
mod ipc_tests;
