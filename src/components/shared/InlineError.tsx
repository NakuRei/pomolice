import { useEffect } from "react";
import { useErrorStore } from "../../store/errorStore";

/** How long a "recoverable" error stays visible before being auto-cleared. */
const RECOVERABLE_AUTO_CLEAR_MS = 5000;

interface InlineErrorProps {
  /** The error ID to look up in the error store (e.g. "start_work", "emergency_stop"). */
  errorId: string;
  /**
   * Optional override for the retry button action.
   * If omitted, the retryFn stored in the error entry is used.
   * If neither is available, no retry button is shown.
   */
  onRetry?: () => void;
}

/**
 * Displays the active error for a given errorId inline inside a dialog or near a button.
 *
 * - Renders nothing when no error with the given id exists.
 * - "recoverable" errors auto-clear after RECOVERABLE_AUTO_CLEAR_MS.
 * - "critical" errors persist until the user retries or the operation succeeds.
 * - Shows a retry button when canRetry is true.
 *
 * Usage example (inside a dialog):
 * ```tsx
 * <InlineError errorId="emergency_stop" onRetry={handleRetry} />
 * ```
 */
export function InlineError({ errorId, onRetry }: InlineErrorProps) {
  const error = useErrorStore((s) => s.errors[errorId]);
  const clearError = useErrorStore((s) => s.clearError);
  const incrementRetryCount = useErrorStore((s) => s.incrementRetryCount);

  // Auto-clear recoverable errors after a timeout
  useEffect(() => {
    if (!error || error.severity !== "recoverable") {
      return;
    }

    const timer = setTimeout(() => {
      clearError(errorId);
    }, RECOVERABLE_AUTO_CLEAR_MS);

    return () => clearTimeout(timer);
  }, [error, errorId, clearError]);

  if (!error) {
    return null;
  }

  const handleRetry = async () => {
    incrementRetryCount(errorId);
    const retryAction = onRetry ?? (error.retryFn ? () => error.retryFn!() : null);
    if (retryAction) {
      retryAction();
    }
  };

  const showRetryButton = error.canRetry && (onRetry != null || error.retryFn != null);

  return (
    <div
      role="alert"
      className="flex items-start gap-2 rounded-md bg-red-50 border border-red-200 px-3 py-2 text-sm text-red-700"
      data-testid={`inline-error-${errorId}`}
    >
      {/* Warning icon */}
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 20 20"
        fill="currentColor"
        className="mt-0.5 h-4 w-4 shrink-0 text-red-500"
        aria-hidden="true"
      >
        <path
          fillRule="evenodd"
          d="M8.485 2.495c.673-1.167 2.357-1.167 3.03 0l6.28 10.875c.673 1.167-.17 2.625-1.516 2.625H3.72c-1.347 0-2.189-1.458-1.515-2.625L8.485 2.495zM10 5a.75.75 0 01.75.75v3.5a.75.75 0 01-1.5 0v-3.5A.75.75 0 0110 5zm0 9a1 1 0 100-2 1 1 0 000 2z"
          clipRule="evenodd"
        />
      </svg>

      <span className="flex-1">{error.message}</span>

      {showRetryButton && (
        <button
          onClick={handleRetry}
          className="shrink-0 font-medium underline hover:no-underline focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-1"
          data-testid={`inline-error-retry-${errorId}`}
        >
          再試行
        </button>
      )}
    </div>
  );
}
