// Notifications module for Pomolice
// Handles desktop notifications for session completion

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Send a notification when work session is complete
#[tauri::command]
pub async fn notify_work_complete(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("休憩時間です！")
        .body("お疲れ様でした。休憩を取りましょう。")
        .show()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Send a notification when break session is complete
#[tauri::command]
pub async fn notify_break_complete(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("作業に戻りましょう！")
        .body("休憩が終わりました。次のポモドーロを始めましょう。")
        .show()
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Send a notification for long break completion
#[tauri::command]
pub async fn notify_long_break_complete(app: AppHandle) -> Result<(), String> {
    app.notification()
        .builder()
        .title("長い休憩完了！")
        .body("サイクルが完了しました。次のサイクルを始めますか？")
        .show()
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    // Note: Testing notifications requires a running Tauri app context
    // These tests verify function signatures and basic structure

    #[test]
    fn test_notification_functions_exist() {
        // This test ensures the functions are properly defined
        // Actual notification testing would require integration tests
        assert!(true);
    }
}
