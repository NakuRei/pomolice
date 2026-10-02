import type {
  TimerEffectsState,
  TimerSnapshot,
  FullStatePayload,
} from "@/ipc/generated";
import {
  settingsPayloadForTest,
  createSettingsForTest,
} from "./settingsFixture";

export function createTimerEffectsForTest(
  overrides: Partial<TimerEffectsState> = {},
): TimerEffectsState {
  return {
    break_lock: { target: "unlocked", last_attempt: null },
    last_timer_save: null,
    ...overrides,
  };
}

export function createTimerSnapshotForTest(
  overrides: Partial<TimerSnapshot> = {},
  timer: Partial<FullStatePayload> = {},
): TimerSnapshot {
  return {
    revision: "0",
    timer: {
      status: "idle",
      remaining_seconds: 1500,
      session_duration_secs: 1500,
      session_type: "work",
      current_cycle: 0,
      completed_pomodoros: 0,
      break_interaction_count: 0,
      away_during_break: false,
      settings: settingsPayloadForTest(createSettingsForTest()),
      settings_file_status: "ok",
      post_break_decision_pending: false,
      boot_auto_start_offer: null,
      active_window_summary: null,
      ...timer,
    },
    ...overrides,
    effects: overrides.effects ?? createTimerEffectsForTest(),
  };
}
