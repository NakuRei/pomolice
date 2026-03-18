use tauri_build::{AppManifest, Attributes};

fn main() {
    tauri_build::try_build(
        Attributes::new().app_manifest(AppManifest::new().commands(&[
            "start_work",
            "resolve_post_break_decision",
            "emergency_stop",
            "sync_timer_duration_settings",
            "sync_automation_policies",
            "get_full_state",
            "resolve_boot_auto_start_prompt",
        ])),
    )
    .expect("failed to run tauri build script");
}
