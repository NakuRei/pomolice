import { useState } from "react";
import { useErrorStore } from "../../store/errorStore";
import { LoadingSpinnerIcon, WarningTriangleIcon } from "../icons";

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
    <div
      role="alert"
      className="flex items-center justify-between gap-4 rounded-md bg-red-50 border border-red-200 px-4 py-3 text-sm text-red-800"
      data-testid="connection-banner"
    >
      <div className="flex items-center gap-2">
        <WarningTriangleIcon className="h-5 w-5 shrink-0 text-red-500" />
        <span>
          {isRetrying
            ? "接続を試みています..."
            : "バックエンドに接続できません。アプリが正常に動作しない可能性があります。"}
        </span>
      </div>

      {error.canRetry && (
        <button
          onClick={handleRetry}
          disabled={isRetrying}
          className="shrink-0 rounded px-3 py-1.5 font-medium bg-red-100 hover:bg-red-200 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-1 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          data-testid="connection-banner-retry"
        >
          {isRetrying ? (
            <span className="flex items-center gap-1.5">
              <LoadingSpinnerIcon className="h-3.5 w-3.5 animate-spin" />
              接続中
            </span>
          ) : (
            "再試行"
          )}
        </button>
      )}
    </div>
  );
}
