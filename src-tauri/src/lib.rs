// Pomolice - Pomodoro Timer Application
// Phase 2: Desktop notifications
// Phase 3: Fullscreen break enforcement
// Phase 4: Input monitoring and penalty system

mod notifications;
mod window_control;
mod input_monitor;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            notifications::notify_work_complete,
            notifications::notify_break_complete,
            notifications::notify_long_break_complete,
            window_control::enter_fullscreen,
            window_control::exit_fullscreen,
            window_control::set_always_on_top,
            window_control::restore_window,
            window_control::enter_fullscreen_all_monitors,
            window_control::exit_fullscreen_all_monitors,
            input_monitor::start_monitoring,
            input_monitor::stop_monitoring,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
