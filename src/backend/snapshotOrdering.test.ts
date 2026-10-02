import { beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "./observationStore";
import { useOperationStore } from "./operationStore";
import { startWork } from "@/features/timer/commands";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { resetAppStateForTest } from "@/test/resetAppState";
import type { CommandResponse, TimerSnapshot } from "@/ipc/generated";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
beforeEach(() => {
  resetAppStateForTest();
  vi.mocked(invoke).mockReset();
});

it("keeps every field of a newer snapshot when an old or duplicate observation arrives", () => {
  const older = createTimerSnapshotForTest({ revision: "9007199254740992" });
  const newer = createTimerSnapshotForTest({
    revision: "9007199254740993",
    timer: {
      ...older.timer,
      post_break_decision_pending: true,
      settings: { ...older.timer.settings, notification_sound_muted: true },
    },
    effects: { ...older.effects, last_timer_save: "failed" },
  });
  const { observeSnapshot } = useBackendObservationStore.getState();
  observeSnapshot(newer);
  const observed = useBackendObservationStore.getState();
  observeSnapshot(older);
  observeSnapshot({ ...older, revision: newer.revision });
  expect(useBackendObservationStore.getState()).toEqual(observed);
  expect(observed.requiresManualContinueDecision).toBe(true);
  expect(observed.settings?.notificationSoundMuted).toBe(true);
  expect(observed.effects?.last_timer_save).toBe("failed");
});

it("records a successful operation even when its response snapshot is older than an event", async () => {
  let finish: (reply: CommandResponse<TimerSnapshot>) => void = () => {
    throw new Error("Expected a pending command");
  };
  vi.mocked(invoke).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = startWork();
  const latest = createTimerSnapshotForTest({ revision: "2" });
  latest.timer.post_break_decision_pending = true;
  useBackendObservationStore.getState().observeSnapshot(latest);
  finish({
    status: "accepted",
    value: createTimerSnapshotForTest({ revision: "1" }),
  });
  await expect(pending).resolves.toMatchObject({ status: "accepted" });
  expect(
    useBackendObservationStore.getState().requiresManualContinueDecision,
  ).toBe(true);
  expect(useOperationStore.getState().operations.start_work?.outcome).toEqual({
    status: "accepted",
  });
});

it("adopts a command's complete state even before any event arrives", async () => {
  const snapshot = createTimerSnapshotForTest({ revision: "1" });
  snapshot.timer.status = "working";
  vi.mocked(invoke).mockResolvedValueOnce({
    status: "accepted",
    value: snapshot,
  });
  await startWork();
  expect(useBackendObservationStore.getState().status).toBe("working");
  expect(useBackendObservationStore.getState().settings).not.toBeNull();
});
