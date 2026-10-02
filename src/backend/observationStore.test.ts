import { beforeEach, describe, expect, it } from "vitest";
import {
  useBackendObservationStore,
  selectBreakLockObservation,
} from "./observationStore";
import { resetAppStateForTest } from "@/test/resetAppState";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

beforeEach(() => {
  useBackendObservationStore.setState(
    useBackendObservationStore.getInitialState(),
    true,
  );
});

describe("timer observations", () => {
  it("starts without claiming to have observed an idle timer", () => {
    expect(useBackendObservationStore.getState().status).toBeNull();
    expect(useBackendObservationStore.getState().settings).toBeNull();
  });

  it("does not resurrect a failure from an old reply after recovery", () => {
    useBackendObservationStore.getState().observeSnapshot(
      createTimerSnapshotForTest({
        revision: "3",
        effects: createTimerEffectsForTest({
          last_timer_save: "succeeded",
        }),
      }),
    );
    useBackendObservationStore.getState().observeSnapshot(
      createTimerSnapshotForTest({
        revision: "2",
        effects: createTimerEffectsForTest({ last_timer_save: "failed" }),
      }),
    );
    expect(useBackendObservationStore.getState().effects?.last_timer_save).toBe(
      "succeeded",
    );
  });

  describe("observeSnapshot()", () => {
    it("takes the settings and file status the backend announces", () => {
      const settings = createSettingsForTest({ notificationSoundMuted: true });

      useBackendObservationStore.getState().observeSnapshot(
        createTimerSnapshotForTest(
          {},
          {
            settings: settingsPayloadForTest(settings),
            settings_file_status: "ok",
          },
        ),
      );

      expect(useBackendObservationStore.getState().settings).toEqual(settings);
      expect(useBackendObservationStore.getState().settingsFileStatus).toBe(
        "ok",
      );
    });
  });
});

describe("lock observation", () => {
  it.each([
    { attempt: null, observed: "unknown" },
    { attempt: { action: "enter", status: "succeeded" }, observed: "locked" },
    {
      attempt: { action: "release", status: "succeeded" },
      observed: "unlocked",
    },
    { attempt: { action: "enter", status: "failed" }, observed: "unknown" },
    { attempt: { action: "release", status: "failed" }, observed: "unknown" },
  ] as const)("derives $observed from $attempt", ({ attempt, observed }) => {
    expect(
      selectBreakLockObservation({
        effects: createTimerEffectsForTest({
          break_lock: { target: "unlocked", last_attempt: attempt },
        }),
      }),
    ).toBe(observed);
  });

  it("never treats a failed release as a confirmed lock or a timer phase", () => {
    resetAppStateForTest();
    useBackendObservationStore.getState().observeSnapshot(
      createTimerSnapshotForTest({
        revision: "1",
        effects: createTimerEffectsForTest({
          break_lock: {
            target: "unlocked",
            last_attempt: { action: "release", status: "failed" },
          },
        }),
      }),
    );
    expect(
      selectBreakLockObservation(useBackendObservationStore.getState()),
    ).toBe("unknown");
    expect(useBackendObservationStore.getState().status).toBe("idle");
    expect(
      useBackendObservationStore.getState().effects?.break_lock.last_attempt
        ?.status,
    ).toBe("failed");
  });
});
