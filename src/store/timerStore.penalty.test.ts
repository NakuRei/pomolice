import { beforeEach, describe, expect, it } from "vitest";
import {
  useTimerStore,
  SessionType,
  TimerStatus,
  type TimerTickPayload,
} from "./timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

const breakTick: TimerTickPayload = {
  status: TimerStatus.ON_BREAK,
  remaining_seconds: 300,
  session_duration_secs: 300,
  session_type: SessionType.SHORT_BREAK,
  current_cycle: 1,
  completed_pomodoros: 1,
  break_interaction_count: 0,
  is_fullscreen_break: true,
  away_during_break: false,
};

describe("backend penalty state", () => {
  beforeEach(resetTimerStoreForTest);

  it("reflects the penalty count and countdown reset from successive backend ticks", () => {
    const { handleTimerTick } = useTimerStore.getState();
    handleTimerTick({
      ...breakTick,
      remaining_seconds: 30,
      break_interaction_count: 5,
    });

    expect(useTimerStore.getState()).toMatchObject({
      remainingSeconds: 30,
      breakInteractionCount: 5,
    });

    handleTimerTick({ ...breakTick, break_interaction_count: 6 });

    expect(useTimerStore.getState()).toMatchObject({
      remainingSeconds: 300,
      breakInteractionCount: 6,
    });
  });

  it("clears the previous penalty count when the backend starts a new break", () => {
    const { handleTimerTick } = useTimerStore.getState();
    handleTimerTick({ ...breakTick, break_interaction_count: 10 });

    handleTimerTick({
      ...breakTick,
      current_cycle: 2,
      completed_pomodoros: 2,
    });

    expect(useTimerStore.getState().breakInteractionCount).toBe(0);
  });

  it("restores the penalty count and fullscreen break from the full backend state", () => {
    useTimerStore.getState().handleFullState({
      ...breakTick,
      remaining_seconds: 250,
      session_type: SessionType.LONG_BREAK,
      current_cycle: 4,
      completed_pomodoros: 4,
      break_interaction_count: 7,
      post_break_decision_pending: false,
      boot_auto_start_offer: null,
      active_window_summary: null,
      session_duration_secs: 900,
      settings: settingsPayloadForTest(createSettingsForTest()),
      settings_file_status: "ok",
    });

    expect(useTimerStore.getState()).toMatchObject({
      breakInteractionCount: 7,
      isFullscreenBreak: true,
    });
  });

  it("enters and exits the fullscreen break as reported by the backend", () => {
    const { handleTimerTick } = useTimerStore.getState();
    handleTimerTick(breakTick);

    expect(useTimerStore.getState().isFullscreenBreak).toBe(true);

    handleTimerTick({
      ...breakTick,
      status: TimerStatus.WORKING,
      remaining_seconds: 1500,
      session_duration_secs: 1500,
      session_type: SessionType.WORK,
      current_cycle: 2,
      is_fullscreen_break: false,
      away_during_break: false,
    });

    expect(useTimerStore.getState().isFullscreenBreak).toBe(false);
  });
});
