import { useTimerStore } from "@/store/timerStore";
import { useSettingsStore } from "@/store/settingsStore";

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
  const currentCycle = useTimerStore((state) => state.currentCycle);
  const longBreakInterval = useSettingsStore(
    (state) => state.longBreakInterval,
  );
  const ariaLabel = `ポモドーロ ${String(currentCycle)} / ${String(
    longBreakInterval,
  )}`;

  // Don't show if cycle hasn't started yet
  if (currentCycle === 0) {
    return null;
  }

  return (
    <div
      className="text-lg font-medium text-text"
      data-testid="session-indicator"
      aria-label={ariaLabel}
    >
      ポモドーロ {currentCycle}/{longBreakInterval}
    </div>
  );
}
