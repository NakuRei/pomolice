import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "./settingsStore";
import { useErrorStore } from "./errorStore";
import { classifyBackendError } from "../errors/classifyBackendError";
import type { WeekdayKey } from "../types/weeklyAutoRun";

/**
 * Timer status constants matching backend TimerStatus enum
 */
export const TimerStatus = {
  IDLE: "idle",
  WORKING: "working",
  ON_BREAK: "on_break",
} as const;

export type TimerStatus = (typeof TimerStatus)[keyof typeof TimerStatus];

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
  status: TimerStatus;
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
  should_prompt_start_on_boot: boolean;
  active_window_summary: ActiveWindowSummaryPayload | null;
}

export interface ActiveWindowSummaryPayload {
  weekday_key: WeekdayKey;
  start_time: string;
  end_time: string;
  crosses_midnight: boolean;
}

export type InfoMessageId = "auto_run_stop_pending" | "auto_run_stopped";

export interface InfoMessage {
  id: InfoMessageId;
  message: string;
  ttlMs: number;
}

type BootPromptRuntimeStateValue =
  | "awaiting_decision"
  | "resolved_start"
  | "resolved_later"
  | "not_applicable";

/**
 * Timer state interface - display-only state from backend
 * The backend is the single source of truth
 */
interface TimerState {
  // Display state (received from backend events)
  status: TimerStatus;
  remainingSeconds: number;
  sessionType: SessionTypeValue;
  currentCycle: number;
  completedPomodoros: number;
  breakInteractionCount: number;
  isFullscreenBreak: boolean;

  // UI-only state
  showContinueDialog: boolean;
  isConnected: boolean;
  shouldPromptStartOnBoot: boolean;
  activeWindowSummary: ActiveWindowSummaryPayload | null;
  infoMessage: InfoMessage | null;
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>;
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
  resolveBootAutoStartPrompt: (startNow: boolean) => Promise<void>;
  showAutoRunStopPendingInfo: () => void;
  showAutoRunStoppedInfo: () => void;
  clearInfoMessage: (expectedInfoMessageId?: InfoMessageId) => void;

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

type SyncAllSettingsCommandPayload = Record<string, unknown> & {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
  autoRunFeatureEnabled: boolean;
  weeklyAutoRunConfig: ReturnType<typeof useSettingsStore.getState>["weeklyAutoRunConfig"];
};

type SyncAutoRunSettingsCommandPayload = Record<string, unknown> & {
  autoRunFeatureEnabled: boolean;
  weeklyAutoRunConfig: ReturnType<typeof useSettingsStore.getState>["weeklyAutoRunConfig"];
};

function createSyncAllSettingsCommandPayload(
  settings: ReturnType<typeof useSettingsStore.getState>
): SyncAllSettingsCommandPayload {
  return {
    workDurationSecs: settings.workDuration * 60,
    shortBreakDurationSecs: settings.shortBreakDuration * 60,
    longBreakDurationSecs: settings.longBreakDuration * 60,
    longBreakInterval: settings.longBreakInterval,
    autoRunFeatureEnabled: settings.autoRunFeatureEnabled,
    weeklyAutoRunConfig: settings.weeklyAutoRunConfig,
  };
}

function createSyncAutoRunSettingsCommandPayload(
  settings: ReturnType<typeof useSettingsStore.getState>
): SyncAutoRunSettingsCommandPayload {
  return {
    autoRunFeatureEnabled: settings.autoRunFeatureEnabled,
    weeklyAutoRunConfig: settings.weeklyAutoRunConfig,
  };
}

const AUTO_RUN_STOP_PENDING_INFO_MESSAGE: InfoMessage = {
  id: "auto_run_stop_pending",
  message: "現在セッション終了後に停止予定",
  ttlMs: 5000,
};

const AUTO_RUN_STOPPED_INFO_MESSAGE: InfoMessage = {
  id: "auto_run_stopped",
  message: "自動停止しました",
  ttlMs: 3500,
};

function shouldSuppressInfoMessageByTtl(
  nowEpochMs: number,
  infoMessage: InfoMessage,
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>
): boolean {
  const lastShownAtEpochMs = infoMessageShownAtById[infoMessage.id];
  return (
    lastShownAtEpochMs !== undefined &&
    nowEpochMs - lastShownAtEpochMs < infoMessage.ttlMs
  );
}

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
  shouldPromptStartOnBoot: false,
  activeWindowSummary: null,
  infoMessage: null,
  infoMessageShownAtById: {},
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
    immer((set, get) => ({
      ...initialState,

      // Start work session.
      // Backend decides manual_inside_window/manual_outside_window from server-side clock and schedule.
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
          set((state) => { state.showContinueDialog = false; });
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
          set((state) => { state.showContinueDialog = false; });
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
        const timerStatus = get().status;
        try {
          if (timerStatus === TimerStatus.IDLE) {
            await invoke(
              "sync_all_settings",
              createSyncAllSettingsCommandPayload(settings)
            );
          } else {
            await invoke(
              "sync_auto_run_settings",
              createSyncAutoRunSettingsCommandPayload(settings)
            );
          }
        } catch (error) {
          // Silently fail - settings sync is not critical
          console.debug("Settings sync failed (timer may be running):", error);
        }
      },

      resolveBootAutoStartPrompt: async (startNow: boolean) => {
        try {
          await invoke<BootPromptRuntimeStateValue>(
            "resolve_boot_auto_start_prompt",
            { startNow }
          );
          set((state) => {
            state.shouldPromptStartOnBoot = false;
          });
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(
            detail,
            "resolve_boot_auto_start_prompt"
          );
          useErrorStore.getState().reportError({
            id: "resolve_boot_auto_start_prompt",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: true,
            retryFn: null,
          });
          throw error;
        }
      },

      showAutoRunStopPendingInfo: () =>
        set((state) => {
          const nowEpochMs = Date.now();
          if (
            shouldSuppressInfoMessageByTtl(
              nowEpochMs,
              AUTO_RUN_STOP_PENDING_INFO_MESSAGE,
              state.infoMessageShownAtById
            )
          ) {
            return;
          }
          state.infoMessage = AUTO_RUN_STOP_PENDING_INFO_MESSAGE;
          state.infoMessageShownAtById.auto_run_stop_pending = nowEpochMs;
        }),

      showAutoRunStoppedInfo: () =>
        set((state) => {
          const nowEpochMs = Date.now();
          if (
            shouldSuppressInfoMessageByTtl(
              nowEpochMs,
              AUTO_RUN_STOPPED_INFO_MESSAGE,
              state.infoMessageShownAtById
            )
          ) {
            return;
          }
          state.infoMessage = AUTO_RUN_STOPPED_INFO_MESSAGE;
          state.infoMessageShownAtById.auto_run_stopped = nowEpochMs;
        }),

      clearInfoMessage: (expectedInfoMessageId?: InfoMessageId) =>
        set((state) => {
          if (!state.infoMessage) {
            return;
          }
          if (
            expectedInfoMessageId &&
            state.infoMessage.id !== expectedInfoMessageId
          ) {
            return;
          }
          state.infoMessage = null;
        }),

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
          state.shouldPromptStartOnBoot = payload.should_prompt_start_on_boot;
          state.activeWindowSummary = payload.active_window_summary;
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
  if (timerState.isConnected) {
    timerState.syncSettings();
  }
});
