import { useTimerSyncStore } from "@/store/timerSyncStore";
import { describeTimerSyncFailure } from "@/hooks/timerSyncConnectionError";
import { Button } from "./Button";
import {
  LoadingSpinnerIcon,
  WarningTriangleIcon,
} from "@/components/icons/AppIcons";
import { StatusNotice } from "./StatusNotice";

export function ConnectionBanner(): React.ReactElement | null {
  const connection = useTimerSyncStore((state) => state.connection);
  const reconnect = useTimerSyncStore((state) => state.reconnect);
  if (connection.status === "idle" || connection.status === "ready")
    return null;
  const connecting = connection.status !== "failed";
  const failure =
    connection.status === "failed"
      ? describeTimerSyncFailure(connection.failure)
      : null;
  return (
    <StatusNotice
      tone="danger"
      role="alert"
      testId="connection-banner"
      icon={
        <WarningTriangleIcon className="h-5 w-5 shrink-0 text-danger-400" />
      }
      actions={
        connecting || failure?.canRetry === true ? (
          <Button
            appearance="secondary"
            size="sm"
            onClick={reconnect}
            disabled={connecting}
            className="shrink-0 border-danger-800 bg-danger-950/40 text-danger-100 hover:border-danger-700 hover:bg-danger-950/60 focus-visible:outline-danger-500 disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text"
            testId="connection-banner-retry"
          >
            {connecting ? (
              <span className="flex items-center gap-1.5">
                <LoadingSpinnerIcon className="h-3.5 w-3.5 animate-spin" />
                接続中
              </span>
            ) : (
              "再試行"
            )}
          </Button>
        ) : undefined
      }
    >
      {connecting ? "接続を試みています..." : failure?.userMessage}
    </StatusNotice>
  );
}
