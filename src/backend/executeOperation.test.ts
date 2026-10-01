import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "@/backend/operationStore";
import {
  startWork,
  emergencyStop,
  resolvePostBreakDecision,
  setAwayDuringBreak,
  resolveBootAutoStartPrompt,
} from "@/features/timer/commands";
import { saveSettings } from "@/features/settings/commands";
import { canRetryOperation } from "./retryOperation";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer command execution", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    resetAppStateForTest();
    useBackendObservationStore.setState(
      useBackendObservationStore.getInitialState(),
      true,
    );
    useOperationStore.setState(useOperationStore.getInitialState(), true);
    useBackendObservationStore.setState({
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
      expect(useBackendObservationStore.getState().effects).toEqual(effects);
      expect(useOperationStore.getState().operations[name]?.outcome).toEqual({
        status: "accepted",
      });
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
      const offer = useBackendObservationStore.getState().bootAutoStartOffer;
      vi.mocked(invoke).mockRejectedValue({ code: "rate_limited" });
      await expect(run()).resolves.toEqual({
        status: "unavailable",
        cause: { code: "rate_limited" },
      });
      const record = useOperationStore.getState().operations[name];
      expect(record?.outcome.status).toBe("unavailable");
      expect(record && canRetryOperation(record, true)).toBe(false);
      expect(useBackendObservationStore.getState().bootAutoStartOffer).toEqual(
        offer,
      );
      expect(
        useBackendObservationStore.getState().requiresManualContinueDecision,
      ).toBe(true);
      expect(useBackendObservationStore.getState().effects).toBeNull();
      expect(invoke).toHaveBeenCalledTimes(1);
    },
  );

  it("retains an unknown result when its notice is dismissed or state is observed", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "rate_limited" });
    expect((await startWork()).status).toBe("unavailable");
    const record = useOperationStore.getState().operations.start_work;
    if (!record) throw new Error("missing operation");
    useOperationStore.getState().dismiss("start_work", record.id);
    useBackendObservationStore
      .getState()
      .observeFullState(createTimerSnapshotForTest());
    expect(useOperationStore.getState().operations.start_work).toMatchObject({
      outcome: { status: "unavailable" },
      dismissed: true,
    });
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not let a late reply erase the result of a newer settings request", async () => {
    let finish: (value: CommandResponse<TimerSnapshot>) => void = () => {
      throw new Error("missing first request");
    };
    vi.mocked(invoke).mockImplementationOnce(
      () =>
        new Promise<CommandResponse<TimerSnapshot>>((resolve) => {
          finish = resolve;
        }),
    );
    const first = saveSettings({ notificationSoundMuted: true });
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "persistence_error" },
    });
    await saveSettings({ notificationSoundMuted: false });
    finish({ status: "accepted", value: createTimerSnapshotForTest() });
    await first;
    expect(
      useOperationStore.getState().operations.update_settings?.outcome,
    ).toEqual({
      status: "rejected",
      error: { code: "persistence_error" },
    });
  });
});
