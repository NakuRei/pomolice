import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetSettingsStoreForTest } from "@/test/resetSettingsStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

interface TestLogger {
  debug: () => void;
  error: () => void;
  info: () => void;
  trace: () => void;
  warn: () => void;
}

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

vi.mock("../logging/logger", () => ({
  createLogger: (): TestLogger => ({
    debug: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    trace: vi.fn(),
    warn: vi.fn(),
  }),
}));

import { TimerStatus, useTimerStore } from "./timerStore";
import {
  WEEKDAY_KEYS,
  createAutomationSettingsCandidate,
  createDurationSettingsCandidate,
  useSettingsStore,
  type SettingsApplicationStatus,
} from "./settingsStore";

interface DeferredPromise<T> {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason: unknown) => void;
}

function createDeferredPromise<T>(): DeferredPromise<T> {
  let resolvePromise: ((value: T) => void) | undefined = undefined;
  let rejectPromise: ((reason: unknown) => void) | undefined = undefined;
  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });

  return {
    promise,
    resolve: (value: T): void => {
      if (!resolvePromise) {
        throw new Error("Expected deferred resolve function");
      }
      resolvePromise(value);
    },
    reject: (reason: unknown): void => {
      if (!rejectPromise) {
        throw new Error("Expected deferred reject function");
      }
      rejectPromise(reason);
    },
  };
}

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
    mockInvoke.mock.calls[callIndex];
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

async function waitForInvokeCount(expectedCount: number): Promise<void> {
  await vi.waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledTimes(expectedCount);
  });
}

function setApplicationStatuses(
  durationStatus: SettingsApplicationStatus,
  automationStatus: SettingsApplicationStatus,
): void {
  useSettingsStore.setState({
    durationSettingsApplicationStatus: durationStatus,
    automationSettingsApplicationStatus: automationStatus,
  });
}

function confirmCurrentDurationCandidate(): void {
  const state = useSettingsStore.getState();
  state.confirmDurationSettingsCandidate(
    createDurationSettingsCandidate(state),
  );
}

function confirmCurrentAutomationCandidate(): void {
  const state = useSettingsStore.getState();
  state.confirmAutomationSettingsCandidate(
    createAutomationSettingsCandidate(state),
  );
}

describe("timerStore syncSettings command routing", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(undefined);

    resetTimerStoreForTest();
    resetSettingsStoreForTest();
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
    expect(firstInvokeCall.payload.workDurationSecs).toBe(1500);
    expect(firstInvokeCall.payload.shortBreakDurationSecs).toBe(300);
    expect(firstInvokeCall.payload.longBreakDurationSecs).toBe(900);
    expect(firstInvokeCall.payload.longBreakInterval).toBe(4);

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
    setApplicationStatuses("pending", "unconfirmed");
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
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "pending",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("unconfirmed");
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

  it.each([TimerStatus.WORKING, TimerStatus.ON_BREAK])(
    "invokes only automation while the timer is %s",
    async (timerStatus) => {
      useTimerStore.setState({ status: timerStatus, isConnected: true });

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
      const settings = useSettingsStore.getState();
      expect(settings.durationSettingsApplicationStatus).toBe("pending");
      expect(settings.automationSettingsApplicationStatus).toBe("confirmed");
    },
  );

  it("should sync auto post-break transition policy value to automation command", async () => {
    useSettingsStore
      .getState()
      .setPostBreakTransitionPolicy("start_next_work_automatically");
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

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

  it("should swallow raw backend errors during sync", async () => {
    confirmCurrentAutomationCandidate();
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce(new Error("sync failed"));

    await expect(
      useTimerStore.getState().syncSettings(),
    ).resolves.toBeUndefined();
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "unconfirmed",
    );
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });

  it("should swallow structured backend errors during sync", async () => {
    confirmCurrentAutomationCandidate();
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce({
      code: "persistence_error",
      message: "failed to save timer settings",
    });

    await expect(
      useTimerStore.getState().syncSettings(),
    ).resolves.toBeUndefined();
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "unconfirmed",
    );
    expect(mockInvoke).toHaveBeenCalledTimes(1);
  });

  it.each([
    {
      label: "duration",
      durationStatus: "pending" as const,
      automationStatus: "confirmed" as const,
      expectedCommand: "sync_timer_duration_settings",
    },
    {
      label: "automation",
      durationStatus: "confirmed" as const,
      automationStatus: "unconfirmed" as const,
      expectedCommand: "sync_automation_policies",
    },
  ])(
    "omits the confirmed group when only $label requires synchronization",
    async ({ durationStatus, automationStatus, expectedCommand }) => {
      setApplicationStatuses(durationStatus, automationStatus);
      useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

      await useTimerStore.getState().syncSettings();

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(getInvokeCall(0).command).toBe(expectedCommand);
    },
  );

  it("does not write either group when both current candidates are confirmed", async () => {
    setApplicationStatuses("confirmed", "confirmed");
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it("fails fast after duration rejection and leaves automation unresolved", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce(new Error("duration failed"));

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    expect(getInvokeCall(0).command).toBe("sync_timer_duration_settings");
    const settings = useSettingsStore.getState();
    expect(settings.durationSettingsApplicationStatus).toBe("unconfirmed");
    expect(settings.automationSettingsApplicationStatus).toBe("pending");
  });

  it("keeps duration confirmed when automation rejects later in the pass", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockResolvedValueOnce(undefined).mockRejectedValueOnce({
      code: "persistence_error",
      message: "automation persistence failed",
    });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke.mock.calls.map(([command]) => command)).toEqual([
      "sync_timer_duration_settings",
      "sync_automation_policies",
    ]);
    const settings = useSettingsStore.getState();
    expect(settings.durationSettingsApplicationStatus).toBe("confirmed");
    expect(settings.automationSettingsApplicationStatus).toBe("unconfirmed");
  });

  it("allows an explicit retry of the same unconfirmed candidate", async () => {
    confirmCurrentAutomationCandidate();
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    mockInvoke.mockRejectedValueOnce(new Error("uncertain duration write"));

    await useTimerStore.getState().syncSettings();
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "unconfirmed",
    );

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).toHaveBeenCalledTimes(2);
    expect(mockInvoke.mock.calls.map(([command]) => command)).toEqual([
      "sync_timer_duration_settings",
      "sync_timer_duration_settings",
    ]);
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
  });

  it.each([
    {
      label: "duration",
      durationStatus: "pending" as const,
      automationStatus: "confirmed" as const,
      statusKey: "durationSettingsApplicationStatus" as const,
      expectedCommand: "sync_timer_duration_settings",
    },
    {
      label: "automation",
      durationStatus: "confirmed" as const,
      automationStatus: "pending" as const,
      statusKey: "automationSettingsApplicationStatus" as const,
      expectedCommand: "sync_automation_policies",
    },
  ])(
    "keeps the same $label candidate unconfirmed after two rejected attempts",
    async ({
      durationStatus,
      automationStatus,
      statusKey,
      expectedCommand,
    }) => {
      setApplicationStatuses(durationStatus, automationStatus);
      useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
      mockInvoke.mockRejectedValue(new Error("settings write failed"));

      await useTimerStore.getState().syncSettings();
      expect(useSettingsStore.getState()[statusKey]).toBe("unconfirmed");

      await useTimerStore.getState().syncSettings();
      expect(useSettingsStore.getState()[statusKey]).toBe("unconfirmed");
      expect(mockInvoke.mock.calls.map(([command]) => command)).toEqual([
        expectedCommand,
        expectedCommand,
      ]);
    },
  );

  it("does not retry a rejected candidate without another trigger", async () => {
    vi.useFakeTimers();
    try {
      confirmCurrentAutomationCandidate();
      useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
      mockInvoke.mockRejectedValueOnce(new Error("uncertain duration write"));

      await useTimerStore.getState().syncSettings();
      await vi.runOnlyPendingTimersAsync();

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("unconfirmed");
    } finally {
      vi.useRealTimers();
    }
  });

  it("exposes the synchronization lane only while a command is unsettled", async () => {
    confirmCurrentAutomationCandidate();
    const durationCommand = createDeferredPromise<unknown>();
    mockInvoke.mockReturnValueOnce(durationCommand.promise);
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    const synchronization = useTimerStore.getState().syncSettings();

    expect(useTimerStore.getState().isSettingsSynchronizationInFlight).toBe(
      true,
    );
    durationCommand.resolve(undefined);
    await synchronization;

    expect(useTimerStore.getState().isSettingsSynchronizationInFlight).toBe(
      false,
    );
  });

  it("never overlaps deferred commands across repeated follow-ups", async () => {
    confirmCurrentAutomationCandidate();
    const firstDurationCommand = createDeferredPromise<unknown>();
    const secondDurationCommand = createDeferredPromise<unknown>();
    const finalDurationCommand = createDeferredPromise<unknown>();
    let activeCommandCount = 0;
    let maximumActiveCommandCount = 0;
    const deferredCommands = [
      firstDurationCommand,
      secondDurationCommand,
      finalDurationCommand,
    ];
    mockInvoke.mockImplementation(() => {
      const deferredCommand = deferredCommands.shift();
      if (!deferredCommand) {
        throw new Error("Unexpected extra duration command");
      }
      activeCommandCount += 1;
      maximumActiveCommandCount = Math.max(
        maximumActiveCommandCount,
        activeCommandCount,
      );
      return deferredCommand.promise.finally(() => {
        activeCommandCount -= 1;
      });
    });
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    const synchronization = useTimerStore.getState().syncSettings();
    useSettingsStore.getState().setWorkDuration(40);

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    firstDurationCommand.resolve(undefined);
    await waitForInvokeCount(2);
    useSettingsStore.getState().setWorkDuration(50);
    secondDurationCommand.resolve(undefined);
    await waitForInvokeCount(3);
    expect(maximumActiveCommandCount).toBe(1);

    finalDurationCommand.resolve(undefined);
    await synchronization;

    expect(maximumActiveCommandCount).toBe(1);
    expect(activeCommandCount).toBe(0);
    expect(
      mockInvoke.mock.calls.map(([, payload]) => payload?.workDurationSecs),
    ).toEqual([1500, 2400, 3000]);
  });

  it("uses immutable first-pass payloads and fresh candidates in the follow-up", async () => {
    const firstDurationCommand = createDeferredPromise<unknown>();
    mockInvoke.mockReturnValueOnce(firstDurationCommand.promise);
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    const synchronization = useTimerStore.getState().syncSettings();
    useSettingsStore.getState().setWorkDuration(40);
    for (const weekday of WEEKDAY_KEYS) {
      useSettingsStore
        .getState()
        .setDayAutoRunEnabled(weekday, weekday !== "mon");
      useSettingsStore.getState().setDayStartTime(weekday, "10:00");
      useSettingsStore.getState().setDayEndTime(weekday, "19:00");
    }

    firstDurationCommand.resolve(undefined);
    await synchronization;

    expect(mockInvoke.mock.calls.map(([command]) => command)).toEqual([
      "sync_timer_duration_settings",
      "sync_automation_policies",
      "sync_timer_duration_settings",
      "sync_automation_policies",
    ]);
    const firstDurationPayload = getInvokeCall(0).payload;
    expectRecord(firstDurationPayload, "first duration payload");
    expect(firstDurationPayload.workDurationSecs).toBe(1500);
    const firstAutomationPayload = getInvokeCall(1).payload;
    expectRecord(firstAutomationPayload, "first automation payload");
    expectRecord(
      firstAutomationPayload.weeklyAutoRunConfig,
      "first weekly auto-run config",
    );
    for (const weekday of WEEKDAY_KEYS) {
      const firstWeekdaySlot =
        firstAutomationPayload.weeklyAutoRunConfig[weekday];
      expectRecord(firstWeekdaySlot, `first ${weekday} auto-run slot`);
      expect(firstWeekdaySlot).toEqual(
        weekday === "mon"
          ? { enabled: true, startTime: "08:00", endTime: "17:00" }
          : { enabled: false, startTime: "09:00", endTime: "18:00" },
      );
    }

    const latestDurationPayload = getInvokeCall(2).payload;
    expectRecord(latestDurationPayload, "latest duration payload");
    expect(latestDurationPayload.workDurationSecs).toBe(2400);
    const latestAutomationPayload = getInvokeCall(3).payload;
    expectRecord(latestAutomationPayload, "latest automation payload");
    expectRecord(
      latestAutomationPayload.weeklyAutoRunConfig,
      "latest weekly auto-run config",
    );
    for (const weekday of WEEKDAY_KEYS) {
      const latestWeekdaySlot =
        latestAutomationPayload.weeklyAutoRunConfig[weekday];
      expectRecord(latestWeekdaySlot, `latest ${weekday} auto-run slot`);
      expect(latestWeekdaySlot).toEqual({
        enabled: weekday !== "mon",
        startTime: "10:00",
        endTime: "19:00",
      });
    }
  });

  it("coalesces multiple edits into one pass for the latest candidate", async () => {
    confirmCurrentAutomationCandidate();
    const firstDurationCommand = createDeferredPromise<unknown>();
    mockInvoke.mockReturnValueOnce(firstDurationCommand.promise);
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    const synchronization = useTimerStore.getState().syncSettings();
    useSettingsStore.getState().setWorkDuration(30);
    useSettingsStore.getState().setWorkDuration(35);
    useSettingsStore.getState().setWorkDuration(40);

    expect(mockInvoke).toHaveBeenCalledTimes(1);
    firstDurationCommand.resolve(undefined);
    await synchronization;

    expect(mockInvoke).toHaveBeenCalledTimes(2);
    const firstPayload = getInvokeCall(0).payload;
    const latestPayload = getInvokeCall(1).payload;
    expectRecord(firstPayload, "first duration payload");
    expectRecord(latestPayload, "latest duration payload");
    expect(firstPayload.workDurationSecs).toBe(1500);
    expect(latestPayload.workDurationSecs).toBe(2400);
  });

  it("synchronizes an edit queued as the previous write finishes", async () => {
    confirmCurrentAutomationCandidate();
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
    const unsubscribe = useSettingsStore.subscribe((settings) => {
      if (
        settings.workDuration === 25 &&
        settings.durationSettingsApplicationStatus === "confirmed"
      ) {
        void Promise.resolve()
          .then(() => undefined)
          .then(() => {
            useSettingsStore.getState().setWorkDuration(40);
          });
      }
    });

    try {
      await useTimerStore.getState().syncSettings();

      await vi.waitFor(() => {
        expect(
          mockInvoke.mock.calls.map(([, payload]) => payload?.workDurationSecs),
        ).toEqual([1500, 2400]);
        expect(
          useSettingsStore.getState().durationSettingsApplicationStatus,
        ).toBe("confirmed");
      });
    } finally {
      unsubscribe();
    }
  });

  it.each(["success", "failure"] as const)(
    "keeps a newer candidate pending after stale %s until its own acknowledgement",
    async (staleOutcome) => {
      confirmCurrentAutomationCandidate();
      const staleDurationCommand = createDeferredPromise<unknown>();
      const latestDurationCommand = createDeferredPromise<unknown>();
      mockInvoke
        .mockReturnValueOnce(staleDurationCommand.promise)
        .mockReturnValueOnce(latestDurationCommand.promise);
      useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

      const synchronization = useTimerStore.getState().syncSettings();
      useSettingsStore.getState().setWorkDuration(40);

      if (staleOutcome === "success") {
        staleDurationCommand.resolve(undefined);
      } else {
        staleDurationCommand.reject(new Error("stale duration failure"));
      }
      await waitForInvokeCount(2);

      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("pending");
      latestDurationCommand.resolve(undefined);
      await synchronization;
      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("confirmed");
    },
  );

  it.each(["success", "failure"] as const)(
    "keeps newer automation pending after stale %s until its own acknowledgement",
    async (staleOutcome) => {
      confirmCurrentDurationCandidate();
      const staleAutomationCommand = createDeferredPromise<unknown>();
      const latestAutomationCommand = createDeferredPromise<unknown>();
      mockInvoke
        .mockReturnValueOnce(staleAutomationCommand.promise)
        .mockReturnValueOnce(latestAutomationCommand.promise);
      useTimerStore.setState({
        status: TimerStatus.WORKING,
        isConnected: true,
      });

      const synchronization = useTimerStore.getState().syncSettings();
      useSettingsStore.getState().setAutoRunFeatureEnabled(false);

      if (staleOutcome === "success") {
        staleAutomationCommand.resolve(undefined);
      } else {
        staleAutomationCommand.reject(new Error("stale automation failure"));
      }
      await waitForInvokeCount(2);

      expect(
        useSettingsStore.getState().automationSettingsApplicationStatus,
      ).toBe("pending");
      latestAutomationCommand.resolve(undefined);
      await synchronization;
      expect(
        useSettingsStore.getState().automationSettingsApplicationStatus,
      ).toBe("confirmed");
    },
  );

  it("does not synchronize while the backend connection is unavailable", async () => {
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: false });

    await useTimerStore.getState().syncSettings();

    expect(mockInvoke).not.toHaveBeenCalled();
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "pending",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("pending");
  });

  it("requests synchronization when a backend-managed candidate changes", async () => {
    setApplicationStatuses("confirmed", "confirmed");
    useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });

    useSettingsStore.getState().setWorkDuration(40);

    await waitForInvokeCount(1);
    await vi.waitFor(() => {
      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("confirmed");
    });
    expect(getInvokeCall(0).command).toBe("sync_timer_duration_settings");
  });

  it("does not self-trigger from status acknowledgements or frontend-only edits", async () => {
    vi.useFakeTimers();
    try {
      useTimerStore.setState({ status: TimerStatus.IDLE, isConnected: true });
      confirmCurrentDurationCandidate();
      confirmCurrentAutomationCandidate();
      useSettingsStore.getState().setNotificationSoundVolumePercent(75);
      useSettingsStore.getState().toggleNotificationSoundMuted();

      await vi.runOnlyPendingTimersAsync();

      expect(mockInvoke).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });
});
