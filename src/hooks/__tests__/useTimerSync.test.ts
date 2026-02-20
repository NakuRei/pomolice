/**
 * useTimerSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify that the hook correctly sets up event listeners.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook, waitFor } from "@testing-library/react";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";

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
  notifyBreakComplete: vi.fn(),
  notifyLongBreakComplete: vi.fn(),
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
      showContinueDialog: false,
      isConnected: false,
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
      show_continue_dialog: false,
      settings: {
        work_duration_secs: 1500,
        short_break_duration_secs: 300,
        long_break_duration_secs: 900,
        long_break_interval: 4,
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
        completed_session_type: "short_break",
        next_session_type: null,
        show_continue_dialog: true,
      },
    });

    expect(useTimerStore.getState().showContinueDialog).toBe(true);
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

  it("should sync settings to backend on mount", async () => {
    renderHook(() => useTimerSync());

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith(
        "sync_settings",
        expect.objectContaining({
          workDurationSecs: expect.any(Number),
          shortBreakDurationSecs: expect.any(Number),
          longBreakDurationSecs: expect.any(Number),
          longBreakInterval: expect.any(Number),
        })
      );
    });
  });
});
