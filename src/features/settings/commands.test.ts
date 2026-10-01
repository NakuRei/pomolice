import {
  useTimerObservationStore,
  selectBreakLockObservation,
} from "@/store/timerObservationStore";
import {
  saveSettings,
  toggleNotificationSoundMuted,
  savePostBreakTransitionPolicy,
} from "./commands";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { beforeEach, describe, expect, it, vi } from "vitest";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

import { useTimerOperationStore } from "@/operations/operationStore";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

describe("settings commands", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
    resetTimerStoreForTest();
  });

  describe("saveSettings()", () => {
    it("sends only the changed settings", async () => {
      mockInvoke.mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });

      const result = await saveSettings({ workDurationSecs: 30 * 60 });

      expect(result).toMatchObject({ status: "accepted" });
      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: { work_duration_secs: 30 * 60 },
      });
    });

    it("leaves every piece of state to the events, so a late reply cannot roll it back", async () => {
      const settings = createSettingsForTest();
      useTimerObservationStore.setState({
        settings,
        status: "on_break",
        sessionType: "short_break",
        remainingSeconds: 299,
      });
      useTimerObservationStore.getState().observeEffects({
        revision: "2",
        last_timer_save: null,
        break_lock: {
          target: "locked",
          last_attempt: { action: "enter", status: "succeeded" },
        },
      });
      const stateBeforeTheReply = useTimerObservationStore.getState();
      mockInvoke.mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });

      await saveSettings({ workDurationSecs: 60 });

      const state = useTimerObservationStore.getState();
      expect(state.settings).toBe(stateBeforeTheReply.settings);
      expect(state.status).toBe("on_break");
      expect(state.remainingSeconds).toBe(299);
      expect(
        selectBreakLockObservation(useTimerObservationStore.getState()),
      ).toBe("locked");
    });

    it("returns every rejected input without reporting a global error", async () => {
      const issues = [
        {
          code: "out_of_range",
          field: "work_duration_secs",
          min: 1,
          max: 7200,
        },
        {
          code: "overlapping_time_ranges",
          primary_weekday: "mon",
          secondary_weekday: "tue",
        },
      ];
      mockInvoke.mockResolvedValueOnce({
        status: "rejected",
        error: {
          code: "invalid_settings",
          message: "Invalid settings",
          issues,
        },
      });

      const result = await saveSettings(createSettingsForTest());

      expect(result).toMatchObject({
        status: "rejected",
        error: { code: "invalid_settings", issues },
      });
      expect(useTimerObservationStore.getState().settings).toBeNull();
      expect(
        useTimerOperationStore.getState().operations.update_settings?.outcome,
      ).toEqual(result);
    });

    it("reports any other failure and keeps the previous settings", async () => {
      const previousSettings = createSettingsForTest();
      useTimerObservationStore.setState({ settings: previousSettings });
      mockInvoke.mockResolvedValueOnce({
        status: "rejected",
        error: {
          code: "persistence_error",
          message: "Persistence error",
        },
      });

      const result = await saveSettings(
        createSettingsForTest({ workDurationSecs: 60 }),
      );

      expect(result).toMatchObject({
        status: "rejected",
        error: { code: "persistence_error" },
      });
      expect(useTimerObservationStore.getState().settings).toEqual(
        previousSettings,
      );
      expect(
        useTimerOperationStore.getState().operations.update_settings,
      ).toMatchObject({
        request: { kind: "update_settings" },
        outcome: result,
      });
    });

    it("clears a previous save failure once a save succeeds", async () => {
      const settings = createSettingsForTest();
      mockInvoke.mockResolvedValueOnce({
        status: "rejected",
        error: {
          code: "persistence_error",
          message: "Persistence error",
        },
      });
      await saveSettings(settings);
      mockInvoke.mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });

      await saveSettings(settings);

      expect(
        useTimerOperationStore.getState().operations.update_settings?.outcome,
      ).toEqual({ status: "accepted" });
    });
  });

  describe("toggleNotificationSoundMuted()", () => {
    it("saves the settings with the mute flag flipped", async () => {
      useTimerObservationStore.setState({
        settings: createSettingsForTest({ notificationSoundMuted: false }),
      });
      mockInvoke.mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });

      await toggleNotificationSoundMuted();

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: { notification_sound_muted: true },
      });
    });

    it("does nothing before the backend has sent the settings", async () => {
      await toggleNotificationSoundMuted();

      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });

  describe("savePostBreakTransitionPolicy()", () => {
    it("saves the settings with the given policy", async () => {
      useTimerObservationStore.setState({
        settings: createSettingsForTest({
          postBreakTransitionPolicy: "require_manual_continue",
        }),
      });
      mockInvoke.mockResolvedValueOnce({
        status: "accepted",
        value: createTimerSnapshotForTest(),
      });

      await savePostBreakTransitionPolicy("start_next_work_automatically");

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: {
          post_break_transition_policy: "start_next_work_automatically",
        },
      });
    });
  });

  it("treats a rejection that names no input as an ordinary failure", async () => {
    mockInvoke.mockResolvedValueOnce({
      status: "rejected",
      error: {
        code: "invalid_settings",
        message: "Invalid settings",
        issues: [],
      },
    });

    const result = await saveSettings(createSettingsForTest());

    expect(result).toMatchObject({
      status: "rejected",
      error: { code: "invalid_settings", issues: [] },
    });
    expect(
      useTimerOperationStore.getState().operations.update_settings,
    ).toBeDefined();
  });
});
