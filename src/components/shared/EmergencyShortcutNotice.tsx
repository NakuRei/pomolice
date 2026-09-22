import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEmergencyShortcutStore } from "@/store/emergencyShortcutStore";
import type { EmergencyShortcutStatus } from "@/types/emergencyShortcut";
import { StatusNotice } from "./StatusNotice";

export function EmergencyShortcutNotice(): React.ReactElement | null {
  const status = useEmergencyShortcutStore((state) => state.status);
  const connectionFailed = useEmergencyShortcutStore(
    (state) => state.connectionFailed,
  );

  useEffect(() => {
    let active = true;
    let receivedNotification = false;
    let unlisten: UnlistenFn | undefined = undefined;

    async function connect(): Promise<void> {
      let subscribed = false;
      try {
        const unsubscribe = await listen<EmergencyShortcutStatus>(
          "emergency_shortcut_changed",
          (event) => {
            if (!active) return;
            receivedNotification = true;
            useEmergencyShortcutStore.setState({
              status: event.payload,
              connectionFailed: false,
            });
          },
        );
        if (!active) {
          unsubscribe();
          return;
        }
        unlisten = unsubscribe;
        subscribed = true;
      } catch {
        if (active)
          useEmergencyShortcutStore.setState({ connectionFailed: true });
      }
      try {
        const snapshot = await invoke<EmergencyShortcutStatus>(
          "get_emergency_shortcut_status",
        );
        if (active && !receivedNotification) {
          useEmergencyShortcutStore.setState({
            status: snapshot,
            connectionFailed: !subscribed,
          });
        }
      } catch {
        if (active && !receivedNotification)
          useEmergencyShortcutStore.setState({
            status: null,
            connectionFailed: true,
          });
      }
    }

    void connect();
    return (): void => {
      active = false;
      unlisten?.();
    };
  }, []);

  if (status?.kind !== "unavailable" && !connectionFailed) return null;
  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {connectionFailed
          ? "緊急解除のショートカット情報を取得できません。"
          : "緊急解除のショートカットを利用できません。"}
      </StatusNotice>
    </div>
  );
}

export function EmergencyShortcutHint(): React.ReactElement {
  const status = useEmergencyShortcutStore((state) => state.status);
  const connectionFailed = useEmergencyShortcutStore(
    (state) => state.connectionFailed,
  );
  let guidance: React.ReactNode = "確認中";
  if (connectionFailed) {
    guidance = "確認不可";
  } else {
    switch (status?.kind) {
      case "input_monitor":
        guidance = <kbd>Ctrl + Alt + Shift + E</kbd>;
        break;
      case "registered":
        guidance = <kbd>{status.trigger}</kbd>;
        break;
      case "registering":
        guidance = "登録待ち";
        break;
      case "unavailable":
        guidance = "ショートカット利用不可";
        break;
      case undefined:
      default:
        break;
    }
  }
  return (
    <p className="text-xs text-text-muted">
      緊急解除：
      {guidance}
    </p>
  );
}
