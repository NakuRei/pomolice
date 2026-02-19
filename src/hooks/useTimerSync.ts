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
import { notifyWorkComplete, notifyBreakComplete, notifyLongBreakComplete } from "../services/notifications";
import { SessionType } from "../store/timerStore";

/**
 * Hook to sync timer state from backend
 * Call this once in your app root component
 */
export function useTimerSync() {
  const handleTimerTick = useTimerStore((s) => s.handleTimerTick);
  const handleSessionCompleted = useTimerStore((s) => s.handleSessionCompleted);
  const handleFullState = useTimerStore((s) => s.handleFullState);
  const setConnected = useTimerStore((s) => s.setConnected);
  const syncSettings = useTimerStore((s) => s.syncSettings);

  useEffect(() => {
    const listeners: UnlistenFn[] = [];
    let mounted = true;

    const setup = async () => {
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

              // Show notification based on completed session type
              const { completed_session_type } = event.payload;
              if (completed_session_type === SessionType.WORK) {
                notifyWorkComplete();
              } else if (completed_session_type === SessionType.SHORT_BREAK) {
                notifyBreakComplete();
              } else if (completed_session_type === SessionType.LONG_BREAK) {
                notifyLongBreakComplete();
              }
            }
          }
        );
        listeners.push(unlistenSessionCompleted);

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
        }
      } catch (error) {
        console.error("Failed to setup timer sync:", error);
        if (mounted) {
          setConnected(false);
        }
      }
    };

    setup();

    return () => {
      mounted = false;
      listeners.forEach((unlisten) => unlisten());
    };
  }, [handleTimerTick, handleSessionCompleted, handleFullState, setConnected, syncSettings]);
}
