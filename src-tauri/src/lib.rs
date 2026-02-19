// Pomolice - Pomodoro Timer Application
// Backend is the single source of truth for all timer state
// Security: Window control is managed through timer_state module

use tauri::Manager;

mod input_monitor;
mod persistence;
mod timer_state;
mod window_control;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            // Initialize timer state from persisted data on startup
            timer_state::initialize_state(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if window.label() == "main" {
                    let app = window.app_handle();
                    for (label, win) in app.webview_windows() {
                        if label.starts_with("break-overlay-") {
                            let _ = win.close();
                        }
                    }
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            // Timer state management (all state is managed here)
            timer_state::start_work,
            timer_state::continue_to_next_session,
            timer_state::end_cycle,
            timer_state::emergency_stop,
            timer_state::sync_settings,
            timer_state::get_full_state,
            timer_state::get_timer_status,
            timer_state::record_skip_attempt,
            // Legacy commands for backward compatibility
            timer_state::start_break,
            timer_state::complete_break,
            timer_state::complete_work,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
