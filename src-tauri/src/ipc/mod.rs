#[cfg(test)]
mod bindings;
mod commands;
pub(crate) mod errors;
pub(crate) mod events;
pub(crate) mod input;
mod registration;
pub(crate) mod result;

pub(crate) fn builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    use commands::{application, timer};
    // Specta needs a concrete runtime for metadata; Tauri infers R for the actual handlers.
    macro_rules! collect_runtime_commands {
        ($($module:ident::$command:ident),* $(,)?) => {
            tauri_specta::collect_commands![$($module::$command::<tauri::Wry>),*]
        };
    }
    tauri_specta::Builder::new()
        .commands(registration::with_commands!(collect_runtime_commands))
        .events(tauri_specta::collect_events![
            events::TimerSnapshotChanged,
            events::SessionCompleted,
            events::EmergencyShortcutChanged,
            events::AutoRunStopPending,
            events::AutoRunStopped,
            events::InputMonitorFailed
        ])
}

#[cfg(test)]
mod tests;
