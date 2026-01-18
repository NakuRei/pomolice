// Pomolice - Pomodoro Timer Application
// Phase 2: Desktop notifications
// Phase 3: Fullscreen break enforcement
// Phase 4: Input monitoring and penalty system
// Security: Window control is managed through timer_state module

use tauri::Manager;

mod window_control;
mod input_monitor;
mod timer_state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
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
