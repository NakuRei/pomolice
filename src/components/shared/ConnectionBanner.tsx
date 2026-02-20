import { useState } from "react";
import { useErrorStore } from "../../store/errorStore";

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
        {/* Warning icon */}
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 20 20"
          fill="currentColor"
          className="h-5 w-5 shrink-0 text-red-500"
          aria-hidden="true"
        >
          <path
            fillRule="evenodd"
            d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z"
            clipRule="evenodd"
          />
        </svg>
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
              {/* Spinner */}
              <svg
                className="h-3.5 w-3.5 animate-spin"
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                aria-hidden="true"
              >
                <circle
                  className="opacity-25"
                  cx="12"
                  cy="12"
                  r="10"
                  stroke="currentColor"
                  strokeWidth="4"
                />
                <path
                  className="opacity-75"
                  fill="currentColor"
                  d="M4 12a8 8 0 018-8v8H4z"
                />
              </svg>
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
