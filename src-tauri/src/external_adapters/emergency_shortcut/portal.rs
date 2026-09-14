use ashpd::desktop::global_shortcuts::{
    BindShortcutsOptions, GlobalShortcuts, NewShortcut, Shortcut,
};
use ashpd::desktop::CreateSessionOptions;
use futures_util::StreamExt;

use super::EmergencyShortcutStatus;

const SHORTCUT_ID: &str = "emergency-release";

pub(super) async fn run(
    connection: zbus::Connection,
    app_id: &str,
    on_status: impl Fn(EmergencyShortcutStatus),
    on_activate: impl Fn(),
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ashpd::register_host_app_with_connection(connection.clone(), app_id.parse()?).await?;
    let portal = GlobalShortcuts::with_connection(connection).await?;
    let mut owner_changes = portal.receive_owner_changed().await?;
    let mut activations = portal.receive_activated().await?;
    let mut changes = portal.receive_shortcuts_changed().await?;
    let session = portal
        .create_session(CreateSessionOptions::default())
        .await?;
    let mut closed = session.receive_closed().await?;
    let session_path: String = serde_json::from_value(serde_json::to_value(&session)?)?;

    let result = async {
        let shortcuts = [NewShortcut::new(SHORTCUT_ID, "Pomolice 緊急解除")
            .preferred_trigger("CTRL+ALT+SHIFT+e")];
        let response = tokio::select! {
            response = portal.bind_shortcuts(&session, &shortcuts, None, BindShortcutsOptions::default()) => response?.response()?,
            _ = closed.next() => return Err("shortcut session closed during registration".into()),
            _ = owner_changes.next() => return Err("shortcut portal disconnected during registration".into()),
        };
        on_status(status_from_shortcuts(response.shortcuts()));

        loop {
            tokio::select! {
                event = activations.next() => {
                    let Some(event) = event else { return Err("shortcut activation stream ended".into()); };
                    if event.session_handle().as_str() == session_path && event.shortcut_id() == SHORTCUT_ID {
                        on_activate();
                    }
                }
                event = changes.next() => {
                    let Some(event) = event else { return Err("shortcut change stream ended".into()); };
                    if event.session_handle().as_str() == session_path {
                        on_status(status_from_shortcuts(event.shortcuts()));
                    }
                }
                _ = closed.next() => return Err("shortcut session closed".into()),
                _ = owner_changes.next() => return Err("shortcut portal disconnected".into()),
            }
        }
    }.await;

    on_status(EmergencyShortcutStatus::Unavailable);
    if let Err(error) = session.close().await {
        log::warn!("Failed to close emergency shortcut session: {error}");
    }
    result
}

fn status_from_shortcuts(shortcuts: &[Shortcut]) -> EmergencyShortcutStatus {
    shortcuts
        .iter()
        .find(|shortcut| shortcut.id() == SHORTCUT_ID)
        .map(Shortcut::trigger_description)
        .filter(|trigger| !trigger.trim().is_empty())
        .map_or(EmergencyShortcutStatus::Unavailable, |trigger| {
            EmergencyShortcutStatus::Registered {
                trigger: trigger.to_owned(),
            }
        })
}

#[cfg(test)]
mod tests;
