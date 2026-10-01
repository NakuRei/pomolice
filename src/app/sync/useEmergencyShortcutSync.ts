import { useEffect } from "react";
import { commands, events } from "@/ipc/generated";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useEmergencyShortcutStore } from "@/backend/emergencyShortcutStore";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";

export function useEmergencyShortcutSync(): void {
  useEffect(() => {
    let active = true;
    let receivedNotification = false;
    let unlisten: UnlistenFn | undefined = undefined;

    async function connect(): Promise<void> {
      let subscribed = false;
      try {
        const unsubscribe = await events.emergencyShortcutChanged.listen(
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
      if (!active) return;
      await receiveCommandResponse(commands.getEmergencyShortcutStatus()).then(
        (response) => {
          if (active && !receivedNotification) {
            useEmergencyShortcutStore.setState(
              response.status === "accepted"
                ? { status: response.value, connectionFailed: !subscribed }
                : { status: null, connectionFailed: true },
            );
          }
        },
      );
    }

    void connect();
    return (): void => {
      active = false;
      unlisten?.();
    };
  }, []);
}
