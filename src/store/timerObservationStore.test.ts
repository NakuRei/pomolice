import { beforeEach, describe, expect, it } from "vitest";
import {
  useTimerObservationStore,
  selectBreakLockObservation,
} from "./timerObservationStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createTimerEffectsForTest } from "@/test/timerEffectsFixture";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

beforeEach(() => {
  useTimerObservationStore.setState(
    useTimerObservationStore.getInitialState(),
    true,
  );
});

describe("timer observations", () => {
  it("starts without claiming to have observed an idle timer", () => {
    expect(useTimerObservationStore.getState().status).toBeNull();
    expect(useTimerObservationStore.getState().settings).toBeNull();
  });

  it("does not resurrect a failure from an old reply after recovery", () => {
    useTimerObservationStore.getState().observeEffects(
      createTimerEffectsForTest({
        revision: "3",
        last_timer_save: "succeeded",
      }),
    );
    useTimerObservationStore
      .getState()
      .observeEffects(
        createTimerEffectsForTest({ revision: "2", last_timer_save: "failed" }),
      );
    expect(useTimerObservationStore.getState().effects?.last_timer_save).toBe(
      "succeeded",
    );
  });

  describe("observeSettingsChanged()", () => {
    it("takes the settings and file status the backend announces", () => {
      const settings = createSettingsForTest({ notificationSoundMuted: true });

      useTimerObservationStore.getState().observeSettingsChanged({
        settings: settingsPayloadForTest(settings),
        settings_file_status: "ok",
      });

      expect(useTimerObservationStore.getState().settings).toEqual(settings);
      expect(useTimerObservationStore.getState().settingsFileStatus).toBe("ok");
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
    resetTimerStoreForTest();
    useTimerObservationStore.getState().observeEffects(
      createTimerEffectsForTest({
        revision: "1",
        break_lock: {
          target: "unlocked",
          last_attempt: { action: "release", status: "failed" },
        },
      }),
    );
    expect(
      selectBreakLockObservation(useTimerObservationStore.getState()),
    ).toBe("unknown");
    expect(useTimerObservationStore.getState().status).toBeNull();
    expect(
      useTimerObservationStore.getState().effects?.break_lock.last_attempt
        ?.status,
    ).toBe("failed");
  });
});
