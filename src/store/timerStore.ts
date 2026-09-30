import {
  type TimerSnapshot,
  type SettingsFileStatus,
  type SettingsValidationIssue,
  type PostBreakDecisionAction,
  type PostBreakTransitionPolicy,
  type TimerStatus,
  type SessionType,
  type TimerTickPayload,
  type SessionCompletedPayload,
  type BootAutoStartOfferPayload,
  type ActiveWindowSummaryPayload,
  type ActiveWindowChangedPayload,
  type SettingsChangedPayload,
  commands,
} from "@/ipc/generated";
import { create } from "zustand";
import { devtools } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import { useErrorStore } from "./errorStore";
import { useTimerEffectsStore } from "./timerEffectsStore";

import {
  classifyBackendError,
  formatPublicCommandErrorDetail,
  settingsValidationIssuesOf,
} from "../errors/classifyBackendError";
import {
  settingsChangesToPayload,
  settingsFromPayload,
  type Settings,
  type SettingsChanges,
} from "../types/settings";

export type InfoMessageId = "auto_run_stop_pending" | "auto_run_stopped";

export interface InfoMessage {
  id: InfoMessageId;
  message: string;
  ttlMs: number;
}

/**
 * Timer state interface - display-only state from backend
 * The backend is the single source of truth
 */
interface TimerState {
  // Display state (received from backend events)
  status: TimerStatus;
  remainingSeconds: number;
  /** Total length of the current session; it keeps the length the session
   * started with even after settings are saved. */
  sessionDurationSeconds: number;
  sessionType: SessionType;
  currentCycle: number;
  completedPomodoros: number;
  breakInteractionCount: number;
  isAwayDuringBreak: boolean;

  // UI-only state
  requiresManualContinueDecision: boolean;
  isConnected: boolean;
  // Undefined until observed; null is final for this backend run.
  bootAutoStartOffer: BootAutoStartOfferPayload | null | undefined;
  activeWindowSummary: ActiveWindowSummaryPayload | null;
  infoMessage: InfoMessage | null;
  infoMessageShownAtById: Partial<Record<InfoMessageId, number>>;

  // Settings owned by the backend; null until the first full state arrives.
  settings: Settings | null;
  settingsFileStatus: SettingsFileStatus;
}

/**
 * Outcome of saving settings. `rejected` carries every input the backend
 * refused; `failed` means the save did not happen for another reason, which
 * is reported to the error store.
 */
export type SettingsSaveResult =
  | { status: "saved" }
  | { status: "rejected"; issues: SettingsValidationIssue[] }
  | { status: "failed" };

/**
 * Timer actions interface
 * Actions send commands to backend or handle events from backend
 */
interface TimerActions {
  // User actions (send commands to backend)
  start: () => Promise<void>;
  resolvePostBreakDecision: (action: PostBreakDecisionAction) => Promise<void>;
  emergencyStop: () => Promise<void>;
  setAwayDuringBreak: (away: boolean) => Promise<void>;
  saveSettings: (changes: SettingsChanges) => Promise<SettingsSaveResult>;
  /** Saves the settings with the notification sound muted or unmuted. */
  toggleNotificationSoundMuted: () => Promise<void>;
  /** Saves the settings with another post-break transition policy. */
  savePostBreakTransitionPolicy: (
    policy: PostBreakTransitionPolicy,
  ) => Promise<void>;
  resolveBootAutoStartPrompt: (startNow: boolean) => Promise<void>;
  showAutoRunStopPendingInfo: () => void;
  showAutoRunStoppedInfo: () => void;
  clearInfoMessage: (expectedInfoMessageId?: InfoMessageId) => void;

  // Event handlers (called by event listeners from useTimerSync)
  handleTimerTick: (payload: TimerTickPayload) => void;
  handleSessionCompleted: (payload: SessionCompletedPayload) => void;
  handleFullState: (snapshot: TimerSnapshot) => void;
  handleActiveWindowChanged: (payload: ActiveWindowChangedPayload) => void;
  handleSettingsChanged: (payload: SettingsChangedPayload) => void;
  clearBootAutoStartOffer: () => void;
  setConnected: (connected: boolean) => void;
}

type TimerStore = TimerState & TimerActions;

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

/**
 * Initial state for the timer
 */
const initialState: TimerState = {
  status: "idle",
  remainingSeconds: 25 * 60, // Will be updated from backend
  sessionDurationSeconds: 25 * 60, // Will be updated from backend
  sessionType: "work",
  currentCycle: 0,
  completedPomodoros: 0,
  breakInteractionCount: 0,
  isAwayDuringBreak: false,
  requiresManualContinueDecision: false,
  isConnected: false,
  bootAutoStartOffer: undefined,
  activeWindowSummary: null,
  infoMessage: null,
  infoMessageShownAtById: {},
  settings: null,
  settingsFileStatus: "ok",
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
        try {
          const outcome = await commands.startWork();
          useTimerEffectsStore.getState().observe(outcome.effects);
          useErrorStore.getState().clearError("start_work");
        } catch (error) {
          const detail = formatPublicCommandErrorDetail(error);
          const { userMessage, severity, canRetry } =
            classifyBackendError(error);
          useErrorStore.getState().reportError({
            id: "start_work",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  await get().start();
                }
              : null,
          });
          throw error;
        }
      },

      // Resolve manual decision after break (continue or stop cycle).
      resolvePostBreakDecision: async (
        action: PostBreakDecisionAction,
      ): Promise<void> => {
        try {
          const outcome = await commands.resolvePostBreakDecision(action);
          useTimerEffectsStore.getState().observe(outcome.effects);
          set((state) => {
            state.requiresManualContinueDecision = false;
          });
          useErrorStore.getState().clearError("resolve_post_break_decision");
        } catch (error) {
          const detail = formatPublicCommandErrorDetail(error);
          const { userMessage, severity, canRetry } =
            classifyBackendError(error);
          useErrorStore.getState().reportError({
            id: "resolve_post_break_decision",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  await get().resolvePostBreakDecision(action);
                }
              : null,
          });
          throw error;
        }
      },

      // Emergency stop
      emergencyStop: async (): Promise<void> => {
        try {
          const outcome = await commands.emergencyStop();
          useTimerEffectsStore.getState().observe(outcome.effects);
          useErrorStore.getState().clearError("emergency_stop");
        } catch (error) {
          const detail = formatPublicCommandErrorDetail(error);
          const { userMessage, canRetry } = classifyBackendError(error);
          useErrorStore.getState().reportError({
            id: "emergency_stop",
            message: userMessage,
            technicalDetail: detail,
            severity: "critical",
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  await get().emergencyStop();
                }
              : null,
          });
          throw error;
        }
      },

      saveSettings: async (
        changes: SettingsChanges,
      ): Promise<SettingsSaveResult> => {
        try {
          // Read settings from ordered events so a delayed reply cannot
          // overwrite a newer settings change.
          const outcome = await commands.updateSettings(
            settingsChangesToPayload(changes),
          );
          useTimerEffectsStore.getState().observe(outcome.effects);
          useErrorStore.getState().clearError("update_settings");
          return { status: "saved" };
        } catch (error) {
          const issues = settingsValidationIssuesOf(error);
          if (issues !== null) {
            return { status: "rejected", issues };
          }

          const { userMessage, severity, canRetry } =
            classifyBackendError(error);
          useErrorStore.getState().reportError({
            id: "update_settings",
            message: userMessage,
            technicalDetail: formatPublicCommandErrorDetail(error),
            severity,
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  await get().saveSettings(changes);
                }
              : null,
          });
          return { status: "failed" };
        }
      },

      toggleNotificationSoundMuted: async (): Promise<void> => {
        const { settings } = get();
        if (settings === null) {
          return;
        }
        await get().saveSettings({
          notificationSoundMuted: !settings.notificationSoundMuted,
        });
      },

      savePostBreakTransitionPolicy: async (
        policy: PostBreakTransitionPolicy,
      ): Promise<void> => {
        await get().saveSettings({ postBreakTransitionPolicy: policy });
      },

      setAwayDuringBreak: async (away: boolean): Promise<void> => {
        try {
          const outcome = await commands.setAwayDuringBreak(away);
          useTimerEffectsStore.getState().observe(outcome.effects);
          useErrorStore.getState().clearError("set_away_during_break");
        } catch (error) {
          const detail = formatPublicCommandErrorDetail(error);
          const { userMessage, severity, canRetry } =
            classifyBackendError(error);
          useErrorStore.getState().reportError({
            id: "set_away_during_break",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  await get().setAwayDuringBreak(away);
                }
              : null,
          });
          throw error;
        }
      },

      resolveBootAutoStartPrompt: async (startNow: boolean): Promise<void> => {
        const previousError =
          useErrorStore.getState().errors.resolve_boot_auto_start_prompt;
        try {
          const outcome = await commands.resolveBootAutoStartPrompt(startNow);
          useTimerEffectsStore.getState().observe(outcome.effects);
          set((state) => {
            state.bootAutoStartOffer = null;
          });
          if (
            previousError?.canRetry === true &&
            useErrorStore.getState().errors.resolve_boot_auto_start_prompt ===
              previousError
          ) {
            useErrorStore
              .getState()
              .clearError("resolve_boot_auto_start_prompt");
          }
        } catch (error) {
          const detail = formatPublicCommandErrorDetail(error);
          const {
            userMessage,
            severity,
            canRetry: retryableError,
          } = classifyBackendError(error);
          const canRetry = retryableError && Boolean(get().bootAutoStartOffer);
          useErrorStore.getState().reportError({
            id: "resolve_boot_auto_start_prompt",
            message: userMessage,
            technicalDetail: detail,
            severity,
            origin: "backend_command",
            canRetry,
            retryFn: canRetry
              ? async (): Promise<void> => {
                  if (get().bootAutoStartOffer) {
                    await get().resolveBootAutoStartPrompt(startNow);
                  }
                }
              : null,
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
          state.sessionDurationSeconds = payload.session_duration_secs;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isAwayDuringBreak = payload.away_during_break;
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
      handleFullState: (snapshot: TimerSnapshot): void => {
        useTimerEffectsStore.getState().observe(snapshot.effects);
        const payload = snapshot.timer;
        set((state) => {
          state.status = payload.status;
          state.remainingSeconds = payload.remaining_seconds;
          state.sessionDurationSeconds = payload.session_duration_secs;
          state.sessionType = payload.session_type;
          state.currentCycle = payload.current_cycle;
          state.completedPomodoros = payload.completed_pomodoros;
          state.breakInteractionCount = payload.break_interaction_count;
          state.isAwayDuringBreak = payload.away_during_break;
          state.settings = settingsFromPayload(payload.settings);
          state.settingsFileStatus = payload.settings_file_status;
          state.requiresManualContinueDecision =
            payload.post_break_decision_pending;
          // A late snapshot cannot reopen an offer closed during this backend run.
          if (state.bootAutoStartOffer !== null) {
            state.bootAutoStartOffer = payload.boot_auto_start_offer;
          }
          state.activeWindowSummary = payload.active_window_summary;
          state.isConnected = true;
        });
      },

      clearBootAutoStartOffer: (): void => {
        set((state) => {
          state.bootAutoStartOffer = null;
        });
      },

      handleActiveWindowChanged: (
        payload: ActiveWindowChangedPayload,
      ): void => {
        set((state) => {
          state.activeWindowSummary = payload.active_window_summary;
        });
      },

      handleSettingsChanged: (payload: SettingsChangedPayload): void => {
        set((state) => {
          state.settings = settingsFromPayload(payload.settings);
          state.settingsFileStatus = payload.settings_file_status;
        });
      },

      // Set connection status
      setConnected: (connected: boolean): void => {
        set((state) => {
          state.isConnected = connected;
        });
      },
    })),
    { name: "TimerStore" },
  ),
);
