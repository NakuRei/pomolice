use std::error::Error;
use std::io::Write;

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
            "update_settings",
            "get_full_state",
            "resolve_boot_auto_start_prompt",
            "retry_failed_timer_effects",
        ])),
    )?;

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let mut output = std::io::stdout().lock();
        let output_dir = std::env::var("OUT_DIR")?;
        writeln!(output, "cargo:rustc-link-search=native={output_dir}")?;
    }

    Ok(())
}
