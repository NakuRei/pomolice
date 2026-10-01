import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
/**
 * Timer Sync Hook
 * Sets up event listeners to receive timer state from backend
 * The backend is the single source of truth for all timer state
 */

import { useEffect } from "react";
import {
  useTimerSyncStore,
  type TimerSyncFailure,
} from "@/store/timerSyncStore";
import { useTimerNoticeStore } from "@/store/timerNoticeStore";

import { type UnlistenFn } from "@tauri-apps/api/event";
import { commands, events } from "@/ipc/generated";
import { createLogger } from "../logging/logger";
import { FRONTEND_LOG_EVENT_IDS } from "../logging/logEvents";
import { useTimerObservationStore } from "../store/timerObservationStore";

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
import { formatTimerSyncConnectionErrorDetail } from "./timerSyncConnectionError";
import { classifyCommandFailure } from "../errors/classifyBackendError";

const logger = createLogger("timer_sync");

/**
 * Hook to sync timer state from backend.
 * Call this once in your app root component.
 *
 */
export function useTimerSync(): void {
  const observeEffects = useTimerObservationStore(
    (state) => state.observeEffects,
  );
  const generation = useTimerSyncStore((state) => state.generation);
  const setConnection = useTimerSyncStore((state) => state.setConnection);
  const observeTimerTick = useTimerObservationStore((s) => s.observeTimerTick);
  const observeSessionCompleted = useTimerObservationStore(
    (s) => s.observeSessionCompleted,
  );
  const observeFullState = useTimerObservationStore((s) => s.observeFullState);
  const clearBootAutoStartOffer = useTimerObservationStore(
    (s) => s.clearBootAutoStartOffer,
  );
  const observeActiveWindowChanged = useTimerObservationStore(
    (s) => s.observeActiveWindowChanged,
  );
  const observeSettingsChanged = useTimerObservationStore(
    (s) => s.observeSettingsChanged,
  );
  const showAutoRunStopPendingInfo = useTimerNoticeStore(
    (s) => s.showAutoRunStopPendingInfo,
  );
  const showAutoRunStoppedInfo = useTimerNoticeStore(
    (s) => s.showAutoRunStoppedInfo,
  );

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
      const unlistenTick = await events.timerTick.listen((event) => {
        if (mounted) {
          observeTimerTick(event.payload);
        }
      });
      if (!registerActiveListener(unlistenTick)) {
        return false;
      }

      // Listen for session completed events
      const unlistenSessionCompleted = await events.sessionCompleted.listen(
        (event) => {
          if (mounted) {
            observeSessionCompleted(event.payload);

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

      const unlistenAutoRunStopPending = await events.autoRunStopPending.listen(
        () => {
          if (mounted) {
            showAutoRunStopPendingInfo();
          }
        },
      );
      if (!registerActiveListener(unlistenAutoRunStopPending)) {
        return false;
      }

      const unlistenAutoRunStopped = await events.autoRunStopped.listen(() => {
        if (mounted) {
          showAutoRunStoppedInfo();
        }
      });
      if (!registerActiveListener(unlistenAutoRunStopped)) {
        return false;
      }

      const unlistenActiveWindowChanged =
        await events.activeWindowChanged.listen((event) => {
          if (mounted) {
            observeActiveWindowChanged(event.payload);
          }
        });
      if (!registerActiveListener(unlistenActiveWindowChanged)) {
        return false;
      }

      const unlistenSettingsChanged = await events.settingsChanged.listen(
        (event) => {
          if (mounted) {
            observeSettingsChanged(event.payload);
          }
        },
      );
      if (!registerActiveListener(unlistenSettingsChanged)) {
        return false;
      }

      const unlistenBootOfferClosed =
        await events.bootAutoStartOfferClosed.listen(() => {
          if (mounted) {
            clearBootAutoStartOffer();
          }
        });
      if (!registerActiveListener(unlistenBootOfferClosed)) return false;
      const unlistenEffects = await events.timerEffectsChanged.listen(
        (event) => {
          if (mounted) observeEffects(event.payload);
        },
      );
      return registerActiveListener(unlistenEffects);
    }

    async function setup(retryAttempt = 0): Promise<void> {
      setConnection({ status: "connecting", attempt: retryAttempt });
      const listenerStartIndex = listeners.length;
      function reportConnectionFailure(failure: TimerSyncFailure): void {
        logger.error(
          FRONTEND_LOG_EVENT_IDS.timerSyncSetupFailed,
          failure.status === "rejected" ? failure.error : failure.cause,
          {
            retry_attempt: retryAttempt,
            max_connection_retries: TIMER_SYNC_MAX_CONNECTION_RETRIES,
            failure_message: formatTimerSyncConnectionErrorDetail(failure),
          },
        );
        clearListenersFrom(listenerStartIndex);
        if (!mounted) return;
        const canRetry =
          failure.status !== "rejected" ||
          classifyCommandFailure(failure).canRetry;
        if (canRetry && shouldRetryTimerSyncConnection(retryAttempt)) {
          setConnection({
            status: "retry_wait",
            attempt: retryAttempt,
            failure,
          });
          clearRetryTimeout();
          retryTimeoutId = setTimeout(() => {
            retryTimeoutId = null;
            if (mounted) void setup(retryAttempt + 1);
          }, timerSyncRetryDelayMs(retryAttempt));
        } else {
          setConnection({ status: "failed", failure });
        }
      }
      try {
        if (!(await registerBackendListeners())) return;
        const response = await receiveCommandResponse(commands.getFullState());
        if (!mounted) return;
        if (response.status !== "accepted") {
          reportConnectionFailure(response);
          return;
        }
        observeFullState(response.value);
        setConnection({ status: "ready" });
      } catch (cause) {
        reportConnectionFailure({ status: "setup", cause });
      }
    }

    void setup();

    return (): void => {
      mounted = false;
      clearRetryTimeout();
      clearListenersFrom(0);
      setConnection({ status: "idle" });
    };
  }, [
    observeTimerTick,
    observeSessionCompleted,
    observeFullState,
    clearBootAutoStartOffer,
    observeEffects,
    observeActiveWindowChanged,
    observeSettingsChanged,
    setConnection,
    generation,
    showAutoRunStopPendingInfo,
    showAutoRunStoppedInfo,
  ]);
}
