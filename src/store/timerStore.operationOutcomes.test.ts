import {
  useTimerObservationStore,
  selectBreakLockObservation,
} from "@/store/timerObservationStore";
import {
  startWork,
  emergencyStop,
  resolvePostBreakDecision,
  setAwayDuringBreak,
  saveSettings,
  resolveBootAutoStartPrompt,
  retryFailedEffects,
} from "@/timer/commands";
import { canRetryOperation } from "@/timer/retryOperation";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useTimerOperationStore } from "@/store/timerOperationStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("accepted timer operations", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    resetTimerStoreForTest();
    useTimerObservationStore.setState(
      useTimerObservationStore.getInitialState(),
      true,
    );
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
    useTimerObservationStore.setState({
      bootAutoStartOffer: createBootAutoStartOfferForTest(),
      requiresManualContinueDecision: true,
    });
  });

  it.each([
    {
      name: "start_work",
      run: (): Promise<unknown> => startWork(),
    },
    {
      name: "emergency_stop",
      run: (): Promise<unknown> => emergencyStop(),
    },
    {
      name: "resolve_post_break_decision",
      run: (): Promise<unknown> => resolvePostBreakDecision("continue"),
    },
    {
      name: "resolve_boot_auto_start_prompt",
      run: (): Promise<unknown> => resolveBootAutoStartPrompt(true),
    },
    {
      name: "set_away_during_break",
      run: (): Promise<unknown> => setAwayDuringBreak(true),
    },
    {
      name: "update_settings",
      run: (): Promise<unknown> =>
        saveSettings({ notificationSoundMuted: true }),
    },
  ] as const)(
    "keeps $name accepted when an effect failed",
    async ({ name, run }) => {
      const effects = createTimerEffectsForTest({
        revision: "1",
        last_timer_save: "failed",
      });
      vi.mocked(invoke).mockResolvedValue({
        status: "accepted",
        value: createTimerSnapshotForTest({ effects }),
      });
      await run();
      expect(useTimerObservationStore.getState().effects).toEqual(effects);
      expect(
        useTimerOperationStore.getState().operations[name]?.outcome,
      ).toEqual({ status: "accepted" });
      expect(invoke).toHaveBeenCalledTimes(1);
    },
  );

  it.each([
    ["start_work", (): Promise<unknown> => startWork()],
    ["emergency_stop", (): Promise<unknown> => emergencyStop()],
    [
      "resolve_post_break_decision",
      (): Promise<unknown> => resolvePostBreakDecision("continue"),
    ],
    [
      "resolve_boot_auto_start_prompt",
      (): Promise<unknown> => resolveBootAutoStartPrompt(true),
    ],
    ["set_away_during_break", (): Promise<unknown> => setAwayDuringBreak(true)],
    [
      "update_settings",
      (): Promise<unknown> => saveSettings({ notificationSoundMuted: true }),
    ],
  ] as const)(
    "never retries %s when response delivery fails",
    async (name, run) => {
      const offer = useTimerObservationStore.getState().bootAutoStartOffer;
      vi.mocked(invoke).mockRejectedValue({ code: "rate_limited" });
      await expect(run()).resolves.toEqual({
        status: "unavailable",
        cause: { code: "rate_limited" },
      });
      const record = useTimerOperationStore.getState().operations[name];
      expect(record?.outcome.status).toBe("unavailable");
      expect(record && canRetryOperation(record, true)).toBe(false);
      expect(useTimerObservationStore.getState().bootAutoStartOffer).toEqual(
        offer,
      );
      expect(
        useTimerObservationStore.getState().requiresManualContinueDecision,
      ).toBe(true);
      expect(useTimerObservationStore.getState().effects).toBeNull();
      expect(invoke).toHaveBeenCalledTimes(1);
    },
  );

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

  it("retries the failed effects without resending the accepted timer command", async () => {
    useTimerObservationStore
      .getState()
      .observeEffects(
        createTimerEffectsForTest({ revision: "1", last_timer_save: "failed" }),
      );
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          revision: "2",
          last_timer_save: "succeeded",
        }),
      }),
    });
    await retryFailedEffects();
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    expect(useTimerObservationStore.getState().effects?.last_timer_save).toBe(
      "succeeded",
    );
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
