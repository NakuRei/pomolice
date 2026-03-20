import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore, VALIDATION_RULES } from "./settingsStore";
import { useErrorStore } from "./errorStore";
import { classifyBackendError } from "../errors/classifyBackendError";
import type { WeekdayKey } from "../types/weeklyAutoRun";
import type {
  PostBreakDecisionAction,
  PostBreakTransitionPolicy,
  SessionTransitionOutcome,
} from "../types/sessionTransitionPolicy";

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
export interface SessionCompletedPayload extends SessionTransitionOutcome {
  // Temporary compatibility alias; mirrors requires_manual_continue_decision.
  post_break_decision_pending: boolean;
}

/**
 * Full state payload from backend get_full_state command
 */
export interface FullStatePayload extends TimerTickPayload {
  post_break_decision_pending: boolean;
  settings: {
    work_duration_secs: number;
    short_break_duration_secs: number;
    long_break_duration_secs: number;
    long_break_interval: number;
    post_break_transition_policy: PostBreakTransitionPolicy;
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
  requiresManualContinueDecision: boolean;
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
  resolvePostBreakDecision: (action: PostBreakDecisionAction) => Promise<void>;
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

type TimerDurationSettingsCommandPayload = Record<string, unknown> & {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
};

type SyncAutomationPoliciesCommandPayload = Record<string, unknown> & {
  postBreakTransitionPolicy: PostBreakTransitionPolicy;
  autoRunFeatureEnabled: boolean;
  weeklyAutoRunConfig: ReturnType<
    typeof useSettingsStore.getState
  >["weeklyAutoRunConfig"];
};

function clampWithinRange(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

function normalizeDurationMinutesToWholeSeconds(
  durationMinutes: number,
  minMinutes: number,
  maxMinutes: number,
): number {
  const clampedMinutes = clampWithinRange(
    durationMinutes,
    minMinutes,
    maxMinutes,
  );
  return Math.max(1, Math.round(clampedMinutes * 60));
}

function normalizeLongBreakInterval(intervalCount: number): number {
  const clampedInterval = clampWithinRange(
    intervalCount,
    VALIDATION_RULES.longBreakInterval.min,
    VALIDATION_RULES.longBreakInterval.max,
  );

  return Math.round(clampedInterval);
}

function createTimerDurationSettingsCommandPayload(
  settings: ReturnType<typeof useSettingsStore.getState>,
): TimerDurationSettingsCommandPayload {
  return {
    workDurationSecs: normalizeDurationMinutesToWholeSeconds(
      settings.workDuration,
      VALIDATION_RULES.workDuration.min,
      VALIDATION_RULES.workDuration.max,
    ),
    shortBreakDurationSecs: normalizeDurationMinutesToWholeSeconds(
      settings.shortBreakDuration,
      VALIDATION_RULES.shortBreakDuration.min,
      VALIDATION_RULES.shortBreakDuration.max,
    ),
    longBreakDurationSecs: normalizeDurationMinutesToWholeSeconds(
      settings.longBreakDuration,
      VALIDATION_RULES.longBreakDuration.min,
      VALIDATION_RULES.longBreakDuration.max,
    ),
    longBreakInterval: normalizeLongBreakInterval(settings.longBreakInterval),
  };
}

function createSyncAutomationPoliciesCommandPayload(
  settings: ReturnType<typeof useSettingsStore.getState>,
): SyncAutomationPoliciesCommandPayload {
  return {
    postBreakTransitionPolicy: settings.post_break_transition_policy,
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
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>,
): boolean {
  const lastShownAtEpochMs = infoMessageShownAtById[infoMessage.id];
  return (
    lastShownAtEpochMs !== undefined &&
    nowEpochMs - lastShownAtEpochMs < infoMessage.ttlMs
  );
}

function keepLegacyTimerActionAsNoop(): void {
  return undefined;
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
  requiresManualContinueDecision: false,
  isConnected: false,
  shouldPromptStartOnBoot: false,
  activeWindowSummary: null,
  infoMessage: null,
  infoMessageShownAtById: {},
};

/**
 * Helper to conditionally apply devtools middleware only in development
 */
const noopDevtools = (<T>(config: T): T => config) as typeof devtools;
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
      start: async (): Promise<void> => {
        const settings = useSettingsStore.getState();
        try {
          await invoke("start_work", {
            ...createTimerDurationSettingsCommandPayload(settings),
            postBreakTransitionPolicy: settings.post_break_transition_policy,
          });
          useErrorStore.getState().clearError("start_work");
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(
            detail,
            "start_work",
          );
          useErrorStore.getState().reportError({
            id: "start_work",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: false,
            retryFn: null,
          });
          throw error;
        }
      },

      // Resolve manual decision after break (continue or stop cycle).
      resolvePostBreakDecision: async (
        action: PostBreakDecisionAction,
      ): Promise<void> => {
        try {
          await invoke("resolve_post_break_decision", { action });
          set((state) => {
            state.requiresManualContinueDecision = false;
          });
          useErrorStore.getState().clearError("resolve_post_break_decision");
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(
            detail,
            "resolve_post_break_decision",
          );
          useErrorStore.getState().reportError({
            id: "resolve_post_break_decision",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry: true,
            retryFn: async () => {
              await get().resolvePostBreakDecision(action);
            },
          });
          throw error;
        }
      },

      // Emergency stop
      emergencyStop: async (): Promise<void> => {
        try {
          await invoke("emergency_stop");
          useErrorStore.getState().clearError("emergency_stop");
        } catch (error) {
          const detail = String(error);
          const { userMessage } = classifyBackendError(
            detail,
            "emergency_stop",
          );
          useErrorStore.getState().reportError({
            id: "emergency_stop",
            message: userMessage,
            technicalDetail: detail,
            severity: "critical",
            origin: "backend_command",
            canRetry: true,
            retryFn: null,
          });
          throw error;
        }
      },

      // Sync settings to backend
      syncSettings: async (): Promise<void> => {
        const settings = useSettingsStore.getState();
        const timerStatus = get().status;
        try {
          const automationPoliciesPayload =
            createSyncAutomationPoliciesCommandPayload(settings);
          if (timerStatus === TimerStatus.IDLE) {
            await invoke(
              "sync_timer_duration_settings",
              createTimerDurationSettingsCommandPayload(settings),
            );
          }
          await invoke("sync_automation_policies", automationPoliciesPayload);
        } catch {
          // Silently fail - settings sync is not critical
        }
      },

      resolveBootAutoStartPrompt: async (startNow: boolean): Promise<void> => {
        try {
          await invoke<BootPromptRuntimeStateValue>(
            "resolve_boot_auto_start_prompt",
            { startNow },
          );
          set((state) => {
            state.shouldPromptStartOnBoot = false;
          });
        } catch (error) {
          const detail = String(error);
          const { userMessage, severity } = classifyBackendError(
            detail,
            "resolve_boot_auto_start_prompt",
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

      showAutoRunStopPendingInfo: (): void => {
        set((state) => {
          const nowEpochMs = Date.now();
          if (
            shouldSuppressInfoMessageByTtl(
              nowEpochMs,
              AUTO_RUN_STOP_PENDING_INFO_MESSAGE,
              state.infoMessageShownAtById,
            )
          ) {
            return;
          }
          state.infoMessage = AUTO_RUN_STOP_PENDING_INFO_MESSAGE;
          state.infoMessageShownAtById.auto_run_stop_pending = nowEpochMs;
        });
      },

      showAutoRunStoppedInfo: (): void => {
        set((state) => {
          const nowEpochMs = Date.now();
          if (
            shouldSuppressInfoMessageByTtl(
              nowEpochMs,
              AUTO_RUN_STOPPED_INFO_MESSAGE,
              state.infoMessageShownAtById,
            )
          ) {
            return;
          }
          state.infoMessage = AUTO_RUN_STOPPED_INFO_MESSAGE;
          state.infoMessageShownAtById.auto_run_stopped = nowEpochMs;
        });
      },

      clearInfoMessage: (expectedInfoMessageId?: InfoMessageId): void => {
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
        });
      },

      // Handle timer tick event from backend
      handleTimerTick: (payload: TimerTickPayload): void => {
        set((state) => {
          state.status = payload.status;
          state.remainingSeconds = payload.remaining_seconds;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isFullscreenBreak = payload.is_fullscreen_break;
        });
      },

      // Handle session completed event from backend
      handleSessionCompleted: (payload: SessionCompletedPayload): void => {
        set((state) => {
          state.requiresManualContinueDecision =
            payload.requires_manual_continue_decision;
          // Session type will be updated via timer_tick event
        });
      },

      // Handle full state from backend (initial sync)
      handleFullState: (payload: FullStatePayload): void => {
        set((state) => {
          state.status = payload.status;
          state.remainingSeconds = payload.remaining_seconds;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isFullscreenBreak = payload.is_fullscreen_break;
          state.requiresManualContinueDecision =
            payload.post_break_decision_pending;
          state.shouldPromptStartOnBoot = payload.should_prompt_start_on_boot;
          state.activeWindowSummary = payload.active_window_summary;
          state.isConnected = true;
        });
      },

      // Set connection status
      setConnected: (connected: boolean): void => {
        set((state) => {
          state.isConnected = connected;
        });
      },

      // Legacy compatibility - these do nothing now
      stop: keepLegacyTimerActionAsNoop,
      reset: keepLegacyTimerActionAsNoop,
      tick: keepLegacyTimerActionAsNoop,
    })),
    { name: "TimerStore" },
  ),
);

// Sync settings to backend when they change (while idle)
useSettingsStore.subscribe(() => {
  const timerState = useTimerStore.getState();
  if (timerState.isConnected) {
    void timerState.syncSettings();
  }
});
