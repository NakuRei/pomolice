import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type {
  TimerSnapshot,
  TimerEffectsState,
  SettingsFileStatus,
  TimerStatus,
  SessionType,
  BootAutoStartOfferPayload,
  ActiveWindowSummaryPayload,
} from "@/ipc/generated";
import { settingsFromPayload, type Settings } from "@/backend/settings";
import type { BreakLockObservation } from "./effects";

interface BackendObservations {
  revision: TimerSnapshot["revision"] | null;
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
  // Undefined until observed.
  bootAutoStartOffer: BootAutoStartOfferPayload | null | undefined;
  activeWindowSummary: ActiveWindowSummaryPayload | null | undefined;

  // Settings owned by the backend; null until the first full state arrives.
  settings: Settings | null;
  settingsFileStatus: SettingsFileStatus | null;
  effects: TimerEffectsState | null;
}

interface ObservationActions {
  observeSnapshot: (snapshot: TimerSnapshot) => void;
}
export const useBackendObservationStore = create<
  BackendObservations & ObservationActions
>()(
  immer((set) => ({
    revision: null,
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
    observeSnapshot: (snapshot: TimerSnapshot): void => {
      const payload = snapshot.timer;
      set((state) => {
        if (
          state.revision !== null &&
          BigInt(snapshot.revision) <= BigInt(state.revision)
        )
          return;
        state.revision = snapshot.revision;
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
        state.bootAutoStartOffer = payload.boot_auto_start_offer;
        state.activeWindowSummary = payload.active_window_summary;
        state.effects = snapshot.effects;
      });
    },
  })),
);
export function selectBreakLockObservation(
  state: Pick<BackendObservations, "effects">,
): BreakLockObservation {
  const attempt = state.effects?.break_lock.last_attempt;
  if (!attempt || attempt.status === "failed") return "unknown";
  return attempt.action === "enter" ? "locked" : "unlocked";
}
