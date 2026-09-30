use std::sync::Arc;

use chrono::{Local, TimeZone, Utc};
use serde_json::{json, Value};
use tauri::http::HeaderMap;
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{get_ipc_response, mock_builder, MockRuntime, INVOKE_KEY};
use tauri::webview::InvokeRequest;
use tauri::{App, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_specta::Event as _;

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
    let ipc = super::builder();
    let invoke_handler = ipc.invoke_handler();
    let app = mock_builder()
        .manage(state)
        .invoke_handler(invoke_handler)
        .build(crate::app_context())
        .expect("app should build with production capabilities");
    ipc.mount_events(&app);
    let window = WebviewWindowBuilder::new(&app, label, WebviewUrl::default())
        .build()
        .expect("test window should build");
    (app, window, before)
}

fn invoke(
    window: &WebviewWindow<MockRuntime>,
    command: &str,
    arguments: Value,
) -> Result<Value, Value> {
    get_ipc_response(
        window,
        InvokeRequest {
            cmd: command.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: if cfg!(any(windows, target_os = "android")) {
                "http://tauri.localhost"
            } else {
                "tauri://localhost"
            }
            .parse()
            .expect("valid local app URL"),
            body: InvokeBody::Json(arguments),
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

    let response = invoke(&window, "retry_failed_timer_effects", json!({}))
        .expect("main should be allowed to retry effects");

    assert_eq!(response, before);
    assert_eq!(response["timer"]["status"], "idle");
    assert_eq!(response["effects"]["revision"], "0");
    assert!(response["effects"]["last_timer_save"].is_null());
    assert!(response["effects"]["break_lock"]["last_attempt"].is_null());
}

#[test]
fn break_overlay_cannot_retry_through_ipc() {
    let (_app, window, _) = app_with_window("break-overlay-1");

    let error = invoke(&window, "retry_failed_timer_effects", json!({}))
        .expect_err("overlay must not be allowed to retry effects");

    let message = error
        .as_str()
        .expect("Tauri ACL should reject before the command runs");
    assert!(message.contains("retry_failed_timer_effects"));
}

#[test]
fn settings_omission_null_and_validation_remain_distinct_over_ipc() {
    let (_app, window, before) = app_with_window("main");
    assert_eq!(
        invoke(&window, "update_settings", json!({"changes": {}})).unwrap(),
        before
    );
    for changes in [
        json!({"notification_sound_muted": null}),
        json!({"work_duration_secs": "60"}),
        json!({"unknown": true}),
    ] {
        let error = invoke(&window, "update_settings", json!({"changes": changes})).unwrap_err();
        assert_eq!(
            error,
            json!({"code": "invalid_command_input", "message": "Invalid command input"})
        );
    }
    let invalid = invoke(
        &window,
        "update_settings",
        json!({"changes": {"work_duration_secs": 0}}),
    )
    .unwrap_err();
    assert_eq!(invalid["code"], "invalid_settings");
    assert_eq!(
        invalid["issues"],
        json!([{"code": "out_of_range", "field": "work_duration_secs", "min": 1, "max": 7200}])
    );
    assert_eq!(
        invoke(&window, "get_full_state", json!({})).unwrap(),
        before
    );
}

#[test]
fn command_arguments_use_the_generated_camel_case_keys_and_enum_values() {
    let (_app, window, before) = app_with_window("main");
    assert_eq!(
        invoke(
            &window,
            "resolve_boot_auto_start_prompt",
            json!({"startNow": false})
        )
        .unwrap(),
        before
    );
    let wrong_key = invoke(
        &window,
        "resolve_boot_auto_start_prompt",
        json!({"start_now": false}),
    )
    .unwrap_err();
    assert_eq!(wrong_key["code"], "invalid_command_input");
    let wrong_action = invoke(
        &window,
        "resolve_post_break_decision",
        json!({"action": "continue_now"}),
    )
    .unwrap_err();
    assert_eq!(wrong_action["code"], "invalid_command_input");
    let wrong_state = invoke(
        &window,
        "resolve_post_break_decision",
        json!({"action": "continue"}),
    )
    .unwrap_err();
    assert_eq!(wrong_state["code"], "invalid_state_transition");
    let wrong_state = invoke(&window, "set_away_during_break", json!({"away": true})).unwrap_err();
    assert_eq!(wrong_state["code"], "invalid_state_transition");
}

#[test]
fn overlay_cannot_invoke_any_application_command() {
    macro_rules! command_names {
        ($($module:ident::$command:ident),* $(,)?) => { &[$(stringify!($command)),*] };
    }
    let (_app, window, _) = app_with_window("break-overlay-1");
    for name in super::registration::with_commands!(command_names) {
        let error = invoke(&window, name, json!({})).unwrap_err();
        assert!(
            error.as_str().is_some_and(|message| message.contains(name)),
            "{name}: {error}"
        );
    }
}

#[test]
fn typed_events_deliver_the_wire_payload_without_a_wrapper_or_precision_loss() {
    use tauri::Listener;
    let (app, _, _) = app_with_window("main");
    let (sender, receiver) = std::sync::mpsc::channel();
    app.listen("timer_effects_changed", move |event| {
        sender
            .send(event.payload().to_string())
            .expect("receiver should exist");
    });
    super::events::TimerEffectsChanged(super::responses::TimerEffectsState {
        revision: super::responses::EffectRevision(u64::MAX),
        ..Default::default()
    })
    .emit(&app)
    .expect("event should emit");
    let payload: Value = serde_json::from_str(
        &receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("event should arrive"),
    )
    .unwrap();
    assert_eq!(payload["revision"], "18446744073709551615");
    assert_eq!(payload["break_lock"]["target"], "unlocked");
    assert!(payload["last_timer_save"].is_null());
}
