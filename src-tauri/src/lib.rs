// Pomolice - Pomodoro Timer Application
// Phase 2: Desktop notifications
// Phase 3: Fullscreen break enforcement
// Phase 4: Input monitoring and penalty system
// Security: Window control is managed through timer_state module

mod window_control;
mod input_monitor;
mod timer_state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            // High-level timer state management (ONLY these are exposed to frontend)
            timer_state::start_break,
            timer_state::complete_break,
            timer_state::start_work,
            timer_state::complete_work,
            timer_state::get_timer_status,
            timer_state::record_skip_attempt,
            // Note: window_control functions are NOT exposed - internal use only
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
