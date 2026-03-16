import { useEffect } from "react";
import { useTimerStore } from "../../store/timerStore";
import { StatusNotice } from "./StatusNotice";

/**
 * Displays short-lived informational messages from timerStore.
 * This is intentionally separate from errorStore-based banners.
 */
export function InfoMessageBanner() {
  const infoMessage = useTimerStore((state) => state.infoMessage);
  const clearInfoMessage = useTimerStore((state) => state.clearInfoMessage);

  useEffect(() => {
    if (!infoMessage) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      clearInfoMessage(infoMessage.id);
    }, infoMessage.ttlMs);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [clearInfoMessage, infoMessage]);

  if (!infoMessage) {
    return null;
  }

  return (
    <StatusNotice
      tone="info"
      role="status"
      ariaLive="polite"
      testId="info-message-banner"
    >
      {infoMessage.message}
    </StatusNotice>
  );
}
