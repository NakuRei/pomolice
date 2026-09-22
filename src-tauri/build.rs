use std::error::Error;

use tauri_build::{AppManifest, Attributes};

fn main() -> Result<(), Box<dyn Error>> {
    tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(&[
            "is_tray_available",
            "has_input_monitor_failed",
            "get_emergency_shortcut_status",
            "exit_without_tray",
            "start_work",
            "resolve_post_break_decision",
            "set_away_during_break",
            "emergency_stop",
            "sync_timer_duration_settings",
            "sync_automation_policies",
            "get_full_state",
            "resolve_boot_auto_start_prompt",
        ])),
    )?;

    Ok(())
}
