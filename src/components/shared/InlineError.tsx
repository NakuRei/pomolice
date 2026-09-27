import { useEffect } from "react";
import { useErrorStore, type AppError } from "@/store/errorStore";
import { WarningTriangleIcon } from "@/components/icons/AppIcons";
import { StatusNotice } from "./StatusNotice";

/** How long a "recoverable" error stays visible before being auto-cleared. */
const RECOVERABLE_AUTO_CLEAR_MS = 5000;

interface InlineErrorProps {
  /** The error ID to look up in the error store (e.g. "start_work", "emergency_stop"). */
  errorId: string;
  /**
   * Optional override for the retry button action.
   * If omitted, the retryFn stored in the error entry is used.
   * Null disables retry when the action is no longer available.
   * If neither is available, no retry button is shown.
   */
  onRetry?: (() => void | Promise<void>) | null;
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
export function InlineError({
  errorId,
  onRetry,
}: InlineErrorProps): React.ReactElement | null {
  const error = useErrorStore(
    (state): AppError | undefined => state.errors[errorId],
  );
  const clearError = useErrorStore((s) => s.clearError);
  const incrementRetryCount = useErrorStore((s) => s.incrementRetryCount);

  // Auto-clear recoverable errors after a timeout
  useEffect(() => {
    if (error?.severity !== "recoverable") {
      return;
    }

    const timer = setTimeout(() => {
      clearError(errorId);
    }, RECOVERABLE_AUTO_CLEAR_MS);

    return (): void => {
      clearTimeout(timer);
    };
  }, [error, errorId, clearError]);

  if (error === undefined) {
    return null;
  }

  const retryAction = onRetry === undefined ? error.retryFn : onRetry;
  const handleRetry = (): void => {
    incrementRetryCount(errorId);
    if (retryAction !== null) {
      void (async (): Promise<void> => {
        await retryAction();
      })().catch(() => undefined);
    }
  };

  const showRetryButton = error.canRetry && retryAction !== null;

  return (
    <StatusNotice
      tone="danger"
      role="alert"
      testId={`inline-error-${errorId}`}
      icon={
        <WarningTriangleIcon className="mt-0.5 h-4 w-4 shrink-0 text-danger-400" />
      }
      actions={
        showRetryButton ? (
          <button
            onClick={() => {
              handleRetry();
            }}
            className="shrink-0 font-medium underline hover:no-underline focus:ring-2 focus:ring-danger-500 focus:ring-offset-1 focus:outline-none"
            data-testid={`inline-error-retry-${errorId}`}
          >
            再試行
          </button>
        ) : undefined
      }
    >
      {error.message}
    </StatusNotice>
  );
}
