// Pomolice - Pomodoro Timer Application
// Phase 2: Desktop notifications

mod notifications;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            notifications::notify_work_complete,
            notifications::notify_break_complete,
            notifications::notify_long_break_complete,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests;
