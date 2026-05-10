/**
 * Timer Sync Hook
 * Sets up event listeners to receive timer state from backend
 * The backend is the single source of truth for all timer state
 */

import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../logging/logger";
import { FRONTEND_LOG_EVENT_IDS } from "../logging/logEvents";
import {
  type TimerTickPayload,
  type SessionCompletedPayload,
  type FullStatePayload,
  useTimerStore,
} from "../store/timerStore";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "../services/notifications";
import { useErrorStore } from "../store/errorStore";

/** Maximum number of automatic retry attempts before giving up and showing the error banner. */
const MAX_CONNECTION_RETRIES = 3;
const logger = createLogger("timer_sync");

/**
 * Delay in ms before each retry attempt.
 * Follows exponential backoff: attempt 0 → 1s, attempt 1 → 2s, attempt 2 → 4s.
 */
function retryDelayMs(attempt: number): number {
  return 1000 * 2 ** attempt;
}

/**
 * Hook to sync timer state from backend.
 * Call this once in your app root component.
 *
 * On failure, automatically retries up to MAX_CONNECTION_RETRIES times using
 * exponential backoff. After exhausting retries, reports a "connection" error
 * to the error store so the UI can surface a ConnectionBanner.
 */
export function useTimerSync(): void {
  const handleTimerTick = useTimerStore((s) => s.handleTimerTick);
  const handleSessionCompleted = useTimerStore((s) => s.handleSessionCompleted);
  const handleFullState = useTimerStore((s) => s.handleFullState);
  const setConnected = useTimerStore((s) => s.setConnected);
  const syncSettings = useTimerStore((s) => s.syncSettings);
  const showAutoRunStopPendingInfo = useTimerStore(
    (s) => s.showAutoRunStopPendingInfo,
  );
  const showAutoRunStoppedInfo = useTimerStore((s) => s.showAutoRunStoppedInfo);

  useEffect(() => {
    const listeners: UnlistenFn[] = [];
    let mounted = true;
    let retryTimeoutId: ReturnType<typeof setTimeout> | null = null;

    function clearRetryTimeout(): void {
      if (retryTimeoutId !== null) {
        clearTimeout(retryTimeoutId);
        retryTimeoutId = null;
      }
    }

    function clearListenersFrom(startIndex: number): void {
      const listenersToClear = listeners.splice(startIndex);
      listenersToClear.forEach((unlisten) => {
        unlisten();
      });
    }

    function registerActiveListener(unlisten: UnlistenFn): boolean {
      if (!mounted) {
        unlisten();
        return false;
      }

      listeners.push(unlisten);
      return true;
    }

    async function registerBackendListeners(): Promise<boolean> {
      // Listen for timer tick events
      const unlistenTick = await listen<TimerTickPayload>(
        "timer_tick",
        (event) => {
          if (mounted) {
            handleTimerTick(event.payload);
          }
        },
      );
      if (!registerActiveListener(unlistenTick)) {
        return false;
      }

      // Listen for session completed events
      const unlistenSessionCompleted = await listen<SessionCompletedPayload>(
        "session_completed",
        (event) => {
          if (mounted) {
            handleSessionCompleted(event.payload);

            // Notification routing is decided only by transition_reason.
            const { transition_reason, next_session_type } = event.payload;
            if (transition_reason === "work_to_break") {
              void notifyWorkComplete(next_session_type);
            } else if (transition_reason === "break_waiting_decision") {
              void notifyBreakComplete();
            } else if (transition_reason === "break_auto_started") {
              void notifyBreakAutoStarted();
            }
          }
        },
      );
      if (!registerActiveListener(unlistenSessionCompleted)) {
        return false;
      }

      const unlistenAutoRunStopPending = await listen(
        "auto_run_stop_pending",
        () => {
          if (mounted) {
            showAutoRunStopPendingInfo();
          }
        },
      );
      if (!registerActiveListener(unlistenAutoRunStopPending)) {
        return false;
      }

      const unlistenAutoRunStopped = await listen("auto_run_stopped", () => {
        if (mounted) {
          showAutoRunStoppedInfo();
        }
      });
      if (!registerActiveListener(unlistenAutoRunStopped)) {
        return false;
      }

      // Listen for break started events (for UI updates)
      const unlistenBreakStarted = await listen("break_started", () => {
        // State is already updated via timer_tick
      });
      if (!registerActiveListener(unlistenBreakStarted)) {
        return false;
      }

      // Listen for break completed events (for UI updates)
      const unlistenBreakCompleted = await listen("break_completed", () => {
        // State is already updated via timer_tick
      });
      if (!registerActiveListener(unlistenBreakCompleted)) {
        return false;
      }

      return true;
    }

    function scheduleRetry(
      currentRetryAttempt: number,
      delay: number,
      startNextAttempt: (nextRetryAttempt: number) => void,
    ): void {
      clearRetryTimeout();
      retryTimeoutId = setTimeout(() => {
        retryTimeoutId = null;
        if (mounted) {
          startNextAttempt(currentRetryAttempt + 1);
        }
      }, delay);
    }

    async function setup(retryAttempt = 0): Promise<void> {
      const listenerStartIndex = listeners.length;

      try {
        const listenersRegistered = await registerBackendListeners();
        if (!listenersRegistered) {
          return;
        }

        // Get initial state from backend
        const fullState = await invoke<FullStatePayload>("get_full_state");
        if (mounted) {
          handleFullState(fullState);

          // Sync frontend settings to backend
          await syncSettings();
        }

        if (mounted) {
          setConnected(true);
          // Clear any lingering connection error from a previous failed attempt
          useErrorStore.getState().clearError("connection");
        }
      } catch (error) {
        logger.error(FRONTEND_LOG_EVENT_IDS.timerSyncSetupFailed, error, {
          retry_attempt: retryAttempt,
          max_connection_retries: MAX_CONNECTION_RETRIES,
          failure_message: String(error),
        });

        clearListenersFrom(listenerStartIndex);

        if (!mounted) {
          return;
        }

        if (retryAttempt < MAX_CONNECTION_RETRIES) {
          // Schedule the next retry with exponential backoff
          const delay = retryDelayMs(retryAttempt);
          scheduleRetry(retryAttempt, delay, (nextRetryAttempt) => {
            void setup(nextRetryAttempt);
          });
        } else {
          // All retries exhausted — surface the error to the user
          setConnected(false);
          useErrorStore.getState().reportError({
            id: "connection",
            message: "バックエンドに接続できません",
            technicalDetail: String(error),
            severity: "critical",
            origin: "backend_connection",
            canRetry: true,
            // retryFn is set here so ConnectionBanner can trigger a fresh attempt
            retryFn: async () => {
              useErrorStore.getState().clearError("connection");
              await setup(0);
            },
          });
        }
      }
    }

    void setup();

    return (): void => {
      mounted = false;
      clearRetryTimeout();
      clearListenersFrom(0);
    };
  }, [
    handleTimerTick,
    handleSessionCompleted,
    handleFullState,
    setConnected,
    showAutoRunStopPendingInfo,
    showAutoRunStoppedInfo,
    syncSettings,
  ]);
}
