import { useTimerObservationStore } from "@/store/timerObservationStore";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { startWork, saveSettings, retryFailedEffects } from "./commands";
import { retryRejectedOperation } from "./retryOperation";
import { useTimerOperationStore } from "@/store/timerOperationStore";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";
import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer operation ownership", () => {
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

  it("starts without claiming to have observed an idle timer", () => {
    expect(useTimerObservationStore.getState().status).toBeNull();
    expect(useTimerObservationStore.getState().settings).toBeNull();
  });

  it("retries the submitted settings even if the caller edits its draft later", async () => {
    const changes = { notificationSoundMuted: true };
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    await saveSettings(changes);
    changes.notificationSoundMuted = false;
    const record = useTimerOperationStore.getState().operations.update_settings;
    if (!record) throw new Error("Expected settings operation");
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    await retryRejectedOperation(record);
    expect(invoke).toHaveBeenLastCalledWith("update_settings", {
      changes: { notification_sound_muted: true },
    });
  });

  it("ignores retry and dismissal captured from a replaced operation", async () => {
    vi.mocked(invoke).mockResolvedValue({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    await startWork();
    const previous = useTimerOperationStore.getState().operations.start_work;
    if (!previous) throw new Error("Expected start operation");
    await startWork();
    await retryRejectedOperation(previous);
    useTimerOperationStore.getState().dismiss("start_work", previous.id);
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(
      useTimerOperationStore.getState().operations.start_work,
    ).toMatchObject({ dismissed: false, outcome: { status: "rejected" } });
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

  it("retains an unknown result when its notice is dismissed or state is observed", async () => {
    vi.mocked(invoke).mockRejectedValue({ code: "rate_limited" });
    expect((await startWork()).status).toBe("unavailable");
    const record = useTimerOperationStore.getState().operations.start_work;
    if (!record) throw new Error("missing operation");
    useTimerOperationStore.getState().dismiss("start_work", record.id);
    useTimerObservationStore
      .getState()
      .observeFullState(createTimerSnapshotForTest());
    expect(
      useTimerOperationStore.getState().operations.start_work,
    ).toMatchObject({
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
      useTimerOperationStore.getState().operations.update_settings?.outcome,
    ).toEqual({
      status: "rejected",
      error: { code: "persistence_error" },
    });
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
