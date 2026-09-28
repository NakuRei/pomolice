import {
  useTimerEffectsStore,
  selectBreakLockObservation,
} from "./timerEffectsStore";
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

import { useErrorStore } from "./errorStore";
import { useTimerStore } from "./timerStore";
import { SessionType, TimerStatus } from "@/types/timer";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

describe("timerStore settings saving", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    useErrorStore.getState().clearAllErrors();
    resetTimerStoreForTest();
  });

  describe("saveSettings()", () => {
    it("sends only the changed settings", async () => {
      mockInvoke.mockResolvedValueOnce(createTimerSnapshotForTest());

      const result = await useTimerStore
        .getState()
        .saveSettings({ workDurationSecs: 30 * 60 });

      expect(result).toEqual({ status: "saved" });
      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: { work_duration_secs: 30 * 60 },
      });
    });

    it("leaves every piece of state to the events, so a late reply cannot roll it back", async () => {
      const settings = createSettingsForTest();
      useTimerStore.setState({
        settings,
        status: TimerStatus.ON_BREAK,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 299,
      });
      useTimerEffectsStore.getState().observe({
        revision: 2,
        last_timer_save: null,
        break_lock: {
          target: "locked",
          last_attempt: { action: "enter", status: "succeeded" },
        },
      });
      const stateBeforeTheReply = useTimerStore.getState();
      mockInvoke.mockResolvedValueOnce(createTimerSnapshotForTest());

      await useTimerStore.getState().saveSettings({ workDurationSecs: 60 });

      const state = useTimerStore.getState();
      expect(state.settings).toBe(stateBeforeTheReply.settings);
      expect(state.status).toBe(TimerStatus.ON_BREAK);
      expect(state.remainingSeconds).toBe(299);
      expect(selectBreakLockObservation(useTimerEffectsStore.getState())).toBe(
        "locked",
      );
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
      mockInvoke.mockRejectedValueOnce({
        code: "invalid_settings",
        message: "Invalid settings",
        issues,
      });

      const result = await useTimerStore
        .getState()
        .saveSettings(createSettingsForTest());

      expect(result).toEqual({ status: "rejected", issues });
      expect(useTimerStore.getState().settings).toBeNull();
      expect(useErrorStore.getState().errors.update_settings).toBeUndefined();
    });

    it("reports any other failure and keeps the previous settings", async () => {
      const previousSettings = createSettingsForTest();
      useTimerStore.setState({ settings: previousSettings });
      mockInvoke.mockRejectedValueOnce({
        code: "persistence_error",
        message: "Persistence error",
      });

      const result = await useTimerStore
        .getState()
        .saveSettings(createSettingsForTest({ workDurationSecs: 60 }));

      expect(result).toEqual({ status: "failed" });
      expect(useTimerStore.getState().settings).toEqual(previousSettings);
      expect(useErrorStore.getState().errors.update_settings).toMatchObject({
        id: "update_settings",
        origin: "backend_command",
        canRetry: true,
      });
    });

    it("clears a previous save failure once a save succeeds", async () => {
      const settings = createSettingsForTest();
      mockInvoke.mockRejectedValueOnce({
        code: "persistence_error",
        message: "Persistence error",
      });
      await useTimerStore.getState().saveSettings(settings);
      mockInvoke.mockResolvedValueOnce(createTimerSnapshotForTest());

      await useTimerStore.getState().saveSettings(settings);

      expect(useErrorStore.getState().errors.update_settings).toBeUndefined();
    });
  });

  describe("toggleNotificationSoundMuted()", () => {
    it("saves the settings with the mute flag flipped", async () => {
      useTimerStore.setState({
        settings: createSettingsForTest({ notificationSoundMuted: false }),
      });
      mockInvoke.mockResolvedValueOnce(createTimerSnapshotForTest());

      await useTimerStore.getState().toggleNotificationSoundMuted();

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: { notification_sound_muted: true },
      });
    });

    it("does nothing before the backend has sent the settings", async () => {
      await useTimerStore.getState().toggleNotificationSoundMuted();

      expect(mockInvoke).not.toHaveBeenCalled();
    });
  });

  describe("savePostBreakTransitionPolicy()", () => {
    it("saves the settings with the given policy", async () => {
      useTimerStore.setState({
        settings: createSettingsForTest({
          postBreakTransitionPolicy: "require_manual_continue",
        }),
      });
      mockInvoke.mockResolvedValueOnce(createTimerSnapshotForTest());

      await useTimerStore
        .getState()
        .savePostBreakTransitionPolicy("start_next_work_automatically");

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: {
          post_break_transition_policy: "start_next_work_automatically",
        },
      });
    });
  });

  describe("handleSettingsChanged()", () => {
    it("takes the settings and file status the backend announces", () => {
      const settings = createSettingsForTest({ notificationSoundMuted: true });

      useTimerStore.getState().handleSettingsChanged({
        settings: settingsPayloadForTest(settings),
        settings_file_status: "ok",
      });

      expect(useTimerStore.getState().settings).toEqual(settings);
      expect(useTimerStore.getState().settingsFileStatus).toBe("ok");
    });
  });

  it("treats a rejection that names no input as an ordinary failure", async () => {
    mockInvoke.mockRejectedValueOnce({
      code: "invalid_settings",
      message: "Invalid settings",
      issues: [],
    });

    const result = await useTimerStore
      .getState()
      .saveSettings(createSettingsForTest());

    expect(result).toEqual({ status: "failed" });
    expect(useErrorStore.getState().errors.update_settings).toBeDefined();
  });
});
