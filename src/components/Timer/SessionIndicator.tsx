import { useTimerObservationStore } from "@/store/timerObservationStore";

/**
 * SessionIndicator Component
 * Displays current pomodoro cycle progress
 *
 * Phase 2: Shows "ポモドーロ X/Y" where:
 * - X = current cycle (1-4 typically)
 * - Y = long break interval (4 typically)
 *
 * Example: "ポモドーロ 2/4" means 2nd pomodoro out of 4 before long break
 */
export function SessionIndicator(): React.ReactElement | null {
  const currentCycle = useTimerObservationStore((state) => state.currentCycle);
  const longBreakInterval = useTimerObservationStore(
    (state) => state.settings?.longBreakInterval ?? null,
  );

  // Don't show if cycle hasn't started yet
  if (
    currentCycle === null ||
    currentCycle === 0 ||
    longBreakInterval === null
  ) {
    return null;
  }

  // Past the interval, the next break is long; show it as the last cycle.
  const displayedCycle = Math.min(currentCycle, longBreakInterval);
  const ariaLabel = `ポモドーロ ${String(displayedCycle)} / ${String(
    longBreakInterval,
  )}`;

  return (
    <div
      className="text-lg font-medium text-text"
      data-testid="session-indicator"
      aria-label={ariaLabel}
    >
      ポモドーロ {displayedCycle}/{longBreakInterval}
    </div>
  );
}
