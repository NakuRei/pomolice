import { useEffect } from "react";
import { commands } from "@/ipc/generated";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
import { useTrayStore } from "@/backend/trayStore";

export function useTraySync(): void {
  const checkAttempt = useTrayStore((state) => state.checkAttempt);
  useEffect(() => {
    let active = true;
    void receiveCommandResponse(commands.isTrayAvailable()).then((response) => {
      if (!active) return;
      if (response.status !== "accepted") {
        useTrayStore.setState({ status: "unknown" });
      } else {
        useTrayStore.setState({
          status: response.value ? "available" : "unavailable",
        });
      }
    });
    return (): void => {
      active = false;
    };
  }, [checkAttempt]);
}
