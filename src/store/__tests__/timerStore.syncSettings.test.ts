import { beforeEach, describe, expect, it, vi } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { SessionType, TimerStatus, useTimerStore } from "../timerStore";
import { useSettingsStore } from "../settingsStore";

describe("timerStore syncSettings command routing", () => {
  beforeEach(() => {
    vi.clearAllMocks();

    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      isFullscreenBreak: false,
      showContinueDialog: false,
      isConnected: false,
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });

    useSettingsStore.getState().resetToDefaults();
    useSettingsStore.getState().setAutoRunFeatureEnabled(true);
    useSettingsStore.getState().setDayAutoRunEnabled("mon", true);
    useSettingsStore.getState().setDayStartTime("mon", "08:00");
    useSettingsStore.getState().setDayEndTime("mon", "17:00");
  });

  it("should invoke sync_all_settings while idle", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).toHaveBeenCalledWith(
      "sync_all_settings",
      expect.objectContaining({
        workDurationSecs: expect.any(Number),
        shortBreakDurationSecs: expect.any(Number),
        longBreakDurationSecs: expect.any(Number),
        longBreakInterval: expect.any(Number),
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: expect.objectContaining({
          mon: expect.objectContaining({
            enabled: true,
            startTime: "08:00",
            endTime: "17:00",
          }),
        }),
      })
    );
  });

  it("should invoke sync_auto_run_settings while running", async () => {
    useTimerStore.setState({ status: TimerStatus.WORKING, isConnected: true });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).toHaveBeenCalledWith(
      "sync_auto_run_settings",
      expect.objectContaining({
        autoRunFeatureEnabled: true,
        weeklyAutoRunConfig: expect.objectContaining({
          mon: expect.objectContaining({
            enabled: true,
            startTime: "08:00",
            endTime: "17:00",
          }),
        }),
      })
    );

    const commandPayload = mockInvoke.mock.calls[0][1] as Record<string, unknown>;
    expect(commandPayload.workDurationSecs).toBeUndefined();
    expect(commandPayload.shortBreakDurationSecs).toBeUndefined();
    expect(commandPayload.longBreakDurationSecs).toBeUndefined();
    expect(commandPayload.longBreakInterval).toBeUndefined();
  });

  it("should swallow backend errors during sync", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce(new Error("sync failed"));

    await expect(useTimerStore.getState().syncSettings()).resolves.toBeUndefined();
  });
});
