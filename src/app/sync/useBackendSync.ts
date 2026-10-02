import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
/**
 * Timer Sync Hook
 * Sets up event listeners to receive timer state from backend
 * The backend is the single source of truth for all timer state
 */

import { useEffect } from "react";
import {
  useBackendSyncStore,
  type BackendSyncFailure,
} from "@/backend/syncStore";
import { useTimerNoticeStore } from "@/features/timer/noticeStore";

import { type UnlistenFn } from "@tauri-apps/api/event";
import { commands, events } from "@/ipc/generated";
import { createLogger } from "../../logging/logger";
import { FRONTEND_LOG_EVENT_IDS } from "../../logging/logEvents";
import { useBackendObservationStore } from "../../backend/observationStore";

import { notifySessionCompleted } from "@/features/timer/sessionNotifications";
import {
  shouldRetryTimerSyncConnection,
  TIMER_SYNC_MAX_CONNECTION_RETRIES,
  timerSyncRetryDelayMs,
} from "./connectionRetryPolicy";
import { formatTimerSyncConnectionErrorDetail } from "../../backend/connectionError";
import { classifyCommandFailure } from "../../errors/classifyBackendError";

const logger = createLogger("timer_sync");

/**
 * Hook to sync timer state from backend.
 * Call this once in your app root component.
 *
 */
export function useBackendSync(): void {
  const generation = useBackendSyncStore((state) => state.generation);
  const setConnection = useBackendSyncStore((state) => state.setConnection);
  const observeSnapshot = useBackendObservationStore(
    (state) => state.observeSnapshot,
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
      const unlistenSnapshot = await events.timerSnapshotChanged.listen(
        (event) => {
          if (mounted) observeSnapshot(event.payload);
        },
      );
      if (!registerActiveListener(unlistenSnapshot)) return false;

      const unlistenSessionCompleted = await events.sessionCompleted.listen(
        (event) => {
          if (mounted) {
            void notifySessionCompleted(event.payload);
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

      return true;
    }

    async function setup(retryAttempt = 0): Promise<void> {
      setConnection({ status: "connecting", attempt: retryAttempt });
      const listenerStartIndex = listeners.length;
      function reportConnectionFailure(failure: BackendSyncFailure): void {
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
        observeSnapshot(response.value);
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
    observeSnapshot,
    setConnection,
    generation,
    showAutoRunStopPendingInfo,
    showAutoRunStoppedInfo,
  ]);
}
