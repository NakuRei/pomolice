import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "@/backend/operationStore";
import { startWork } from "@/features/timer/commands";
import { saveSettings } from "@/features/settings/commands";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { retryRejectedOperation } from "./retryOperation";
import { executeOperation } from "./executeOperation";
import type { Operation } from "./operation";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("rejected operation retry", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    useBackendObservationStore.setState(
      useBackendObservationStore.getInitialState(),
      true,
    );
    useOperationStore.setState(useOperationStore.getInitialState(), true);
  });

  it("retries the submitted settings even if the caller edits its draft later", async () => {
    const changes = { notificationSoundMuted: true };
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    await saveSettings(changes);
    changes.notificationSoundMuted = false;
    const record = useOperationStore.getState().operations.update_settings;
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
    const previous = useOperationStore.getState().operations.start_work;
    if (!previous) throw new Error("Expected start operation");
    await startWork();
    await retryRejectedOperation(previous);
    useOperationStore.getState().dismiss("start_work", previous.id);
    expect(invoke).toHaveBeenCalledTimes(2);
    expect(useOperationStore.getState().operations.start_work).toMatchObject({
      dismissed: false,
      outcome: { status: "rejected" },
    });
  });

  it.each([
    { kind: "resolve_post_break_decision", action: "continue" },
    { kind: "resolve_boot_auto_start_prompt", startNow: true },
  ] satisfies Operation[])(
    "closes the pending decision after an accepted $kind retry",
    async (request) => {
      const offer = createBootAutoStartOfferForTest();
      useBackendObservationStore.setState({
        bootAutoStartOffer: offer,
        requiresManualContinueDecision: true,
      });
      vi.mocked(invoke).mockResolvedValueOnce({
        status: "rejected",
        error: { code: "rate_limited" },
      });
      await executeOperation(request);
      const record = useOperationStore.getState().operations[request.kind];
      if (!record) throw new Error("Expected rejected decision");
      expect(useBackendObservationStore.getState().bootAutoStartOffer).toEqual(
        offer,
      );
      expect(
        useBackendObservationStore.getState().requiresManualContinueDecision,
      ).toBe(true);
      vi.mocked(invoke).mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });
      await retryRejectedOperation(record);
      const state = useBackendObservationStore.getState();
      if (request.kind === "resolve_boot_auto_start_prompt") {
        expect(state.bootAutoStartOffer).toBeNull();
        expect(state.requiresManualContinueDecision).toBe(true);
      } else {
        expect(state.requiresManualContinueDecision).toBe(false);
        expect(state.bootAutoStartOffer).toEqual(offer);
      }
    },
  );
});
