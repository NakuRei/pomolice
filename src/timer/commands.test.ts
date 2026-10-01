import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useTimerObservationStore } from "@/store/timerObservationStore";
import { useTimerOperationStore } from "@/operations/operationStore";
import { startWork, retryFailedEffects } from "./commands";
import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer effect recovery commands", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    useTimerObservationStore.setState(
      useTimerObservationStore.getInitialState(),
      true,
    );
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
  });

  it("keeps acceptance separate from failed effects and retries only effects", async () => {
    const effects = createTimerEffectsForTest({
      revision: "1",
      last_timer_save: "failed",
    });
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({ effects }),
    });
    expect((await startWork()).status).toBe("accepted");
    expect(
      useTimerOperationStore.getState().operations.start_work?.outcome.status,
    ).toBe("accepted");
    expect(useTimerObservationStore.getState().effects).toEqual(effects);
    await retryFailedEffects();
    expect(vi.mocked(invoke).mock.calls.map(([name]) => name)).toEqual([
      "start_work",
      "retry_failed_timer_effects",
    ]);
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

  it("coalesces overlapping effect recovery requests and allows the next attempt", async () => {
    let finish: (value: CommandResponse<TimerSnapshot>) => void = () => {
      throw new Error("missing recovery request");
    };
    vi.mocked(invoke).mockImplementationOnce(
      () =>
        new Promise<CommandResponse<TimerSnapshot>>((resolve) => {
          finish = resolve;
        }),
    );
    const first = retryFailedEffects();
    const second = retryFailedEffects();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(
      useTimerOperationStore.getState().operations.retry_failed_timer_effects
        ?.outcome.status,
    ).toBe("pending");
    finish({ status: "rejected", error: { code: "rate_limited" } });
    expect(await second).toEqual(await first);
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    await retryFailedEffects();
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
