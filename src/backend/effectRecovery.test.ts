import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "./observationStore";
import { useOperationStore } from "./operationStore";
import { startWork } from "@/features/timer/commands";
import { executeOperation } from "@/backend/executeOperation";
import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer effect recovery commands", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    useBackendObservationStore.setState(
      useBackendObservationStore.getInitialState(),
      true,
    );
    useOperationStore.setState(useOperationStore.getInitialState(), true);
  });

  it("keeps acceptance separate from failed effects and retries only effects", async () => {
    const effects = createTimerEffectsForTest({
      last_timer_save: "failed",
    });
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({ effects }),
    });
    expect((await startWork()).status).toBe("accepted");
    expect(
      useOperationStore.getState().operations.start_work?.outcome.status,
    ).toBe("accepted");
    expect(useBackendObservationStore.getState().effects).toEqual(effects);
    await executeOperation({ kind: "retry_failed_timer_effects" });
    expect(vi.mocked(invoke).mock.calls.map(([name]) => name)).toEqual([
      "start_work",
      "retry_failed_timer_effects",
    ]);
  });

  it("retries the failed effects without resending the accepted timer command", async () => {
    useBackendObservationStore.setState({
      effects: createTimerEffectsForTest({ last_timer_save: "failed" }),
    });
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          last_timer_save: "succeeded",
        }),
      }),
    });
    await executeOperation({ kind: "retry_failed_timer_effects" });
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    expect(useBackendObservationStore.getState().effects?.last_timer_save).toBe(
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
    const first = executeOperation({ kind: "retry_failed_timer_effects" });
    const second = executeOperation({ kind: "retry_failed_timer_effects" });
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(
      useOperationStore.getState().operations.retry_failed_timer_effects
        ?.outcome.status,
    ).toBe("pending");
    finish({ status: "rejected", error: { code: "rate_limited" } });
    expect(await second).toEqual(await first);
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    await executeOperation({ kind: "retry_failed_timer_effects" });
    expect(invoke).toHaveBeenCalledTimes(2);
  });
});
