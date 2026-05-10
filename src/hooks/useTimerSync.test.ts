/**
 * useTimerSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify that the hook correctly sets up event listeners.
 */

import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useTimerStore, TimerStatus, SessionType } from "@/store/timerStore";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "@/services/notifications";

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
const { mockTimerSyncLogger } = vi.hoisted(() => ({
  mockTimerSyncLogger: {
    trace: vi.fn(),
    debug: vi.fn(),
    info: vi.fn(),
    warn: vi.fn(),
    error: vi.fn(),
  },
}));

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

vi.mock("@/logging/logger", () => ({
  createLogger: (): typeof mockTimerSyncLogger => mockTimerSyncLogger,
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

function getRequiredVoidListener(
  listener: (() => void) | undefined,
  description: string,
): () => void {
  if (!listener) {
    throw new Error(`Expected ${description} to be defined`);
  }

  return listener;
}

function expectVoidListener(
  handler: TimerEventHandler | (() => void),
  eventName: string,
): () => void {
  if (handler.length !== 0) {
    throw new Error(
      `Expected "${eventName}" to register a no-argument listener`,
    );
  }

  return handler as () => void;
}

describe("useTimerSync", () => {
  beforeEach(() => {
    vi.clearAllMocks();

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

  it("should log setup failures through the logger", async () => {
    mockListen.mockRejectedValueOnce(new Error("listen failed"));

    renderHook(() => {
      useTimerSync();
    });

    await waitFor(() => {
      expect(mockTimerSyncLogger.error).toHaveBeenCalledWith(
        FRONTEND_LOG_EVENT_IDS.timerSyncSetupFailed,
        expect.any(Error),
        expect.objectContaining({
          retry_attempt: 0,
          max_connection_retries: 3,
          failure_message: "Error: listen failed",
        }),
      );
    });
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
    let stopPendingListener: (() => void) | undefined = undefined;

    mockListen.mockImplementation(
      (event: string, handler: TimerEventHandler | (() => void)) => {
        if (event === "auto_run_stop_pending") {
          stopPendingListener = expectVoidListener(
            handler,
            "auto_run_stop_pending",
          );
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

    const definedStopPendingListener = getRequiredVoidListener(
      stopPendingListener,
      "auto_run_stop_pending listener",
    );
    definedStopPendingListener();

    expect(useTimerStore.getState().infoMessage?.id).toBe(
      "auto_run_stop_pending",
    );
  });

  it("should show stopped info on auto_run_stopped events", async () => {
    let stoppedListener: (() => void) | undefined = undefined;

    mockListen.mockImplementation(
      (event: string, handler: TimerEventHandler | (() => void)) => {
        if (event === "auto_run_stopped") {
          stoppedListener = expectVoidListener(handler, "auto_run_stopped");
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

    const definedStoppedListener = getRequiredVoidListener(
      stoppedListener,
      "auto_run_stopped listener",
    );
    definedStoppedListener();

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
