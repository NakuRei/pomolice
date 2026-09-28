/**
 * Timer Sync Hook
 * Sets up event listeners to receive timer state from backend
 * The backend is the single source of truth for all timer state
 */

import { useEffect } from "react";
import { useTimerEffectsStore } from "@/store/timerEffectsStore";
import type { TimerSnapshot, TimerEffectsState } from "@/types/timerEffects";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { createLogger } from "../logging/logger";
import { FRONTEND_LOG_EVENT_IDS } from "../logging/logEvents";
import { useTimerStore } from "../store/timerStore";
import {
  type ActiveWindowChangedPayload,
  type SettingsChangedPayload,
  type TimerTickPayload,
  type SessionCompletedPayload,
} from "@/types/timer";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "../services/notifications";
import {
  shouldRetryTimerSyncConnection,
  TIMER_SYNC_MAX_CONNECTION_RETRIES,
  timerSyncRetryDelayMs,
} from "./timerSyncConnectionRetryPolicy";
import {
  clearTimerSyncConnectionError,
  formatTimerSyncConnectionErrorDetail,
  reportTimerSyncConnectionError,
} from "./timerSyncConnectionError";
import {
  classifyBackendError,
  isBackendErrorResponse,
} from "../errors/classifyBackendError";

const logger = createLogger("timer_sync");

/**
 * Hook to sync timer state from backend.
 * Call this once in your app root component.
 *
 * On failure, automatically retries using the timer sync connection retry
 * policy. After exhausting retries, reports a "connection" error to the error
 * store so the UI can surface a ConnectionBanner.
 */
export function useTimerSync(): void {
  const observeEffects = useTimerEffectsStore((state) => state.observe);
  const handleTimerTick = useTimerStore((s) => s.handleTimerTick);
  const handleSessionCompleted = useTimerStore((s) => s.handleSessionCompleted);
  const handleFullState = useTimerStore((s) => s.handleFullState);
  const clearBootAutoStartOffer = useTimerStore(
    (s) => s.clearBootAutoStartOffer,
  );
  const handleActiveWindowChanged = useTimerStore(
    (s) => s.handleActiveWindowChanged,
  );
  const handleSettingsChanged = useTimerStore((s) => s.handleSettingsChanged);
  const setConnected = useTimerStore((s) => s.setConnected);
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

      const unlistenActiveWindowChanged =
        await listen<ActiveWindowChangedPayload>(
          "active_window_changed",
          (event) => {
            if (mounted) {
              handleActiveWindowChanged(event.payload);
            }
          },
        );
      if (!registerActiveListener(unlistenActiveWindowChanged)) {
        return false;
      }

      const unlistenSettingsChanged = await listen<SettingsChangedPayload>(
        "settings_changed",
        (event) => {
          if (mounted) {
            handleSettingsChanged(event.payload);
          }
        },
      );
      if (!registerActiveListener(unlistenSettingsChanged)) {
        return false;
      }

      const unlistenBootOfferClosed = await listen(
        "boot_auto_start_offer_closed",
        () => {
          if (mounted) {
            clearBootAutoStartOffer();
          }
        },
      );
      if (!registerActiveListener(unlistenBootOfferClosed)) return false;
      const unlistenEffects = await listen<TimerEffectsState>(
        "timer_effects_changed",
        (event) => {
          if (mounted) observeEffects(event.payload);
        },
      );
      return registerActiveListener(unlistenEffects);
    }

    function shouldRetryTimerSyncSetupFailure(
      error: unknown,
      failureSource: "setup" | "command" | "command_transport",
      retryAttempt: number,
    ): boolean {
      if (!shouldRetryTimerSyncConnection(retryAttempt)) {
        return false;
      }

      return (
        failureSource !== "command" || classifyBackendError(error).canRetry
      );
    }

    async function setup(retryAttempt = 0): Promise<void> {
      const listenerStartIndex = listeners.length;
      let failureSource: "setup" | "command" | "command_transport" = "setup";

      try {
        const listenersRegistered = await registerBackendListeners();
        if (!listenersRegistered) {
          return;
        }

        failureSource = "command_transport";
        const fullState = await invoke<TimerSnapshot>("get_full_state");
        failureSource = "setup";
        if (!mounted) {
          return;
        }
        handleFullState(fullState);
        clearTimerSyncConnectionError();
      } catch (error) {
        if (
          failureSource === "command_transport" &&
          isBackendErrorResponse(error)
        ) {
          failureSource = "command";
        }
        logger.error(FRONTEND_LOG_EVENT_IDS.timerSyncSetupFailed, error, {
          retry_attempt: retryAttempt,
          max_connection_retries: TIMER_SYNC_MAX_CONNECTION_RETRIES,
          failure_message: formatTimerSyncConnectionErrorDetail(
            error,
            failureSource,
          ),
        });

        clearListenersFrom(listenerStartIndex);

        if (!mounted) {
          return;
        }

        if (
          shouldRetryTimerSyncSetupFailure(error, failureSource, retryAttempt)
        ) {
          clearRetryTimeout();
          retryTimeoutId = setTimeout(() => {
            retryTimeoutId = null;
            if (mounted) {
              void setup(retryAttempt + 1);
            }
          }, timerSyncRetryDelayMs(retryAttempt));
        } else {
          setConnected(false);
          reportTimerSyncConnectionError(
            error,
            async () => {
              clearTimerSyncConnectionError();
              await setup(0);
            },
            failureSource,
          );
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
    clearBootAutoStartOffer,
    observeEffects,
    handleActiveWindowChanged,
    handleSettingsChanged,
    setConnected,
    showAutoRunStopPendingInfo,
    showAutoRunStoppedInfo,
  ]);
}
