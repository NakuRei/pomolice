import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useTimerStore } from "./timerStore";
import {
  useTimerEffectsStore,
  selectBreakLockObservation,
} from "./timerEffectsStore";
import { useErrorStore } from "./errorStore";
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
    useTimerEffectsStore.setState(useTimerEffectsStore.getInitialState(), true);
    useErrorStore.getState().clearAllErrors();
    useTimerStore.setState({
      bootAutoStartOffer: createBootAutoStartOfferForTest(),
      requiresManualContinueDecision: true,
    });
  });

  it.each([
    {
      name: "start_work",
      run: (): Promise<unknown> => useTimerStore.getState().start(),
    },
    {
      name: "emergency_stop",
      run: (): Promise<unknown> => useTimerStore.getState().emergencyStop(),
    },
    {
      name: "resolve_post_break_decision",
      run: (): Promise<unknown> =>
        useTimerStore.getState().resolvePostBreakDecision("continue"),
    },
    {
      name: "resolve_boot_auto_start_prompt",
      run: (): Promise<unknown> =>
        useTimerStore.getState().resolveBootAutoStartPrompt(true),
    },
    {
      name: "set_away_during_break",
      run: (): Promise<unknown> =>
        useTimerStore.getState().setAwayDuringBreak(true),
    },
    {
      name: "update_settings",
      run: (): Promise<unknown> =>
        useTimerStore.getState().saveSettings({ notificationSoundMuted: true }),
    },
  ])("keeps $name accepted when an effect failed", async ({ name, run }) => {
    const effects = createTimerEffectsForTest({
      revision: "1",
      last_timer_save: "failed",
    });
    vi.mocked(invoke).mockResolvedValue(
      createTimerSnapshotForTest({ effects }),
    );
    await run();
    expect(useTimerEffectsStore.getState().effects).toEqual(effects);
    expect(useErrorStore.getState().errors[name]).toBeUndefined();
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not resurrect a failure from an old reply after recovery", () => {
    useTimerEffectsStore.getState().observe(
      createTimerEffectsForTest({
        revision: "3",
        last_timer_save: "succeeded",
      }),
    );
    useTimerEffectsStore
      .getState()
      .observe(
        createTimerEffectsForTest({ revision: "2", last_timer_save: "failed" }),
      );
    expect(useTimerEffectsStore.getState().effects?.last_timer_save).toBe(
      "succeeded",
    );
  });

  it("retries the failed effects without resending the accepted timer command", async () => {
    useTimerEffectsStore
      .getState()
      .observe(
        createTimerEffectsForTest({ revision: "1", last_timer_save: "failed" }),
      );
    vi.mocked(invoke).mockResolvedValue(
      createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          revision: "2",
          last_timer_save: "succeeded",
        }),
      }),
    );
    await useTimerEffectsStore.getState().retry();
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    expect(useTimerEffectsStore.getState().effects?.last_timer_save).toBe(
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
    useTimerEffectsStore.getState().observe(
      createTimerEffectsForTest({
        revision: "1",
        break_lock: {
          target: "unlocked",
          last_attempt: { action: "release", status: "failed" },
        },
      }),
    );
    expect(selectBreakLockObservation(useTimerEffectsStore.getState())).toBe(
      "unknown",
    );
    expect(useTimerStore.getState().status).toBe("idle");
    expect(
      useTimerEffectsStore.getState().effects?.break_lock.last_attempt?.status,
    ).toBe("failed");
  });
});
