import { describe, it, expect, beforeEach, vi, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTimer } from '../useTimer';
import { useTimerStore } from '../../store/timerStore';

describe('useTimer', () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
    // Use fake timers to control time in tests
    vi.useFakeTimers();
  });

  afterEach(() => {
    // Restore real timers after each test
    vi.restoreAllMocks();
  });

  it('should not start interval when timer status is IDLE', () => {
    const tickSpy = vi.spyOn(useTimerStore.getState(), 'tick');

    renderHook(() => useTimer());

    // Advance time by 1 second
    vi.advanceTimersByTime(1000);

    // tick should not be called when status is IDLE
    expect(tickSpy).not.toHaveBeenCalled();
  });

  it('should start interval when timer status is RUNNING', () => {
    const { start } = useTimerStore.getState();
    start();

    renderHook(() => useTimer());

    const initialSeconds = useTimerStore.getState().remainingSeconds;

    // Advance time by 1 second
    vi.advanceTimersByTime(1000);

    // remainingSeconds should decrease by 1
    expect(useTimerStore.getState().remainingSeconds).toBe(initialSeconds - 1);
  });

  it('should call tick every second when running', () => {
    const { start } = useTimerStore.getState();
    start();

    renderHook(() => useTimer());

    const initialSeconds = useTimerStore.getState().remainingSeconds;

    // Advance time by 3 seconds
    vi.advanceTimersByTime(3000);

    // remainingSeconds should decrease by 3
    expect(useTimerStore.getState().remainingSeconds).toBe(initialSeconds - 3);
  });

  it('should stop interval when timer is stopped', () => {
    const { start, stop } = useTimerStore.getState();

    act(() => {
      start();
    });

    renderHook(() => useTimer());

    // Advance time by 1 second
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    const currentSeconds = useTimerStore.getState().remainingSeconds;

    // Stop the timer
    act(() => {
      stop();
    });

    // Advance time by another second
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    // remainingSeconds should not decrease after stopping
    expect(useTimerStore.getState().remainingSeconds).toBe(currentSeconds);
  });

  it('should cleanup interval on unmount', () => {
    const { start } = useTimerStore.getState();
    start();

    const { unmount } = renderHook(() => useTimer());

    const currentSeconds = useTimerStore.getState().remainingSeconds;

    // Unmount the hook
    unmount();

    // Advance time by 1 second
    vi.advanceTimersByTime(1000);

    // remainingSeconds should not decrease after unmount
    expect(useTimerStore.getState().remainingSeconds).toBe(currentSeconds);
  });
});
