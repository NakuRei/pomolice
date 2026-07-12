use tauri::{AppHandle, Emitter};

use crate::state;
use crate::types::TimerTickPayload;

pub(crate) fn emit_timer_tick(app: &AppHandle, tick_payload: &TimerTickPayload) {
    let _ = app.emit("timer_tick", tick_payload);
}

pub(crate) fn update_tray_icon(app: &AppHandle, tick_payload: &TimerTickPayload) {
    state::update_tray_icon(app, tick_payload);
}
