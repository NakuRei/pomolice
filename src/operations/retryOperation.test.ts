import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useTimerObservationStore } from "@/store/timerObservationStore";
import { useTimerOperationStore } from "@/operations/operationStore";
import { startWork } from "@/timer/commands";
import { saveSettings } from "@/features/settings/commands";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { retryRejectedOperation } from "./retryOperation";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("rejected operation retry", () => {
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
});
