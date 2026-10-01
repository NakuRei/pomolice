import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type {
  TimerSnapshot,
  TimerEffectsState,
  SettingsFileStatus,
  TimerStatus,
  SessionType,
  TimerTickPayload,
  SessionCompletedPayload,
  BootAutoStartOfferPayload,
  ActiveWindowSummaryPayload,
  ActiveWindowChangedPayload,
  SettingsChangedPayload,
} from "@/ipc/generated";
import { settingsFromPayload, type Settings } from "@/types/settings";
import type { BreakLockObservation } from "@/types/timerEffects";

interface TimerObservations {
  status: TimerStatus | null;
  remainingSeconds: number | null;
  /** Total length of the current session; it keeps the length the session
   * started with even after settings are saved. */
  sessionDurationSeconds: number | null;
  sessionType: SessionType | null;
  currentCycle: number | null;
  completedPomodoros: number | null;
  breakInteractionCount: number | null;
  isAwayDuringBreak: boolean | null;

  requiresManualContinueDecision: boolean | null;
  // Undefined until observed; null is final for this backend run.
  bootAutoStartOffer: BootAutoStartOfferPayload | null | undefined;
  activeWindowSummary: ActiveWindowSummaryPayload | null | undefined;

  // Settings owned by the backend; null until the first full state arrives.
  settings: Settings | null;
  settingsFileStatus: SettingsFileStatus | null;
  effects: TimerEffectsState | null;
}

interface ObservationActions {
  observeTimerTick: (payload: TimerTickPayload) => void;
  observeSessionCompleted: (payload: SessionCompletedPayload) => void;
  observeFullState: (snapshot: TimerSnapshot) => void;
  observeActiveWindowChanged: (payload: ActiveWindowChangedPayload) => void;
  observeSettingsChanged: (payload: SettingsChangedPayload) => void;
  observeEffects: (effects: TimerEffectsState) => void;
  clearBootAutoStartOffer: () => void;
  closePostBreakDecision: () => void;
}
export const useTimerObservationStore = create<
  TimerObservations & ObservationActions
>()(
  immer((set) => ({
    status: null,
    remainingSeconds: null,
    sessionDurationSeconds: null,
    sessionType: null,
    currentCycle: null,
    completedPomodoros: null,
    breakInteractionCount: null,
    isAwayDuringBreak: null,
    requiresManualContinueDecision: null,
    bootAutoStartOffer: undefined,
    activeWindowSummary: undefined,
    settings: null,
    settingsFileStatus: null,
    effects: null,
    observeEffects: (effects): void => {
      set((state) => {
        if (
          state.effects === null ||
          BigInt(effects.revision) > BigInt(state.effects.revision)
        )
          state.effects = effects;
      });
    },
    closePostBreakDecision: (): void => {
      set({ requiresManualContinueDecision: false });
    },
    observeTimerTick: (payload: TimerTickPayload): void => {
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

    observeSessionCompleted: (payload: SessionCompletedPayload): void => {
      set((state) => {
        state.requiresManualContinueDecision =
          payload.requires_manual_continue_decision;
        // Session type will be updated via timer_tick event
      });
    },

    observeFullState: (snapshot: TimerSnapshot): void => {
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
        if (
          state.effects === null ||
          BigInt(snapshot.effects.revision) > BigInt(state.effects.revision)
        )
          state.effects = snapshot.effects;
      });
    },

    clearBootAutoStartOffer: (): void => {
      set((state) => {
        state.bootAutoStartOffer = null;
      });
    },

    observeActiveWindowChanged: (payload: ActiveWindowChangedPayload): void => {
      set((state) => {
        state.activeWindowSummary = payload.active_window_summary;
      });
    },

    observeSettingsChanged: (payload: SettingsChangedPayload): void => {
      set((state) => {
        state.settings = settingsFromPayload(payload.settings);
        state.settingsFileStatus = payload.settings_file_status;
      });
    },
  })),
);
export function selectBreakLockObservation(
  state: Pick<TimerObservations, "effects">,
): BreakLockObservation {
  const attempt = state.effects?.break_lock.last_attempt;
  if (!attempt || attempt.status === "failed") return "unknown";
  return attempt.action === "enter" ? "locked" : "unlocked";
}
