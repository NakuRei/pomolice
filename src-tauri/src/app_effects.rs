// Connects the timer runtime to the running Tauri application: the app
// handle is the effect sink, and the ticker and input callbacks are the
// event sources that have no command of their own.

use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};
use tokio::time::MissedTickBehavior;

use crate::break_lock;
use crate::external_adapters::tray_icon;
use crate::ipc::events;
use crate::state::AppState;
use crate::timer_runtime::{EffectSink, TimerNotification};
use pomolice_core::timer_core::Event;
use tauri_specta::Event as _;

const TICK_INTERVAL: Duration = Duration::from_secs(1);

impl<R: Runtime> EffectSink for AppHandle<R> {
    fn notify(&self, notification: TimerNotification) {
        match notification {
            TimerNotification::SnapshotChanged(value) => {
                tray_icon::update_tray_icon(self, &value.timer.state);
                let _ = events::TimerSnapshotChanged(*value).emit(self);
            }
            TimerNotification::SessionCompleted(value) => {
                let _ = events::SessionCompleted(value).emit(self);
            }
            TimerNotification::AutoRunStopPending => {
                let _ = events::AutoRunStopPending.emit(self);
            }
            TimerNotification::AutoRunStopped => {
                let _ = events::AutoRunStopped.emit(self);
            }
        }
    }

    async fn enter_break_lock(&self) -> crate::error::Result<()> {
        break_lock::enter(&self.state::<AppState>(), self).await
    }

    async fn release_break_lock(&self) -> crate::error::Result<()> {
        break_lock::release(&self.state::<AppState>(), self).await
    }
}

pub(crate) fn spawn_ticker(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(TICK_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let app_state = app.state::<AppState>();
            let _ = app_state
                .timer
                .dispatch(
                    &app,
                    Event::TimeObserved {
                        credited_break_seconds: 1,
                    },
                )
                .await;
        }
    });
}

pub(crate) fn dispatch_from_callback(app: AppHandle, event: Event) {
    tauri::async_runtime::spawn(async move {
        let app_state = app.state::<AppState>();
        let _ = app_state.timer.dispatch(&app, event).await;
    });
}
