import { useEffect } from "react";
import { useTimerStore } from "../../store/timerStore";

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
    <div
      role="status"
      aria-live="polite"
      className="rounded-md border border-sky-200 bg-sky-50 px-4 py-3 text-sm text-sky-900"
      data-testid="info-message-banner"
    >
      {infoMessage.message}
    </div>
  );
}
