/**
 * useTimerSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify that the hook correctly sets up event listeners.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useTimerStore, TimerStatus, SessionType } from "@/store/timerStore";
import { useErrorStore } from "@/store/errorStore";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "@/services/notifications";
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

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("@/services/notifications", () => ({
  notifyWorkComplete: vi.fn(),
  notifyBreakAutoStarted: vi.fn(),
  notifyBreakComplete: vi.fn(),
}));

// Import after mocking
import { useTimerSync } from "./useTimerSync";

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

function getInvokePayload(commandName: string): Record<string, unknown> {
  const invokeCall = mockInvoke.mock.calls.find(
    ([invokedCommand]) => invokedCommand === commandName,
  );
  expect(invokeCall).toBeDefined();
  if (!invokeCall) {
    throw new Error(`Expected invoke command "${commandName}"`);
  }

  const payload = invokeCall[1];
  expectRecord(payload, `${commandName} payload`);
  return payload;
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

describe("useTimerSync", () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    useErrorStore.getState().clearAllErrors();

    // Reset store to initial state
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

    // Default mock implementations
    mockListen.mockResolvedValue(noopUnlisten);
    mockInvoke.mockResolvedValue({
      status: "idle",
      remaining_seconds: 1500,
      session_type: "work",
      current_cycle: 0,
      completed_pomodoros: 0,
      break_interaction_count: 0,
      is_fullscreen_break: false,
      post_break_decision_pending: false,
      should_prompt_start_on_boot: false,
      active_window_summary: null,
      settings: {
        work_duration_secs: 1500,
        short_break_duration_secs: 300,
        long_break_duration_secs: 900,
        long_break_interval: 4,
        post_break_transition_policy: "require_manual_continue",
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("should set up event listeners on mount", async () => {
    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalled();
    });

    // Should set up listeners for timer events
    const listenCalls = mockListen.mock.calls.map((call) => call[0]);
    expect(listenCalls).toContain("timer_tick");
    expect(listenCalls).toContain("session_completed");
    expect(listenCalls).toContain("auto_run_stop_pending");
    expect(listenCalls).toContain("auto_run_stopped");
  });

  it("should report get_full_state command failures through code-derived connection presentation", async () => {
    vi.useFakeTimers();
    const backendError = {
      code: "window_control_error",
      message: "failed to read full state from C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValue(backendError);

    renderHook(() => {
      useTimerSync();
    });

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

    renderHook(() => {
      useTimerSync();
    });

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

    renderHook(() => {
      useTimerSync();
    });

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

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(unlistenTick).toHaveBeenCalledTimes(1);
    });
  });

  it("should retry setup failures according to the connection retry policy", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderHook(() => {
      useTimerSync();
    });

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

    renderHook(() => {
      useTimerSync();
    });

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

    renderHook(() => {
      useTimerSync();
    });

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
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderHook(() => {
      useTimerSync();
    });

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
  });

  it("should call get_full_state on mount", async () => {
    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(mockInvoke.mock.calls).toContainEqual([
        "get_full_state",
        undefined,
      ]);
    });
  });

  it("should set isConnected to true after getting full state", async () => {
    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(useTimerStore.getState().isConnected).toBe(true);
    });
  });

  it("should handle timer_tick events", async () => {
    let tickHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "timer_tick") {
          tickHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(tickHandler).toBeDefined();
    });

    // Simulate timer tick from backend
    const definedTickHandler = getRequiredTimerEventHandler(
      tickHandler,
      "timer_tick handler",
    );
    definedTickHandler({
      payload: {
        status: "working",
        remaining_seconds: 1499,
        session_type: "work",
        current_cycle: 1,
        completed_pomodoros: 0,
        break_interaction_count: 0,
        is_fullscreen_break: false,
      },
    });

    expect(useTimerStore.getState().status).toBe(TimerStatus.WORKING);
    expect(useTimerStore.getState().remainingSeconds).toBe(1499);
  });

  it("should handle session_completed events", async () => {
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "session_completed") {
          sessionHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    // Simulate session completed from backend
    const definedSessionHandler = getRequiredTimerEventHandler(
      sessionHandler,
      "session_completed handler",
    );
    definedSessionHandler({
      payload: {
        transition_reason: "break_waiting_decision",
        completed_session_type: "short_break",
        next_status: "idle",
        next_session_type: "work",
        requires_manual_continue_decision: true,
        post_break_decision_pending: true,
      },
    });

    expect(useTimerStore.getState().requiresManualContinueDecision).toBe(true);
  });

  it("should call notifyWorkComplete with next_session_type", async () => {
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "session_completed") {
          sessionHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    const definedSessionHandler = getRequiredTimerEventHandler(
      sessionHandler,
      "session_completed handler",
    );
    definedSessionHandler({
      payload: {
        transition_reason: "work_to_break",
        completed_session_type: "work",
        next_status: "on_break",
        next_session_type: "long_break",
        requires_manual_continue_decision: false,
        post_break_decision_pending: false,
      },
    });

    expect(notifyWorkComplete).toHaveBeenCalledWith("long_break");
    expect(notifyWorkComplete).toHaveBeenCalledTimes(1);
    expect(notifyBreakComplete).not.toHaveBeenCalled();
    expect(notifyBreakAutoStarted).not.toHaveBeenCalled();
  });

  it("should call notifyBreakComplete when transition_reason is break_waiting_decision", async () => {
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "session_completed") {
          sessionHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    const definedSessionHandler = getRequiredTimerEventHandler(
      sessionHandler,
      "session_completed handler",
    );
    definedSessionHandler({
      payload: {
        transition_reason: "break_waiting_decision",
        completed_session_type: "long_break",
        next_status: "idle",
        next_session_type: "work",
        requires_manual_continue_decision: true,
        post_break_decision_pending: true,
      },
    });

    expect(notifyBreakComplete).toHaveBeenCalledTimes(1);
    expect(notifyBreakAutoStarted).not.toHaveBeenCalled();
    expect(notifyWorkComplete).not.toHaveBeenCalled();
  });

  it("should call notifyBreakAutoStarted when transition_reason is break_auto_started", async () => {
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "session_completed") {
          sessionHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    const definedSessionHandler = getRequiredTimerEventHandler(
      sessionHandler,
      "session_completed handler",
    );
    definedSessionHandler({
      payload: {
        transition_reason: "break_auto_started",
        completed_session_type: "short_break",
        next_status: "working",
        next_session_type: "work",
        requires_manual_continue_decision: false,
        post_break_decision_pending: false,
      },
    });

    expect(notifyBreakAutoStarted).toHaveBeenCalledTimes(1);
    expect(notifyBreakComplete).not.toHaveBeenCalled();
    expect(notifyWorkComplete).not.toHaveBeenCalled();
  });

  it("should not call work-resume notifications for scheduled_stop_executed", async () => {
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined =
      undefined;

    mockListen.mockImplementation(
      (event: string, handler: (event: { payload: unknown }) => void) => {
        if (event === "session_completed") {
          sessionHandler = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    const definedSessionHandler = getRequiredTimerEventHandler(
      sessionHandler,
      "session_completed handler",
    );
    definedSessionHandler({
      payload: {
        transition_reason: "scheduled_stop_executed",
        completed_session_type: "short_break",
        next_status: "idle",
        next_session_type: "work",
        requires_manual_continue_decision: false,
        post_break_decision_pending: false,
      },
    });

    expect(notifyWorkComplete).not.toHaveBeenCalled();
    expect(notifyBreakComplete).not.toHaveBeenCalled();
    expect(notifyBreakAutoStarted).not.toHaveBeenCalled();
  });

  it("should clean up listeners on unmount", async () => {
    const unsubscribe = vi.fn();
    mockListen.mockResolvedValue(unsubscribe);

    const { unmount } = renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalled();
    });

    unmount();

    // Unsubscribe should be called for each listener
    expect(unsubscribe).toHaveBeenCalled();
  });

  it("should show stop-pending info on auto_run_stop_pending events", async () => {
    let stopPendingListener: TimerEventHandler | undefined = undefined;

    mockListen.mockImplementation(
      (event: string, handler: TimerEventHandler | (() => void)) => {
        if (event === "auto_run_stop_pending") {
          stopPendingListener = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(stopPendingListener).toBeDefined();
    });

    const definedStopPendingListener = getRequiredTimerEventHandler(
      stopPendingListener,
      "auto_run_stop_pending listener",
    );
    definedStopPendingListener({ payload: {} });

    expect(useTimerStore.getState().infoMessage?.id).toBe(
      "auto_run_stop_pending",
    );
  });

  it("should show stopped info on auto_run_stopped events", async () => {
    let stoppedListener: TimerEventHandler | undefined = undefined;

    mockListen.mockImplementation(
      (event: string, handler: TimerEventHandler | (() => void)) => {
        if (event === "auto_run_stopped") {
          stoppedListener = handler;
        }
        return Promise.resolve(noopUnlisten);
      },
    );

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(stoppedListener).toBeDefined();
    });

    const definedStoppedListener = getRequiredTimerEventHandler(
      stoppedListener,
      "auto_run_stopped listener",
    );
    definedStoppedListener({ payload: {} });

    expect(useTimerStore.getState().infoMessage?.id).toBe("auto_run_stopped");
  });

  it("should sync settings to backend on mount", async () => {
    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      const durationSettingsPayload = getInvokePayload(
        "sync_timer_duration_settings",
      );
      expect(typeof durationSettingsPayload.workDurationSecs).toBe("number");
      expect(typeof durationSettingsPayload.shortBreakDurationSecs).toBe(
        "number",
      );
      expect(typeof durationSettingsPayload.longBreakDurationSecs).toBe(
        "number",
      );
      expect(typeof durationSettingsPayload.longBreakInterval).toBe("number");

      const automationPoliciesPayload = getInvokePayload(
        "sync_automation_policies",
      );
      expect(automationPoliciesPayload.postBreakTransitionPolicy).toBe(
        "require_manual_continue",
      );
      expect(typeof automationPoliciesPayload.autoRunFeatureEnabled).toBe(
        "boolean",
      );
      expectRecord(
        automationPoliciesPayload.weeklyAutoRunConfig,
        "weeklyAutoRunConfig",
      );
    });
  });
});
