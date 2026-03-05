/**
 * Timer Sync Hook
 * Sets up event listeners to receive timer state from backend
 * The backend is the single source of truth for all timer state
 */

import { useEffect } from "react";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import {
  useTimerStore,
  TimerTickPayload,
  SessionCompletedPayload,
  FullStatePayload,
} from "../store/timerStore";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "../services/notifications";
import { useErrorStore } from "../store/errorStore";

/** Maximum number of automatic retry attempts before giving up and showing the error banner. */
const MAX_CONNECTION_RETRIES = 3;

/**
 * Delay in ms before each retry attempt.
 * Follows exponential backoff: attempt 0 → 1s, attempt 1 → 2s, attempt 2 → 4s.
 */
function retryDelayMs(attempt: number): number {
  return 1000 * Math.pow(2, attempt);
}

/**
 * Hook to sync timer state from backend.
 * Call this once in your app root component.
 *
 * On failure, automatically retries up to MAX_CONNECTION_RETRIES times using
 * exponential backoff. After exhausting retries, reports a "connection" error
 * to the error store so the UI can surface a ConnectionBanner.
 */
export function useTimerSync() {
  const handleTimerTick = useTimerStore((s) => s.handleTimerTick);
  const handleSessionCompleted = useTimerStore((s) => s.handleSessionCompleted);
  const handleFullState = useTimerStore((s) => s.handleFullState);
  const setConnected = useTimerStore((s) => s.setConnected);
  const syncSettings = useTimerStore((s) => s.syncSettings);
  const showAutoRunStopPendingInfo = useTimerStore((s) => s.showAutoRunStopPendingInfo);
  const showAutoRunStoppedInfo = useTimerStore((s) => s.showAutoRunStoppedInfo);

  useEffect(() => {
    const listeners: UnlistenFn[] = [];
    let mounted = true;

    const setup = async (retryAttempt: number = 0) => {
      try {
        // Listen for timer tick events
        const unlistenTick = await listen<TimerTickPayload>(
          "timer_tick",
          (event) => {
            if (mounted) {
              handleTimerTick(event.payload);
            }
          }
        );
        listeners.push(unlistenTick);

        // Listen for session completed events
        const unlistenSessionCompleted = await listen<SessionCompletedPayload>(
          "session_completed",
          (event) => {
            if (mounted) {
              handleSessionCompleted(event.payload);

              // Notification routing is decided only by transition_reason.
              const { transition_reason, next_session_type } = event.payload;
              if (transition_reason === "work_to_break") {
                notifyWorkComplete(next_session_type);
              } else if (transition_reason === "break_waiting_decision") {
                notifyBreakComplete();
              } else if (transition_reason === "break_auto_started") {
                notifyBreakAutoStarted();
              }
            }
          }
        );
        listeners.push(unlistenSessionCompleted);

        const unlistenAutoRunStopPending = await listen("auto_run_stop_pending", () => {
          if (mounted) {
            showAutoRunStopPendingInfo();
          }
        });
        listeners.push(unlistenAutoRunStopPending);

        const unlistenAutoRunStopped = await listen("auto_run_stopped", () => {
          if (mounted) {
            showAutoRunStoppedInfo();
          }
        });
        listeners.push(unlistenAutoRunStopped);

        // Listen for break started events (for UI updates)
        const unlistenBreakStarted = await listen("break_started", () => {
          // State is already updated via timer_tick
        });
        listeners.push(unlistenBreakStarted);

        // Listen for break completed events (for UI updates)
        const unlistenBreakCompleted = await listen("break_completed", () => {
          // State is already updated via timer_tick
        });
        listeners.push(unlistenBreakCompleted);

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
        console.error("Failed to setup timer sync:", error);

        if (!mounted) {
          return;
        }

        if (retryAttempt < MAX_CONNECTION_RETRIES) {
          // Schedule the next retry with exponential backoff
          const delay = retryDelayMs(retryAttempt);
          setTimeout(() => {
            if (mounted) {
              setup(retryAttempt + 1);
            }
          }, delay);
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
    };

    setup();

    return () => {
      mounted = false;
      listeners.forEach((unlisten) => unlisten());
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
