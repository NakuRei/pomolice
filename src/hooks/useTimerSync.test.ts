/**
 * useTimerSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify state synchronization and connection recovery.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import {
  isPermissionGranted,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { resetSettingsStoreForTest } from "@/test/resetSettingsStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import {
  useTimerStore,
  TimerStatus,
  SessionType,
  type FullStatePayload,
  type SessionCompletedPayload,
  type TimerTickPayload,
} from "@/store/timerStore";
import { useErrorStore } from "@/store/errorStore";
import {
  useSettingsStore,
  type SettingsApplicationStatus,
} from "@/store/settingsStore";
import {
  TIMER_SYNC_MAX_CONNECTION_RETRIES,
  timerSyncRetryDelayMs,
} from "./timerSyncConnectionRetryPolicy";

// Mock Tauri APIs
type TimerEventHandler = (event: { payload: unknown }) => void;
type ListenerRegistration = (
  event: string,
  handler: TimerEventHandler | (() => void),
) => Promise<() => void>;
type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockListen = vi.fn<ListenerRegistration>();
const mockInvoke = vi.fn<InvokeCommand>();
const noopUnlisten = (): void => undefined;

vi.mock("@tauri-apps/api/event", () => ({
  listen: (
    event: string,
    handler: TimerEventHandler | (() => void),
  ): Promise<() => void> => mockListen(event, handler),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  trace: vi.fn().mockResolvedValue(undefined),
  debug: vi.fn().mockResolvedValue(undefined),
  info: vi.fn().mockResolvedValue(undefined),
  warn: vi.fn().mockResolvedValue(undefined),
  error: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn((path: string) => Promise.resolve(`/app/${path}`)),
}));

// Import after mocking
import { useTimerSync } from "./useTimerSync";

type FullStateOverrides = Omit<Partial<FullStatePayload>, "settings"> & {
  settings?: Partial<FullStatePayload["settings"]>;
};

function createFullStatePayload(
  overrides: FullStateOverrides = {},
): FullStatePayload {
  const { settings, ...stateOverrides } = overrides;
  return {
    status: TimerStatus.IDLE,
    remaining_seconds: 1500,
    session_type: SessionType.WORK,
    current_cycle: 0,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    is_fullscreen_break: false,
    away_during_break: false,
    post_break_decision_pending: false,
    boot_auto_start_prompt_window: null,
    active_window_summary: null,
    ...stateOverrides,
    settings: {
      work_duration_secs: 1500,
      short_break_duration_secs: 300,
      long_break_duration_secs: 900,
      long_break_interval: 4,
      post_break_transition_policy: "require_manual_continue",
      ...settings,
    },
  };
}

function renderTimerSync(): ReturnType<typeof renderHook> {
  return renderHook(() => {
    useTimerSync();
  });
}

function getRequiredTimerEventHandler(
  handler: TimerEventHandler | undefined,
  description: string,
): TimerEventHandler {
  if (!handler) {
    throw new Error(`Expected ${description} to be defined`);
  }

  return handler;
}

async function renderAndGetTimerEventHandler(
  eventName: string,
): Promise<TimerEventHandler> {
  let capturedHandler: TimerEventHandler | undefined = undefined;
  mockListen.mockImplementation((event, handler) => {
    if (event === eventName) {
      capturedHandler = handler;
    }
    return Promise.resolve(noopUnlisten);
  });

  renderTimerSync();
  await vi.waitFor(() => {
    expect(capturedHandler).toBeDefined();
  });
  return getRequiredTimerEventHandler(capturedHandler, `${eventName} handler`);
}

function emitTimerTick(
  handler: TimerEventHandler,
  overrides: Partial<TimerTickPayload> = {},
): void {
  handler({
    payload: {
      status: TimerStatus.WORKING,
      remaining_seconds: 1499,
      session_type: SessionType.WORK,
      current_cycle: 1,
      completed_pomodoros: 0,
      break_interaction_count: 0,
      is_fullscreen_break: false,
      away_during_break: false,
      ...overrides,
    },
  });
}

function emitSessionCompleted(
  handler: TimerEventHandler,
  overrides: Partial<SessionCompletedPayload> = {},
): void {
  handler({
    payload: {
      transition_reason: "break_waiting_decision",
      completed_session_type: SessionType.SHORT_BREAK,
      next_status: TimerStatus.IDLE,
      next_session_type: SessionType.WORK,
      requires_manual_continue_decision: true,
      post_break_decision_pending: true,
      ...overrides,
    },
  });
}

async function flushTimerSyncSetup(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
}

function timerSyncConnectionRetryAttempts(): number[] {
  return Array.from(
    { length: TIMER_SYNC_MAX_CONNECTION_RETRIES },
    (_unused, retryAttempt) => retryAttempt,
  );
}

async function advanceThroughTimerSyncConnectionRetries(): Promise<void> {
  await timerSyncConnectionRetryAttempts().reduce<Promise<void>>(
    async (previousRetry, retryAttempt) => {
      await previousRetry;
      await vi.advanceTimersByTimeAsync(timerSyncRetryDelayMs(retryAttempt));
    },
    Promise.resolve(),
  );
}

function getFullStateInvokeCount(): number {
  return mockInvoke.mock.calls.filter(
    ([command]) => command === "get_full_state",
  ).length;
}

function settingsCommandNames(): string[] {
  return mockInvoke.mock.calls
    .map(([command]) => command)
    .filter(
      (command) =>
        command === "sync_timer_duration_settings" ||
        command === "sync_automation_policies",
    );
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

function confirmCurrentCandidates(): void {
  const state = useSettingsStore.getState();
  state.confirmDurationSettingsCandidate(state);
  state.confirmAutomationSettingsCandidate(state);
}

describe("useTimerSync", () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    useErrorStore.getState().clearAllErrors();

    resetTimerStoreForTest();
    resetSettingsStoreForTest();

    // Default mock implementations
    mockListen.mockResolvedValue(noopUnlisten);
    mockInvoke.mockResolvedValue(createFullStatePayload());
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("starts backend setup from the settled synchronous hydration result", async () => {
    vi.useFakeTimers();
    vi.spyOn(useSettingsStore.persist, "hasHydrated").mockReturnValue(true);
    renderTimerSync();
    await flushTimerSyncSetup();

    expect(useTimerStore.getState().isConnected).toBe(true);
    expect(settingsCommandNames()).toEqual([
      "sync_timer_duration_settings",
      "sync_automation_policies",
    ]);
  });

  it("keeps timer use available without automatically sending settings after terminal hydration failure", async () => {
    vi.useFakeTimers();
    vi.spyOn(useSettingsStore.persist, "hasHydrated").mockReturnValue(false);
    renderTimerSync();
    await flushTimerSyncSetup();

    expect(useTimerStore.getState().isConnected).toBe(true);
    expect(settingsCommandNames()).toEqual([]);
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "pending",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("pending");
  });

  it("allows explicit settings recovery after terminal hydration failure", async () => {
    vi.useFakeTimers();
    vi.spyOn(useSettingsStore.persist, "hasHydrated").mockReturnValue(false);
    renderTimerSync();
    await flushTimerSyncSetup();
    expect(settingsCommandNames()).toEqual([]);

    await useTimerStore.getState().syncSettings();

    expect(settingsCommandNames()).toEqual([
      "sync_timer_duration_settings",
      "sync_automation_policies",
    ]);
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
    expect(
      useSettingsStore.getState().automationSettingsApplicationStatus,
    ).toBe("confirmed");
  });

  it("receives session completion events while the initial full state is pending", async () => {
    let resolveFullState: (payload: FullStatePayload) => void = () => {
      throw new Error("Expected the full-state request to be pending");
    };
    const pendingFullState = new Promise<FullStatePayload>((resolve) => {
      resolveFullState = resolve;
    });
    mockInvoke.mockReturnValue(pendingFullState);

    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_full_state", undefined);
    });
    emitSessionCompleted(sessionHandler);

    expect(useTimerStore.getState().requiresManualContinueDecision).toBe(true);

    await act(async () => {
      resolveFullState(createFullStatePayload());
      await pendingFullState;
    });
  });

  it("should report get_full_state command failures through code-derived connection presentation", async () => {
    vi.useFakeTimers();
    const backendError = {
      code: "window_control_error",
      message: "failed to read full state from C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValue(backendError);

    renderTimerSync();

    await flushTimerSyncSetup();
    await advanceThroughTimerSyncConnectionRetries();

    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      technicalDetail: "window_control_error",
      severity: "recoverable",
      origin: "backend_connection",
      canRetry: true,
    });
    expect(connectionError?.retryFn).toEqual(expect.any(Function));
    expect(connectionError?.message).toEqual(expect.any(String));
    expect(connectionError?.message).not.toBe("");
    expect(connectionError?.message).not.toContain("failed to read full state");
    expect(connectionError?.message).not.toContain("C:\\Users\\nak");
    expect(connectionError?.message).not.toContain("secret");
  });

  it("should retry raw get_full_state invoke failures as connection failures", async () => {
    vi.useFakeTimers();
    mockInvoke.mockRejectedValue(
      new Error("transport failed from C:\\Users\\nak\\secret"),
    );

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(getFullStateInvokeCount()).toBe(1);
    expect(useErrorStore.getState().errors.connection).toBeUndefined();

    await advanceThroughTimerSyncConnectionRetries();

    expect(getFullStateInvokeCount()).toBe(
      TIMER_SYNC_MAX_CONNECTION_RETRIES + 1,
    );
    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      message:
        "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
      technicalDetail: "Unexpected backend error",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
    });
    expect(connectionError?.retryFn).toEqual(expect.any(Function));
  });

  it("should report non-retryable get_full_state command failures without automatic retries", async () => {
    vi.useFakeTimers();
    const backendError = {
      code: "invalid_command_input",
      message: "invalid type near C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValue(backendError);

    renderTimerSync();

    await flushTimerSyncSetup();

    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      technicalDetail: "invalid_command_input",
      severity: "recoverable",
      origin: "backend_connection",
      canRetry: false,
      retryFn: null,
    });
    expect(connectionError?.message).toEqual(expect.any(String));
    expect(connectionError?.message).not.toBe("");
    expect(connectionError?.message).not.toContain("invalid type");
    expect(connectionError?.message).not.toContain("C:\\Users\\nak");
    expect(connectionError?.message).not.toContain("secret");

    expect(getFullStateInvokeCount()).toBe(1);

    await vi.advanceTimersByTimeAsync(timerSyncRetryDelayMs(0));

    expect(getFullStateInvokeCount()).toBe(1);
  });

  it("should clean up listeners from a failed setup attempt before retrying", async () => {
    const unlistenTick = vi.fn();

    mockListen.mockImplementation((event: string) => {
      if (event === "timer_tick") {
        return Promise.resolve(unlistenTick);
      }

      if (event === "session_completed") {
        return Promise.reject(new Error("session listener failed"));
      }

      return Promise.resolve(noopUnlisten);
    });

    renderTimerSync();

    await waitFor(() => {
      expect(unlistenTick).toHaveBeenCalledTimes(1);
    });
  });

  it("should retry setup failures according to the connection retry policy", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(mockListen).toHaveBeenCalledTimes(1);

    await timerSyncConnectionRetryAttempts().reduce<Promise<void>>(
      async (previousRetry, retryAttempt) => {
        await previousRetry;
        const retryDelayMs = timerSyncRetryDelayMs(retryAttempt);
        const setupAttemptCountBeforeRetry = retryAttempt + 1;

        await vi.advanceTimersByTimeAsync(retryDelayMs - 1);
        expect(mockListen).toHaveBeenCalledTimes(setupAttemptCountBeforeRetry);

        await vi.advanceTimersByTimeAsync(1);

        expect(mockListen).toHaveBeenCalledTimes(
          setupAttemptCountBeforeRetry + 1,
        );
      },
      Promise.resolve(),
    );

    expect(mockListen).toHaveBeenCalledTimes(
      TIMER_SYNC_MAX_CONNECTION_RETRIES + 1,
    );
  });

  it("should report a retryable connection error after retry exhaustion", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(mockListen).toHaveBeenCalledTimes(1);

    await advanceThroughTimerSyncConnectionRetries();

    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      message:
        "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
      technicalDetail: "Error: listen failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
    });
    expect(connectionError?.retryFn).toEqual(expect.any(Function));
    expect(useTimerStore.getState().isConnected).toBe(false);
  });

  it("should clear the connection error and restart setup from attempt 0 on manual retry", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(mockListen).toHaveBeenCalledTimes(1);

    await advanceThroughTimerSyncConnectionRetries();

    expect(useErrorStore.getState().errors.connection?.retryFn).toEqual(
      expect.any(Function),
    );

    const retryFn = useErrorStore.getState().errors.connection?.retryFn;
    if (!retryFn) {
      throw new Error("Expected connection retry function");
    }

    mockListen.mockClear();
    await retryFn();

    expect(useErrorStore.getState().errors.connection).toBeUndefined();
    expect(mockListen).toHaveBeenCalledTimes(1);
  });

  it("should reconnect successfully when manual retry setup succeeds", async () => {
    vi.useFakeTimers();
    setApplicationStatuses("confirmed", "unconfirmed");
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    await advanceThroughTimerSyncConnectionRetries();

    const retryFn = useErrorStore.getState().errors.connection?.retryFn;
    if (!retryFn) {
      throw new Error("Expected connection retry function");
    }

    mockListen.mockClear();
    mockInvoke.mockClear();
    mockListen.mockResolvedValue(noopUnlisten);
    await retryFn();

    expect(useErrorStore.getState().errors.connection).toBeUndefined();
    expect(useTimerStore.getState().isConnected).toBe(true);
    expect(mockInvoke.mock.calls).toContainEqual(["get_full_state", undefined]);
    expect(mockListen.mock.calls.map(([event]) => event)).toContain(
      "timer_tick",
    );
    expect(settingsCommandNames()).toEqual(["sync_automation_policies"]);
  });

  it("restores the running timer and marks the connection available", async () => {
    mockInvoke.mockResolvedValue(
      createFullStatePayload({
        status: TimerStatus.WORKING,
        remaining_seconds: 777,
        current_cycle: 2,
      }),
    );
    renderTimerSync();

    await waitFor(() => {
      expect(useTimerStore.getState()).toMatchObject({
        status: TimerStatus.WORKING,
        remainingSeconds: 777,
        currentCycle: 2,
        isConnected: true,
      });
    });
  });

  it("applies duration readback before starting settings reconciliation", async () => {
    confirmCurrentCandidates();
    let synchronizationObservation:
      | {
          durationStatus: SettingsApplicationStatus;
          remainingSeconds: number;
          isConnected: boolean;
        }
      | undefined = undefined;
    mockInvoke.mockImplementation((command) => {
      if (command === "get_full_state") {
        return Promise.resolve({
          status: "idle",
          remaining_seconds: 777,
          session_type: "work",
          current_cycle: 0,
          completed_pomodoros: 0,
          break_interaction_count: 0,
          is_fullscreen_break: false,
          away_during_break: false,
          post_break_decision_pending: false,
          boot_auto_start_prompt_window: null,
          active_window_summary: null,
          settings: {
            work_duration_secs: 1200,
            short_break_duration_secs: 300,
            long_break_duration_secs: 900,
            long_break_interval: 4,
            post_break_transition_policy: "require_manual_continue",
          },
        });
      }
      if (command === "sync_timer_duration_settings") {
        const timerState = useTimerStore.getState();
        synchronizationObservation = {
          durationStatus:
            useSettingsStore.getState().durationSettingsApplicationStatus,
          remainingSeconds: timerState.remainingSeconds,
          isConnected: timerState.isConnected,
        };
      }
      return Promise.resolve(undefined);
    });

    renderTimerSync();

    await waitFor(() => {
      expect(synchronizationObservation).toBeDefined();
    });
    expect(synchronizationObservation).toEqual({
      durationStatus: "pending",
      remainingSeconds: 777,
      isConnected: true,
    });
  });

  it.each([
    {
      label: "both confirmed",
      durationStatus: "confirmed" as const,
      automationStatus: "confirmed" as const,
      expectedCommands: [],
    },
    {
      label: "only duration pending",
      durationStatus: "pending" as const,
      automationStatus: "confirmed" as const,
      expectedCommands: ["sync_timer_duration_settings"],
    },
    {
      label: "only automation unconfirmed",
      durationStatus: "confirmed" as const,
      automationStatus: "unconfirmed" as const,
      expectedCommands: ["sync_automation_policies"],
    },
    {
      label: "both unresolved",
      durationStatus: "unconfirmed" as const,
      automationStatus: "pending" as const,
      expectedCommands: [
        "sync_timer_duration_settings",
        "sync_automation_policies",
      ],
    },
  ])(
    "synchronizes only eligible unresolved groups at startup when $label",
    async ({ durationStatus, automationStatus, expectedCommands }) => {
      setApplicationStatuses(durationStatus, automationStatus);
      renderTimerSync();

      await waitFor(() => {
        expect(useTimerStore.getState().isConnected).toBe(true);
        expect(settingsCommandNames()).toEqual(expectedCommands);
      });
    },
  );

  it("should handle timer_tick events", async () => {
    const tickHandler = await renderAndGetTimerEventHandler("timer_tick");
    emitTimerTick(tickHandler);

    expect(useTimerStore.getState().status).toBe(TimerStatus.WORKING);
    expect(useTimerStore.getState().remainingSeconds).toBe(1499);
  });

  it("should handle session_completed events", async () => {
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler);

    expect(useTimerStore.getState().requiresManualContinueDecision).toBe(true);
  });

  it("notifies the user with the long-break sound when work ends before a long break", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "work_to_break",
      completed_session_type: SessionType.WORK,
      next_status: TimerStatus.ON_BREAK,
      next_session_type: SessionType.LONG_BREAK,
      requires_manual_continue_decision: false,
      post_break_decision_pending: false,
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "休憩時間です！",
        body: "お疲れ様でした。休憩を取りましょう。",
        sound: "/app/resources/sounds/long_break.wav",
      });
    });
  });

  it("notifies the user to resume work when a break awaits their decision", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      completed_session_type: SessionType.LONG_BREAK,
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "作業に戻りましょう！",
        body: "休憩が終わりました。次のポモドーロを始めましょう。",
        sound: "/app/resources/sounds/work.wav",
      });
    });
  });

  it("notifies the user when the next work session starts automatically", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "break_auto_started",
      next_status: TimerStatus.WORKING,
      requires_manual_continue_decision: false,
      post_break_decision_pending: false,
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "休憩終了",
        body: "休憩終了。次の作業を自動開始しました。",
        sound: "/app/resources/sounds/work.wav",
      });
    });
  });

  it("does not notify the user to resume work after a scheduled stop", async () => {
    vi.useFakeTimers();
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "scheduled_stop_executed",
      requires_manual_continue_decision: false,
      post_break_decision_pending: false,
    });

    await vi.runOnlyPendingTimersAsync();

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("releases a listener registered after unmount without continuing setup", async () => {
    const unlisten = vi.fn();
    let resolveRegistration: (unlisten: () => void) => void = () => {
      throw new Error("Expected a pending listener registration");
    };
    const pendingRegistration = new Promise<() => void>((resolve) => {
      resolveRegistration = resolve;
    });
    mockListen.mockReturnValue(pendingRegistration);

    const { unmount } = renderTimerSync();
    unmount();

    await act(async () => {
      resolveRegistration(unlisten);
      await pendingRegistration;
    });

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(useTimerStore.getState().isConnected).toBe(false);
  });

  it("cancels a pending connection retry on unmount", async () => {
    vi.useFakeTimers();
    mockInvoke.mockRejectedValue(new Error("connection unavailable"));

    const { unmount } = renderTimerSync();
    await flushTimerSyncSetup();
    expect(getFullStateInvokeCount()).toBe(1);

    unmount();
    mockListen.mockClear();
    mockInvoke.mockClear();
    await advanceThroughTimerSyncConnectionRetries();

    expect(mockListen).not.toHaveBeenCalled();
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(useErrorStore.getState().errors.connection).toBeUndefined();
  });

  it("releases every listener and ignores full state resolved after unmount", async () => {
    vi.spyOn(useSettingsStore.persist, "hasHydrated").mockReturnValue(true);
    setApplicationStatuses("pending", "pending");

    const acquiredUnlisteners: ReturnType<typeof vi.fn>[] = [];
    mockListen.mockImplementation(() => {
      const unlisten = vi.fn();
      acquiredUnlisteners.push(unlisten);
      return Promise.resolve(unlisten);
    });

    let resolvePendingFullState: (payload: FullStatePayload) => void = () => {
      throw new Error("Expected pending full-state resolver");
    };
    const pendingFullState = new Promise<FullStatePayload>((resolve) => {
      resolvePendingFullState = resolve;
    });
    mockInvoke.mockImplementation((command) => {
      if (command === "get_full_state") {
        return pendingFullState;
      }
      return Promise.resolve(undefined);
    });

    const remainingSecondsBeforeUnmount = 823;
    useTimerStore.setState({
      remainingSeconds: remainingSecondsBeforeUnmount,
      isConnected: false,
    });
    const { unmount } = renderTimerSync();

    await waitFor(() => {
      expect(getFullStateInvokeCount()).toBe(1);
    });

    unmount();

    expect(acquiredUnlisteners).not.toHaveLength(0);
    for (const unlisten of acquiredUnlisteners) {
      expect(unlisten).toHaveBeenCalledTimes(1);
    }

    await act(async () => {
      resolvePendingFullState(
        createFullStatePayload({ remaining_seconds: 17 }),
      );
      await pendingFullState;
      await Promise.resolve();
    });

    expect(useTimerStore.getState().remainingSeconds).toBe(
      remainingSecondsBeforeUnmount,
    );
    expect(useTimerStore.getState().isConnected).toBe(false);
    expect(settingsCommandNames()).toEqual([]);
  });

  it("should show stop-pending info on auto_run_stop_pending events", async () => {
    const stopPendingListener = await renderAndGetTimerEventHandler(
      "auto_run_stop_pending",
    );
    stopPendingListener({ payload: {} });

    expect(useTimerStore.getState().infoMessage?.id).toBe(
      "auto_run_stop_pending",
    );
  });

  it("should show stopped info on auto_run_stopped events", async () => {
    const stoppedListener =
      await renderAndGetTimerEventHandler("auto_run_stopped");
    stoppedListener({ payload: {} });

    expect(useTimerStore.getState().infoMessage?.id).toBe("auto_run_stopped");
  });

  it("defers restored duration reconciliation while non-idle and recovers on idle", async () => {
    vi.useFakeTimers();
    setApplicationStatuses("unconfirmed", "confirmed");
    mockInvoke.mockImplementation((command) => {
      if (command === "get_full_state") {
        return Promise.resolve(
          createFullStatePayload({
            status: TimerStatus.WORKING,
            remaining_seconds: 900,
            current_cycle: 1,
          }),
        );
      }
      return Promise.resolve(undefined);
    });

    const tickHandler = await renderAndGetTimerEventHandler("timer_tick");
    await vi.waitFor(() => {
      expect(useTimerStore.getState().isConnected).toBe(true);
    });
    await vi.runOnlyPendingTimersAsync();
    expect(settingsCommandNames()).toEqual([]);
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "unconfirmed",
    );

    emitTimerTick(tickHandler, {
      status: TimerStatus.IDLE,
      remaining_seconds: 1500,
    });

    await vi.waitFor(() => {
      expect(settingsCommandNames()).toEqual(["sync_timer_duration_settings"]);
    });
    expect(useSettingsStore.getState().durationSettingsApplicationStatus).toBe(
      "confirmed",
    );
  });

  it("keeps the connection established when settings synchronization rejects", async () => {
    setApplicationStatuses("pending", "confirmed");
    mockInvoke.mockImplementation((command) => {
      if (command === "get_full_state") {
        return Promise.resolve(createFullStatePayload());
      }
      if (command === "sync_timer_duration_settings") {
        return Promise.reject(new Error("failed to persist duration settings"));
      }
      return Promise.resolve(undefined);
    });

    renderTimerSync();

    await waitFor(() => {
      expect(
        useSettingsStore.getState().durationSettingsApplicationStatus,
      ).toBe("unconfirmed");
    });
    expect(useTimerStore.getState().isConnected).toBe(true);
    expect(useErrorStore.getState().errors.connection).toBeUndefined();
    expect(getFullStateInvokeCount()).toBe(1);
    expect(settingsCommandNames()).toEqual(["sync_timer_duration_settings"]);
  });
});
