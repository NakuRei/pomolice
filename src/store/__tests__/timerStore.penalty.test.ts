/**
 * Timer Store Penalty Tests
 * Phase 4: Test input monitoring and penalty system
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useTimerStore, SessionType, TimerStatus } from '../timerStore';
import { useSettingsStore } from '../settingsStore';

// Mock the services
vi.mock('../../services/notifications', () => ({
  notifyWorkComplete: vi.fn(),
  notifyBreakComplete: vi.fn(),
  notifyLongBreakComplete: vi.fn(),
}));

vi.mock('../../services/windowControl', () => ({
  startBreak: vi.fn().mockResolvedValue(undefined),
  completeBreak: vi.fn().mockResolvedValue(undefined),
  startWork: vi.fn().mockResolvedValue(undefined),
  completeWork: vi.fn().mockResolvedValue(undefined),
}));

describe('Timer Store - Penalty System', () => {
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
      lastPenaltyTime: 0,
    });

    useSettingsStore.setState({
      workDuration: 25,
      shortBreakDuration: 5,
      longBreakDuration: 15,
      longBreakInterval: 4,
    });
  });

  describe('breakInteractionCount', () => {
    it('should initialize with 0 interactions', () => {
      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0);
    });

    it('should reset to 0 when entering break mode', async () => {
      // Set some interaction count
      useTimerStore.setState({ breakInteractionCount: 5 });

      // Enter break mode
      await useTimerStore.getState().enterBreakMode();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0);
    });

    it('should increment on handleInputDuringBreak call', () => {
      // Setup: in break mode with short break session
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 3 * 60, // 3 minutes remaining
        breakInteractionCount: 0,
      });

      // Handle input
      useTimerStore.getState().handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(1);
    });

    it('should increment multiple times for multiple inputs', () => {
      vi.useFakeTimers();
      const baseTime = new Date('2025-01-01T00:00:00.000Z').getTime();
      vi.setSystemTime(new Date(baseTime));
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 3 * 60,
        breakInteractionCount: 0,
      });

      const { handleInputDuringBreak } = useTimerStore.getState();

      handleInputDuringBreak();
      vi.setSystemTime(new Date(baseTime + 200));
      handleInputDuringBreak();
      vi.setSystemTime(new Date(baseTime + 400));
      handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(3);
      vi.useRealTimers();
    });

    it('should reset to 0 on emergency stop', () => {
      useTimerStore.setState({ breakInteractionCount: 10 });

      useTimerStore.getState().emergencyStop();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0);
    });
  });

  describe('resetBreakTimer', () => {
    it('should reset short break timer to full duration', () => {
      useTimerStore.setState({
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 30, // Only 30 seconds left
      });

      useTimerStore.getState().resetBreakTimer();

      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(5 * 60); // Reset to 5 minutes
    });

    it('should reset long break timer to full duration', () => {
      useTimerStore.setState({
        sessionType: SessionType.LONG_BREAK,
        remainingSeconds: 60, // Only 1 minute left
      });

      useTimerStore.getState().resetBreakTimer();

      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(15 * 60); // Reset to 15 minutes
    });

    it('should not reset work session timer', () => {
      useTimerStore.setState({
        sessionType: SessionType.WORK,
        remainingSeconds: 10 * 60, // 10 minutes left
      });

      useTimerStore.getState().resetBreakTimer();

      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(10 * 60); // Unchanged
    });

    it('should use custom break durations from settings', () => {
      useSettingsStore.setState({
        shortBreakDuration: 10,
        longBreakDuration: 20,
      });

      // Test short break
      useTimerStore.setState({
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 1,
      });

      useTimerStore.getState().resetBreakTimer();

      let state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(10 * 60);

      // Test long break
      useTimerStore.setState({
        sessionType: SessionType.LONG_BREAK,
        remainingSeconds: 1,
      });

      useTimerStore.getState().resetBreakTimer();

      state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(20 * 60);
    });
  });

  describe('handleInputDuringBreak', () => {
    it('should apply penalty only when in fullscreen break mode', () => {
      useTimerStore.setState({
        isFullscreenBreak: false, // Not in break mode
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 2 * 60,
        breakInteractionCount: 0,
      });

      useTimerStore.getState().handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0); // No increment
      expect(state.remainingSeconds).toBe(2 * 60); // No reset
    });

    it('should apply penalty only during break sessions', () => {
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.WORK, // Work session, not break
        remainingSeconds: 10 * 60,
        breakInteractionCount: 0,
      });

      useTimerStore.getState().handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(0); // No increment
      expect(state.remainingSeconds).toBe(10 * 60); // No reset
    });

    it('should reset timer and increment count during short break', () => {
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 30, // Almost done
        breakInteractionCount: 0,
      });

      useTimerStore.getState().handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(1);
      expect(state.remainingSeconds).toBe(5 * 60); // Reset to full duration
    });

    it('should reset timer and increment count during long break', () => {
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.LONG_BREAK,
        remainingSeconds: 100, // Almost done
        breakInteractionCount: 2,
      });

      useTimerStore.getState().handleInputDuringBreak();

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(3); // Incremented
      expect(state.remainingSeconds).toBe(15 * 60); // Reset to full duration
    });

    it('should apply silent penalty without warnings', () => {
      // This test verifies that the penalty is applied silently
      // No modal, no alert, just a console log (not testable in unit tests)
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 2 * 60,
      });

      // Should not throw any errors
      expect(() => {
        useTimerStore.getState().handleInputDuringBreak();
      }).not.toThrow();

      const state = useTimerStore.getState();
      expect(state.remainingSeconds).toBe(5 * 60);
    });

    it('should handle multiple rapid penalties correctly', () => {
      vi.useFakeTimers();
      const baseTime = new Date('2025-01-01T00:00:00.000Z').getTime();
      vi.setSystemTime(new Date(baseTime));
      useTimerStore.setState({
        isFullscreenBreak: true,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 1 * 60,
        breakInteractionCount: 0,
      });

      const { handleInputDuringBreak } = useTimerStore.getState();

      // Simulate rapid input (5 penalties)
      for (let i = 0; i < 5; i++) {
        handleInputDuringBreak();
        vi.setSystemTime(new Date(baseTime + (i + 1) * 200));
      }

      const state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(5);
      expect(state.remainingSeconds).toBe(5 * 60); // Still reset to full
      vi.useRealTimers();
    });
  });

  describe('Integration with break mode lifecycle', () => {
    it('should track interactions throughout a break session', async () => {
      vi.useFakeTimers();
      const baseTime = new Date('2025-01-01T00:00:00.000Z').getTime();
      vi.setSystemTime(new Date(baseTime));
      useTimerStore.setState({
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 5 * 60,
      });

      // Enter break mode
      await useTimerStore.getState().enterBreakMode();

      let state = useTimerStore.getState();
      expect(state.isFullscreenBreak).toBe(true);
      expect(state.breakInteractionCount).toBe(0);

      // User interacts 3 times
      const { handleInputDuringBreak } = useTimerStore.getState();
      handleInputDuringBreak();
      vi.setSystemTime(new Date(baseTime + 200));
      handleInputDuringBreak();
      vi.setSystemTime(new Date(baseTime + 400));
      handleInputDuringBreak();

      state = useTimerStore.getState();
      expect(state.breakInteractionCount).toBe(3);

      // Exit break mode
      await useTimerStore.getState().exitBreakMode();

      state = useTimerStore.getState();
      expect(state.isFullscreenBreak).toBe(false);
      // Interaction count persists for statistics (not reset on exit)
      expect(state.breakInteractionCount).toBe(3);
      vi.useRealTimers();
    });
  });
});
