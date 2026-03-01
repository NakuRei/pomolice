/**
 * Timer Store Penalty Tests
 * Phase 4: Test input monitoring and penalty system
 *
 * Note: Input penalty logic has been moved to the Rust backend.
 * The frontend now only receives penalty state via timer_tick events.
 * These tests verify that the frontend correctly handles penalty state
 * received from the backend.
 */

import { describe, it, expect, beforeEach, vi } from "vitest";
import { useTimerStore, SessionType, TimerStatus } from "../timerStore";
import { useSettingsStore } from "../settingsStore";

// Mock the services
vi.mock("../../services/notifications", () => ({
  notifyWorkComplete: vi.fn(),
  notifyBreakComplete: vi.fn(),
  notifyLongBreakComplete: vi.fn(),
}));

vi.mock("../../services/windowControl", () => ({
  startWork: vi.fn().mockResolvedValue(undefined),
}));

describe("Timer Store - Penalty System", () => {
  beforeEach(() => {
    // Reset stores to initial state
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      completedPomodoros: 0,
      currentCycle: 0,
      showContinueDialog: false,
      isFullscreenBreak: false,
      breakInteractionCount: 0,
      isConnected: false,
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });

    useSettingsStore.setState({
      workDuration: 25,
      shortBreakDuration: 5,
      longBreakDuration: 15,
      longBreakInterval: 4,
    });
  });

  describe("breakInteractionCount from backend", () => {
    it("should initialize with 0 interactions", () => {
      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0);
    });

    it("should update breakInteractionCount from timer_tick event", () => {
      const { handleTimerTick } = useTimerStore.getState();

      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 5,
        is_fullscreen_break: true,
      });

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(5);
    });

    it("should update breakInteractionCount on multiple ticks", () => {
      const { handleTimerTick } = useTimerStore.getState();

      // First tick with 0 interactions
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 0,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().breakInteractionCount).toBe(0);

      // Backend detected input, sent tick with incremented count
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300, // Timer reset by backend
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 1,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().breakInteractionCount).toBe(1);

      // More inputs detected
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 3,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().breakInteractionCount).toBe(3);
    });

    it("should reset breakInteractionCount when entering new break session", () => {
      // Set some interaction count from previous break
      useTimerStore.setState({ breakInteractionCount: 10 });

      const { handleTimerTick } = useTimerStore.getState();

      // New break session starts with 0 interactions
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 2,
        completed_pomodoros: 2,
        break_interaction_count: 0, // Backend resets on new break
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().breakInteractionCount).toBe(0);
    });
  });

  describe("Timer reset from backend penalty", () => {
    it("should reflect timer reset in remaining_seconds from tick", () => {
      const { handleTimerTick } = useTimerStore.getState();

      // Break running with 30 seconds left
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 30,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 0,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().remainingSeconds).toBe(30);

      // Backend detected input and reset timer to full duration
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300, // Reset to 5 minutes
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 1,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().remainingSeconds).toBe(300);
      expect(useTimerStore.getState().breakInteractionCount).toBe(1);
    });
  });

  describe("handleFullState for penalty state", () => {
    it("should update breakInteractionCount from full state", () => {
      const { handleFullState } = useTimerStore.getState();

      handleFullState({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 250,
        session_type: SessionType.LONG_BREAK,
        current_cycle: 4,
        completed_pomodoros: 4,
        break_interaction_count: 7,
        is_fullscreen_break: true,
        show_continue_dialog: false,
        should_prompt_start_on_boot: false,
        active_window_summary: null,
        settings: {
          work_duration_secs: 1500,
          short_break_duration_secs: 300,
          long_break_duration_secs: 900,
          long_break_interval: 4,
        },
      });

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(7);
      expect(state.isFullscreenBreak).toBe(true);
    });
  });

  describe("isFullscreenBreak state", () => {
    it("should track fullscreen break state from tick", () => {
      const { handleTimerTick } = useTimerStore.getState();

      // Not in fullscreen break
      expect(useTimerStore.getState().isFullscreenBreak).toBe(false);

      // Enter fullscreen break
      handleTimerTick({
        status: TimerStatus.ON_BREAK,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 0,
        is_fullscreen_break: true,
      });

      expect(useTimerStore.getState().isFullscreenBreak).toBe(true);

      // Exit fullscreen break (work session)
      handleTimerTick({
        status: TimerStatus.WORKING,
        remaining_seconds: 1500,
        session_type: SessionType.WORK,
        current_cycle: 2,
        completed_pomodoros: 1,
        break_interaction_count: 0,
        is_fullscreen_break: false,
      });

      expect(useTimerStore.getState().isFullscreenBreak).toBe(false);
    });
  });
});
