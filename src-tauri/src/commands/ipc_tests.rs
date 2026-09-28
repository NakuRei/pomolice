use std::sync::Arc;

use chrono::{Local, TimeZone, Utc};
use serde_json::Value;
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::external_adapters::clock::testing::FixedClock;
use crate::external_adapters::persistence::PersistenceManager;
use crate::state::AppState;

// Tauri embeds Common Controls v6 in app binaries; its unit-test executable needs the same resource.
#[cfg(all(target_os = "windows", target_env = "msvc"))]
#[link(name = "resource", kind = "dylib")]
extern "C" {}

fn app_with_window(label: &str) -> (App<MockRuntime>, WebviewWindow<MockRuntime>, Value) {
    let now = Utc
        .with_ymd_and_hms(2026, 1, 1, 9, 0, 0)
        .single()
        .expect("valid test time")
        .with_timezone(&Local);
    let (state, _) = tauri::async_runtime::block_on(AppState::build_startup_state_for_tests(
        Arc::new(FixedClock::new(now)),
        PersistenceManager::new(None),
        PersistenceManager::new(None),
    ));
    let before = serde_json::to_value(tauri::async_runtime::block_on(
        state.timer.read_full_state(),
    ))
    .expect("snapshot serializes");
    let app = mock_builder()
        .manage(state)
        .invoke_handler(super::invoke_handler())
        .build(crate::app_context())
        .expect("app should build with production capabilities");
    let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::default())
        .build()
        .expect("test window should build");
    (app, window, before)
}

fn retry(window: &WebviewWindow<MockRuntime>) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: "retry_failed_timer_effects".into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("valid local app URL"),
            body: InvokeBody::default(),
            headers: HeaderMap::default(),
            invoke_key: INVOKE_KEY.into(),
        },
    )
    .map(|body| {
        body.deserialize()
            .expect("snapshot response should be JSON")
    })
}

#[test]
fn main_can_retry_through_ipc_without_changing_a_healthy_timer() {
    let (_app, window, before) = app_with_window("main");

    let response = retry(&window).expect("main should be allowed to retry effects");

    assert_eq!(response, before);
    assert_eq!(response["timer"]["status"], "idle");
    assert_eq!(response["effects"]["revision"], 0);
    assert!(response["effects"]["last_timer_save"].is_null());
    assert!(response["effects"]["break_lock"]["last_attempt"].is_null());
}

#[test]
fn break_overlay_cannot_retry_through_ipc() {
    let (_app, window, _) = app_with_window("break-overlay-1");

    let error = retry(&window).expect_err("overlay must not be allowed to retry effects");

    let message = error
        .as_str()
        .expect("Tauri ACL should reject before the command runs");
    assert!(message.contains("retry_failed_timer_effects"));
}
