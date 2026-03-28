import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

/**
 * How severe an error is, which determines the UI treatment:
 * - "critical"    → blocking inline error with retry (e.g. emergency stop failed)
 * - "recoverable" → non-blocking inline error, auto-clears after timeout
 * - "silent"      → no UI; callers may still record diagnostics separately
 */
export type ErrorSeverity = "critical" | "recoverable" | "silent";

/**
 * Where the error originated, used for retry strategy decisions:
 * - "backend_command"    → user-initiated invoke() call (start, stop, etc.)
 * - "backend_connection" → initial sync / event listener setup
 * - "settings_sync"      → syncSettings invoke (intentionally silent)
 * - "notification"       → Tauri notification API (intentionally silent)
 */
export type ErrorOrigin =
  | "backend_command"
  | "backend_connection"
  | "settings_sync"
  | "notification";

/**
 * A single active error entry.
 *
 * Example usage:
 * ```ts
 * {
 *   id: "emergency_stop",
 *   message: "緊急停止に失敗しました",
 *   technicalDetail: "Timer not running",
 *   severity: "critical",
 *   origin: "backend_command",
 *   retryCount: 0,
 *   canRetry: true,
 *   retryFn: async () => { await emergencyStop(); },
 *   timestamp: Date.now(),
 * }
 * ```
 */
export interface AppError {
  /** Stable key used to look up this error. Corresponds to the operation that failed. */
  id: string;
  /** Japanese user-facing message displayed in the UI. */
  message: string;
  /** Additional diagnostic detail for troubleshooting. reportError() itself does not log it anywhere. */
  technicalDetail: string;
  severity: ErrorSeverity;
  origin: ErrorOrigin;
  /** How many times this error has been retried so far. */
  retryCount: number;
  /** Whether a retry button should be shown. */
  canRetry: boolean;
  /** Async function to call when the user clicks retry. Null if canRetry is false. */
  retryFn: (() => Promise<void>) | null;
  timestamp: number;
}

interface ErrorState {
  /**
   * Active errors keyed by their ID.
   * Only one error per ID is kept — a new reportError() with the same ID overwrites the old one.
   */
  errors: Record<string, AppError>;
}

interface ErrorActions {
  /**
   * Register an error. If an error with the same id already exists, it is replaced.
   * Pass all fields except `timestamp` and `retryCount`, which are set automatically.
   */
  reportError: (error: Omit<AppError, "timestamp" | "retryCount">) => void;
  /** Remove the error with the given id. No-op if id does not exist. */
  clearError: (id: string) => void;
  /** Remove all active errors. */
  clearAllErrors: () => void;
  /** Increment the retry counter for an error. Called by the retry handler before invoking retryFn. */
  incrementRetryCount: (id: string) => void;
}

type ErrorStore = ErrorState & ErrorActions;

const initialState: ErrorState = {
  errors: {},
};

const noopDevtools = (<T>(config: T): T => config) as typeof devtools;
const conditionalDevtools = import.meta.env.DEV ? devtools : noopDevtools;

export const useErrorStore = create<ErrorStore>()(
  conditionalDevtools(
    immer((set) => ({
      ...initialState,

      reportError: (error): void => {
        set((state) => {
          state.errors[error.id] = {
            ...error,
            retryCount: 0,
            timestamp: Date.now(),
          };
        });
      },

      clearError: (id): void => {
        set((state) => {
          Reflect.deleteProperty(state.errors, id);
        });
      },

      clearAllErrors: (): void => {
        set((state) => {
          state.errors = {};
        });
      },

      incrementRetryCount: (id): void => {
        set((state) => {
          const activeError = state.errors[id];
          if (activeError === undefined) {
            return;
          }

          activeError.retryCount += 1;
        });
      },
    })),
    { name: "ErrorStore" },
  ),
);
