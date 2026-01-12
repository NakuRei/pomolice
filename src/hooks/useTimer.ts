import { useEffect } from 'react';
import { useTimerStore, TimerStatus } from '../store/timerStore';

/**
 * Custom hook for timer interval management
 * Handles the countdown logic with setInterval
 * Automatically starts/stops the interval based on timer status
 * Cleans up the interval on unmount
 */
export function useTimer() {
  const status = useTimerStore((state) => state.status);
  const tick = useTimerStore((state) => state.tick);

  useEffect(() => {
    // Only run interval when timer is running
    if (status !== TimerStatus.RUNNING) {
      return;
    }

    // Create interval that calls tick every 1000ms (1 second)
    const intervalId = setInterval(() => {
      tick();
    }, 1000);

    // Cleanup function: clear interval when effect is cleaned up
    return () => {
      clearInterval(intervalId);
    };
  }, [status, tick]);
}
