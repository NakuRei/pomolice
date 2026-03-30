import { beforeEach, describe, expect, it, vi } from "vitest";

const { mockSettingsSyncLogger } = vi.hoisted(() => ({
  mockSettingsSyncLogger: {
    trace: vi.fn(),
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

vi.mock("@/logging/logger", () => ({
  createLogger: (): typeof mockSettingsSyncLogger => mockSettingsSyncLogger,
}));

import { SessionType, TimerStatus, useTimerStore } from "./timerStore";
import { useSettingsStore } from "./settingsStore";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";

function expectRecord(
  value: unknown,
  description: string,
): asserts value is Record<string, unknown> {
  expect(value).not.toBeNull();
  expect(typeof value).toBe("object");
  if (value === null || typeof value !== "object") {
    throw new Error(`Expected ${description} to be an object`);
  }
}

function getInvokeCall(callIndex: number): {
  command: string;
  payload: Record<string, unknown> | undefined;
} {
  const invokeCall: (typeof mockInvoke.mock.calls)[number] | undefined =
    mockInvoke.mock.calls[callIndex] as
      | (typeof mockInvoke.mock.calls)[number]
      | undefined;
  expect(invokeCall).toBeDefined();
  if (invokeCall === undefined) {
    throw new Error(`Expected invoke call at index ${String(callIndex)}`);
  }

  const [command, payload] = invokeCall;
  return { command, payload };
}

function getLastInvokeCallByCommand(commandName: string): {
  command: string;
  payload: Record<string, unknown> | undefined;
} {
  const invokeCall = [...mockInvoke.mock.calls]
    .reverse()
    .find(([invokedCommand]) => invokedCommand === commandName);
  expect(invokeCall).toBeDefined();
  if (!invokeCall) {
    throw new Error(`Expected invoke command "${commandName}"`);
  }

  const [command, payload] = invokeCall;
  return { command, payload };
}

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
      requiresManualContinueDecision: false,
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

  it("should invoke duration and automation sync commands while idle", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    await useTimerStore.getState().syncSettings();

    const firstInvokeCall = getInvokeCall(0);
    expect(firstInvokeCall.command).toBe("sync_timer_duration_settings");
    expectRecord(
      firstInvokeCall.payload,
      "sync_timer_duration_settings payload",
    );
    expect(typeof firstInvokeCall.payload.workDurationSecs).toBe("number");
    expect(typeof firstInvokeCall.payload.shortBreakDurationSecs).toBe(
      "number",
    );
    expect(typeof firstInvokeCall.payload.longBreakDurationSecs).toBe("number");
    expect(typeof firstInvokeCall.payload.longBreakInterval).toBe("number");

    const secondInvokeCall = getInvokeCall(1);
    expect(secondInvokeCall.command).toBe("sync_automation_policies");
    expectRecord(secondInvokeCall.payload, "sync_automation_policies payload");
    expect(secondInvokeCall.payload.postBreakTransitionPolicy).toBe(
      "require_manual_continue",
    );
    expect(secondInvokeCall.payload.autoRunFeatureEnabled).toBe(true);
    expectRecord(
      secondInvokeCall.payload.weeklyAutoRunConfig,
      "weeklyAutoRunConfig",
    );
    const mondayAutoRunConfig =
      secondInvokeCall.payload.weeklyAutoRunConfig.mon;
    expectRecord(mondayAutoRunConfig, "monday auto-run config");
    expect(mondayAutoRunConfig.enabled).toBe(true);
    expect(mondayAutoRunConfig.startTime).toBe("08:00");
    expect(mondayAutoRunConfig.endTime).toBe("17:00");
  });

  it("should round decimal durations and interval before invoking start_work", async () => {
    useSettingsStore.setState({
      workDuration: 0.01,
      shortBreakDuration: 0.02,
      longBreakDuration: 0.03,
      longBreakInterval: 4.6,
    });

    await useTimerStore.getState().start();

    const { command, payload } = getInvokeCall(0);
    expect(command).toBe("start_work");
    expectRecord(payload, "start_work payload");
    expect(payload.workDurationSecs).toBe(1);
    expect(payload.shortBreakDurationSecs).toBe(1);
    expect(payload.longBreakDurationSecs).toBe(2);
    expect(payload.longBreakInterval).toBe(5);
  });

  it("should round decimal durations and interval before syncing timer settings", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    useSettingsStore.setState({
      workDuration: 0.01,
      shortBreakDuration: 0.02,
      longBreakDuration: 0.03,
      longBreakInterval: 4.6,
    });

    await useTimerStore.getState().syncSettings();

    const firstInvokeCall = getInvokeCall(0);
    expect(firstInvokeCall.command).toBe("sync_timer_duration_settings");
    expectRecord(
      firstInvokeCall.payload,
      "sync_timer_duration_settings payload",
    );
    expect(firstInvokeCall.payload.workDurationSecs).toBe(1);
    expect(firstInvokeCall.payload.shortBreakDurationSecs).toBe(1);
    expect(firstInvokeCall.payload.longBreakDurationSecs).toBe(2);
    expect(firstInvokeCall.payload.longBreakInterval).toBe(5);
  });

  it("should invoke automation sync command only while running", async () => {
    useTimerStore.setState({ status: TimerStatus.WORKING, isConnected: true });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    const { command, payload: commandPayload } = getInvokeCall(0);
    expect(command).toBe("sync_automation_policies");
    expectRecord(commandPayload, "sync_automation_policies payload");
    expect(commandPayload.workDurationSecs).toBeUndefined();
    expect(commandPayload.shortBreakDurationSecs).toBeUndefined();
    expect(commandPayload.longBreakDurationSecs).toBeUndefined();
    expect(commandPayload.longBreakInterval).toBeUndefined();
    expect(commandPayload.postBreakTransitionPolicy).toBe(
      "require_manual_continue",
    );
    expect(commandPayload.autoRunFeatureEnabled).toBe(true);
    expectRecord(commandPayload.weeklyAutoRunConfig, "weeklyAutoRunConfig");
    const mondayAutoRunConfig = commandPayload.weeklyAutoRunConfig.mon;
    expectRecord(mondayAutoRunConfig, "monday auto-run config");
    expect(mondayAutoRunConfig.enabled).toBe(true);
    expect(mondayAutoRunConfig.startTime).toBe("08:00");
    expect(mondayAutoRunConfig.endTime).toBe("17:00");
  });

  it("should sync auto post-break transition policy value to automation command", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    useSettingsStore
      .getState()
      .setPostBreakTransitionPolicy("start_next_work_automatically");

    await useTimerStore.getState().syncSettings();

    const { command, payload } = getLastInvokeCallByCommand(
      "sync_automation_policies",
    );
    expect(command).toBe("sync_automation_policies");
    expectRecord(payload, "sync_automation_policies payload");
    expect(payload.postBreakTransitionPolicy).toBe(
      "start_next_work_automatically",
    );
  });

  it("should swallow backend errors during sync", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce(new Error("sync failed"));

    await expect(
      useTimerStore.getState().syncSettings(),
    ).resolves.toBeUndefined();
    expect(mockSettingsSyncLogger.warn).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.settingsSyncFailed,
      expect.objectContaining({
        failure_message: "Error: sync failed",
        timer_status: TimerStatus.IDLE,
      }),
    );
  });
});
