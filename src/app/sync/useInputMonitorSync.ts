import { useEffect } from "react";
import { commands, events } from "@/ipc/generated";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { useInputMonitorStore } from "@/backend/inputMonitorStore";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";

export function useInputMonitorSync(): void {
  const attempt = useInputMonitorStore((state) => state.attempt);
  useEffect(() => {
    let active = true;
    let unlisten: UnlistenFn | undefined = undefined;

    async function connect(): Promise<void> {
      let notificationsAvailable = false;
      try {
        const unsubscribe = await events.inputMonitorFailed.listen(() => {
          if (active) useInputMonitorStore.setState({ failureObserved: true });
        });
        if (!active) {
          unsubscribe();
          return;
        }
        unlisten = unsubscribe;
        notificationsAvailable = true;
      } catch {
        if (active) useInputMonitorStore.setState({ connectionFailed: true });
      }

      if (!active) return;
      await receiveCommandResponse(commands.hasInputMonitorFailed()).then(
        (response) => {
          if (!active) return;
          if (response.status === "accepted") {
            useInputMonitorStore.setState((state) => ({
              failureObserved: state.failureObserved || response.value,
              connectionFailed: !notificationsAvailable,
            }));
          } else {
            useInputMonitorStore.setState({ connectionFailed: true });
          }
        },
      );
    }

    void connect();
    return (): void => {
      active = false;
      unlisten?.();
    };
  }, [attempt]);
}
