import { useEffect } from "react";
import { useTimerNoticeStore } from "@/store/timerNoticeStore";
import { StatusNotice } from "./StatusNotice";

export function InfoMessageBanner(): React.ReactElement | null {
  const infoMessage = useTimerNoticeStore((state) => state.infoMessage);
  const clearInfoMessage = useTimerNoticeStore(
    (state) => state.clearInfoMessage,
  );

  useEffect(() => {
    if (!infoMessage) {
      return;
    }

    const timeoutId = window.setTimeout(() => {
      clearInfoMessage(infoMessage.id);
    }, infoMessage.ttlMs);

    return (): void => {
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
