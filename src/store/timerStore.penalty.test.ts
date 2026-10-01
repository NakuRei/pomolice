import {
  useTimerObservationStore,
  selectBreakLockObservation,
} from "@/store/timerObservationStore";
import type { TimerTickPayload } from "@/ipc/generated";
import { beforeEach, describe, expect, it } from "vitest";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

const breakTick: TimerTickPayload = {
  status: "on_break",
  remaining_seconds: 300,
  session_duration_secs: 300,
  session_type: "short_break",
  current_cycle: 1,
  completed_pomodoros: 1,
  break_interaction_count: 0,
  away_during_break: false,
};

describe("backend penalty state", () => {
  beforeEach(resetTimerStoreForTest);

  it("reflects the penalty count and countdown reset from successive backend ticks", () => {
    const { observeTimerTick } = useTimerObservationStore.getState();
    observeTimerTick({
      ...breakTick,
      remaining_seconds: 30,
      break_interaction_count: 5,
    });

    expect(useTimerObservationStore.getState()).toMatchObject({
      remainingSeconds: 30,
      breakInteractionCount: 5,
    });

    observeTimerTick({ ...breakTick, break_interaction_count: 6 });

    expect(useTimerObservationStore.getState()).toMatchObject({
      remainingSeconds: 300,
      breakInteractionCount: 6,
    });
  });

  it("clears the previous penalty count when the backend starts a new break", () => {
    const { observeTimerTick } = useTimerObservationStore.getState();
    observeTimerTick({ ...breakTick, break_interaction_count: 10 });

    observeTimerTick({
      ...breakTick,
      current_cycle: 2,
      completed_pomodoros: 2,
    });

    expect(useTimerObservationStore.getState().breakInteractionCount).toBe(0);
  });

  it("restores the penalty count and fullscreen break from the full backend state", () => {
    useTimerObservationStore.getState().observeFullState({
      timer: {
        ...breakTick,
        remaining_seconds: 250,
        session_type: "long_break",
        current_cycle: 4,
        completed_pomodoros: 4,
        break_interaction_count: 7,
        post_break_decision_pending: false,
        boot_auto_start_offer: null,
        active_window_summary: null,
        session_duration_secs: 900,
        settings: settingsPayloadForTest(createSettingsForTest()),
        settings_file_status: "ok",
      },
      effects: {
        revision: "1",
        last_timer_save: null,
        break_lock: {
          target: "locked",
          last_attempt: { action: "enter", status: "succeeded" },
        },
      },
    });

    expect(useTimerObservationStore.getState().breakInteractionCount).toBe(7);
    expect(
      selectBreakLockObservation(useTimerObservationStore.getState()),
    ).toBe("locked");
  });

  it("timer ticks never overwrite the independently reported lock result", () => {
    const { observeTimerTick } = useTimerObservationStore.getState();
    useTimerObservationStore.getState().observeEffects({
      revision: "1",
      last_timer_save: null,
      break_lock: {
        target: "locked",
        last_attempt: { action: "enter", status: "succeeded" },
      },
    });
    observeTimerTick(breakTick);

    expect(
      selectBreakLockObservation(useTimerObservationStore.getState()),
    ).toBe("locked");

    observeTimerTick({
      ...breakTick,
      status: "working",
      remaining_seconds: 1500,
      session_duration_secs: 1500,
      session_type: "work",
      current_cycle: 2,
      away_during_break: false,
    });

    expect(
      selectBreakLockObservation(useTimerObservationStore.getState()),
    ).toBe("locked");
    expect(useTimerObservationStore.getState().status).toBe("working");
  });
});
