import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "./settingsStore";
import { useErrorStore } from "./errorStore";
import { classifyBackendError } from "../errors/classifyBackendError";

/**
 * Timer status constants matching backend TimerStatus enum
 */
export const TimerStatus = {
  IDLE: "idle",
  WORKING: "working",
  ON_BREAK: "on_break",
} as const;

export type TimerStatusType = (typeof TimerStatus)[keyof typeof TimerStatus];

/**
 * Session type constants matching backend SessionType enum
 */
export const SessionType = {
  WORK: "work",
  SHORT_BREAK: "short_break",
  LONG_BREAK: "long_break",
} as const;

export type SessionTypeValue = (typeof SessionType)[keyof typeof SessionType];

/**
 * Payload received from backend timer_tick events
 */
export interface TimerTickPayload {
  status: TimerStatusType;
  remaining_seconds: number;
  session_type: SessionTypeValue;
  current_cycle: number;
  completed_pomodoros: number;
  break_interaction_count: number;
  is_fullscreen_break: boolean;
}

/**
 * Payload received from backend session_completed events
 */
export interface SessionCompletedPayload {
  completed_session_type: SessionTypeValue;
  next_session_type: SessionTypeValue | null;
  show_continue_dialog: boolean;
}

/**
 * Full state payload from backend get_full_state command
 */
export interface FullStatePayload extends TimerTickPayload {
  show_continue_dialog: boolean;
  settings: {
    work_duration_secs: number;
    short_break_duration_secs: number;
    long_break_duration_secs: number;
    long_break_interval: number;
  };
}

/**
 * Timer state interface - display-only state from backend
 * The backend is the single source of truth
 */
interface TimerState {
  // Display state (received from backend events)
  status: TimerStatusType;
  remainingSeconds: number;
  sessionType: SessionTypeValue;
  currentCycle: number;
  completedPomodoros: number;
  breakInteractionCount: number;
  isFullscreenBreak: boolean;

  // UI-only state
  showContinueDialog: boolean;
  isConnected: boolean;
}

/**
 * Timer actions interface
 * Actions send commands to backend or handle events from backend
 */
interface TimerActions {
  // User actions (send commands to backend)
  start: () => Promise<void>;
  startNextSession: () => Promise<void>;
  endCycle: () => Promise<void>;
  emergencyStop: () => Promise<void>;
  syncSettings: () => Promise<void>;

  // Event handlers (called by event listeners from useTimerSync)
  handleTimerTick: (payload: TimerTickPayload) => void;
  handleSessionCompleted: (payload: SessionCompletedPayload) => void;
  handleFullState: (payload: FullStatePayload) => void;
  setConnected: (connected: boolean) => void;

  // Legacy compatibility (to be removed)
  stop: () => void;
  reset: () => void;
  tick: () => void;
}

type TimerStore = TimerState & TimerActions;

/**
 * Initial state for the timer
 */
const initialState: TimerState = {
  status: TimerStatus.IDLE,
  remainingSeconds: 25 * 60, // Will be updated from backend
  sessionType: SessionType.WORK,
  currentCycle: 0,
  completedPomodoros: 0,
  breakInteractionCount: 0,
  isFullscreenBreak: false,
  showContinueDialog: false,
  isConnected: false,
};

/**
 * Helper to conditionally apply devtools middleware only in development
 */
const noopDevtools = (<T,>(config: T): T => config) as typeof devtools;
const conditionalDevtools = import.meta.env.DEV ? devtools : noopDevtools;

/**
 * Zustand store for timer state management
 * This store is display-only - all state comes from backend events
 */
export const useTimerStore = create<TimerStore>()(
  conditionalDevtools(
    immer((set) => ({
      ...initialState,

      // Start work session - sends command to backend with current settings
      start: async () => {
        const settings = useSettingsStore.getState();
        try {
          await invoke("start_work", {
            workDurationSecs: settings.workDuration * 60,
            shortBreakDurationSecs: settings.shortBreakDuration * 60,
            longBreakDurationSecs: settings.longBreakDuration * 60,
            longBreakInterval: settings.longBreakInterval,
          });
          useErrorStore.getState().clearError("start_work");
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(detail, "start_work");
          useErrorStore.getState().reportError({
            id: "start_work",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: false,
            retryFn: null,
          });
          console.error("Failed to start work:", error);
          throw error;
        }
      },

      // Continue to next work session after break
      startNextSession: async () => {
        try {
          await invoke("continue_to_next_session");
          useErrorStore.getState().clearError("continue_to_next_session");
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(detail, "continue_to_next_session");
          useErrorStore.getState().reportError({
            id: "continue_to_next_session",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: true,
            retryFn: null,
          });
          console.error("Failed to continue to next session:", error);
          throw error;
        }
      },

      // End current cycle
      endCycle: async () => {
        try {
          await invoke("end_cycle");
          useErrorStore.getState().clearError("end_cycle");
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(detail, "end_cycle");
          useErrorStore.getState().reportError({
            id: "end_cycle",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: true,
            retryFn: null,
          });
          console.error("Failed to end cycle:", error);
          throw error;
        }
      },

      // Emergency stop
      emergencyStop: async () => {
        try {
          await invoke("emergency_stop");
          useErrorStore.getState().clearError("emergency_stop");
        } catch (error) {
          const detail = String(error);
          const { userMessage } = classifyBackendError(detail, "emergency_stop");
          useErrorStore.getState().reportError({
            id: "emergency_stop",
            message: userMessage,
            technicalDetail: detail,
            severity: "critical",
            origin: "backend_command",
            canRetry: true,
            retryFn: null,
          });
          console.error("Failed to emergency stop:", error);
          throw error;
        }
      },

      // Sync settings to backend
      syncSettings: async () => {
        const settings = useSettingsStore.getState();
        try {
          await invoke("sync_settings", {
            workDurationSecs: settings.workDuration * 60,
            shortBreakDurationSecs: settings.shortBreakDuration * 60,
            longBreakDurationSecs: settings.longBreakDuration * 60,
            longBreakInterval: settings.longBreakInterval,
          });
        } catch (error) {
          // Silently fail - settings sync is not critical
          console.debug("Settings sync failed (timer may be running):", error);
        }
      },

      // Handle timer tick event from backend
      handleTimerTick: (payload: TimerTickPayload) =>
        set((state) => {
          state.status = payload.status;
          state.remainingSeconds = payload.remaining_seconds;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isFullscreenBreak = payload.is_fullscreen_break;
        }),

      // Handle session completed event from backend
      handleSessionCompleted: (payload: SessionCompletedPayload) =>
        set((state) => {
          state.showContinueDialog = payload.show_continue_dialog;
          // Session type will be updated via timer_tick event
        }),

      // Handle full state from backend (initial sync)
      handleFullState: (payload: FullStatePayload) =>
        set((state) => {
          state.status = payload.status;
          state.remainingSeconds = payload.remaining_seconds;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isFullscreenBreak = payload.is_fullscreen_break;
          state.showContinueDialog = payload.show_continue_dialog;
          state.isConnected = true;
        }),

      // Set connection status
      setConnected: (connected: boolean) =>
        set((state) => {
          state.isConnected = connected;
        }),

      // Legacy compatibility - these do nothing now
      stop: () => {},
      reset: () => {},
      tick: () => {},
    })),
    { name: "TimerStore" }
  )
);

// Sync settings to backend when they change (while idle)
useSettingsStore.subscribe(() => {
  const timerState = useTimerStore.getState();
  if (timerState.status === TimerStatus.IDLE && timerState.isConnected) {
    timerState.syncSettings();
  }
});
