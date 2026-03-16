import { useState } from "react";
import { useErrorStore } from "../../store/errorStore";
import { Button } from "./Button";
import { LoadingSpinnerIcon, WarningTriangleIcon } from "../icons";
import { StatusNotice } from "./StatusNotice";

/**
 * Displays a top-of-page banner when the backend connection has failed.
 *
 * Renders nothing while there is no "connection" error in the store.
 *
 * Two states:
 * 1. Retrying (retryFn present, no active retry in progress) → shows "再試行" button.
 * 2. Retrying in progress → shows a spinner and disables the button.
 *
 * The retry is triggered by calling the retryFn stored in the error entry,
 * which was set by useTimerSync to restart the setup() flow from the beginning.
 */
export function ConnectionBanner() {
  const error = useErrorStore((s) => s.errors["connection"]);
  const [isRetrying, setIsRetrying] = useState(false);

  if (!error) {
    return null;
  }

  const handleRetry = async () => {
    if (!error.retryFn || isRetrying) {
      return;
    }

    setIsRetrying(true);
    try {
      await error.retryFn();
    } finally {
      setIsRetrying(false);
    }
  };

  return (
    <StatusNotice
      tone="danger"
      role="alert"
      testId="connection-banner"
      icon={
        <WarningTriangleIcon className="h-5 w-5 shrink-0 text-danger-400" />
      }
      actions={
        error.canRetry ? (
          <Button
            appearance="secondary"
            size="sm"
            onClick={handleRetry}
            disabled={isRetrying}
            className="shrink-0 border-danger-800 bg-danger-950/40 text-danger-100 hover:border-danger-700 hover:bg-danger-950/60 focus-visible:outline-danger-500 disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text"
            testId="connection-banner-retry"
          >
            {isRetrying ? (
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
      {isRetrying
        ? "接続を試みています..."
        : "バックエンドに接続できません。アプリが正常に動作しない可能性があります。"}
    </StatusNotice>
  );
}
