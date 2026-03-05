/**
 * useTimerSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify that the hook correctly sets up event listeners.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";
import {
  notifyWorkComplete,
  notifyBreakAutoStarted,
  notifyBreakComplete,
} from "../../services/notifications";

// Mock Tauri APIs
const mockListen = vi.fn();
const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => mockListen(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("../../services/notifications", () => ({
  notifyWorkComplete: vi.fn(),
  notifyBreakAutoStarted: vi.fn(),
  notifyBreakComplete: vi.fn(),
}));

// Import after mocking
import { useTimerSync } from "../useTimerSync";

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
    mockListen.mockResolvedValue(() => {});
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
    renderHook(() => useTimerSync());

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

  it("should call get_full_state on mount", async () => {
    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_full_state");
    });
  });

  it("should set isConnected to true after getting full state", async () => {
    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(useTimerStore.getState().isConnected).toBe(true);
    });
  });

  it("should handle timer_tick events", async () => {
    let tickHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "timer_tick") {
        tickHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(tickHandler).toBeDefined();
    });

    // Simulate timer tick from backend
    tickHandler!({
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
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "session_completed") {
        sessionHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    // Simulate session completed from backend
    sessionHandler!({
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
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "session_completed") {
        sessionHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    sessionHandler!({
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
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "session_completed") {
        sessionHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    sessionHandler!({
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
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "session_completed") {
        sessionHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    sessionHandler!({
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
    let sessionHandler: ((event: { payload: unknown }) => void) | undefined;

    mockListen.mockImplementation((event: string, handler: (event: { payload: unknown }) => void) => {
      if (event === "session_completed") {
        sessionHandler = handler;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(sessionHandler).toBeDefined();
    });

    sessionHandler!({
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

    const { unmount } = renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(mockListen).toHaveBeenCalled();
    });

    unmount();

    // Unsubscribe should be called for each listener
    expect(unsubscribe).toHaveBeenCalled();
  });

  it("should show stop-pending info on auto_run_stop_pending events", async () => {
    let stopPendingListener: (() => void) | undefined;

    mockListen.mockImplementation((event: string, listener: () => void) => {
      if (event === "auto_run_stop_pending") {
        stopPendingListener = listener;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(stopPendingListener).toBeDefined();
    });

    stopPendingListener!();

    expect(useTimerStore.getState().infoMessage?.id).toBe("auto_run_stop_pending");
  });

  it("should show stopped info on auto_run_stopped events", async () => {
    let stoppedListener: (() => void) | undefined;

    mockListen.mockImplementation((event: string, listener: () => void) => {
      if (event === "auto_run_stopped") {
        stoppedListener = listener;
      }
      return Promise.resolve(() => {});
    });

    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(stoppedListener).toBeDefined();
    });

    stoppedListener!();

    expect(useTimerStore.getState().infoMessage?.id).toBe("auto_run_stopped");
  });

  it("should sync settings to backend on mount", async () => {
    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "sync_timer_duration_settings",
        expect.objectContaining({
          workDurationSecs: expect.any(Number),
          shortBreakDurationSecs: expect.any(Number),
          longBreakDurationSecs: expect.any(Number),
          longBreakInterval: expect.any(Number),
        })
      );

      expect(mockInvoke).toHaveBeenCalledWith(
        "sync_automation_policies",
        expect.objectContaining({
          postBreakTransitionPolicy: "require_manual_continue",
          autoRunFeatureEnabled: expect.any(Boolean),
          weeklyAutoRunConfig: expect.any(Object),
        })
      );
    });
  });
});

