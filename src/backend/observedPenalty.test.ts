import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import {
  useBackendObservationStore,
  selectBreakLockObservation,
} from "./observationStore";
import type { FullStatePayload } from "@/ipc/generated";
import { beforeEach, describe, expect, it } from "vitest";

import { resetAppStateForTest } from "@/test/resetAppState";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

const breakTick = {
  status: "on_break",
  remaining_seconds: 300,
  session_duration_secs: 300,
  session_type: "short_break",
  current_cycle: 1,
  completed_pomodoros: 1,
  break_interaction_count: 0,
  away_during_break: false,
} satisfies Partial<FullStatePayload>;

describe("backend penalty state", () => {
  beforeEach(resetAppStateForTest);

  it("reflects the penalty count and countdown reset from successive backend ticks", () => {
    const { observeSnapshot } = useBackendObservationStore.getState();
    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        {
          ...breakTick,
          remaining_seconds: 30,
          break_interaction_count: 5,
        },
      ),
    );

    expect(useBackendObservationStore.getState()).toMatchObject({
      remainingSeconds: 30,
      breakInteractionCount: 5,
    });

    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        { ...breakTick, break_interaction_count: 6 },
      ),
    );

    expect(useBackendObservationStore.getState()).toMatchObject({
      remainingSeconds: 300,
      breakInteractionCount: 6,
    });
  });

  it("clears the previous penalty count when the backend starts a new break", () => {
    const { observeSnapshot } = useBackendObservationStore.getState();
    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        { ...breakTick, break_interaction_count: 10 },
      ),
    );

    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        {
          ...breakTick,
          current_cycle: 2,
          completed_pomodoros: 2,
        },
      ),
    );

    expect(useBackendObservationStore.getState().breakInteractionCount).toBe(0);
  });

  it("restores the penalty count and fullscreen break from the full backend state", () => {
    useBackendObservationStore.getState().observeSnapshot({
      revision: "1",
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
        last_timer_save: null,
        break_lock: {
          target: "locked",
          last_attempt: { action: "enter", status: "succeeded" },
        },
      },
    });

    expect(useBackendObservationStore.getState().breakInteractionCount).toBe(7);
    expect(
      selectBreakLockObservation(useBackendObservationStore.getState()),
    ).toBe("locked");
  });

  it("snapshots report the lock result independently of the timer phase", () => {
    const { observeSnapshot } = useBackendObservationStore.getState();
    useBackendObservationStore.getState().observeSnapshot(
      createTimerSnapshotForTest({
        revision: "1",
        effects: {
          last_timer_save: null,
          break_lock: {
            target: "locked",
            last_attempt: { action: "enter", status: "succeeded" },
          },
        },
      }),
    );
    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        breakTick,
      ),
    );

    expect(
      selectBreakLockObservation(useBackendObservationStore.getState()),
    ).toBe("locked");

    observeSnapshot(
      createTimerSnapshotForTest(
        {
          revision: String(
            BigInt(useBackendObservationStore.getState().revision ?? "0") + 1n,
          ),
          effects: useBackendObservationStore.getState().effects ?? undefined,
        },
        {
          ...breakTick,
          status: "working",
          remaining_seconds: 1500,
          session_duration_secs: 1500,
          session_type: "work",
          current_cycle: 2,
          away_during_break: false,
        },
      ),
    );

    expect(
      selectBreakLockObservation(useBackendObservationStore.getState()),
    ).toBe("locked");
    expect(useBackendObservationStore.getState().status).toBe("working");
  });
});
