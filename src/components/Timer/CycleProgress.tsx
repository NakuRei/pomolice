import { useTimerStore } from "../../store/timerStore";
import { useSettingsStore } from "../../store/settingsStore";

/**
 * CycleProgress Component
 * Displays visual progress dots showing completed cycles before long break
 *
 * Phase 2: Visual representation of pomodoro cycle progress
 * - Filled dots (●) = completed cycles
 * - Empty dots (○) = remaining cycles
 *
 * Example with 4 cycles, 2 completed: ●●○○
 */
export function CycleProgress(): React.ReactElement | null {
  const currentCycle = useTimerStore((state) => state.currentCycle);
  const longBreakInterval = useSettingsStore(
    (state) => state.longBreakInterval,
  );

  // Don't show if cycle hasn't started yet
  if (currentCycle === 0) {
    return null;
  }

  // Create array of dots based on long break interval
  const dots = Array.from({ length: longBreakInterval }, (_, index) => {
    const isCompleted = index < currentCycle;
    const dotNumber = index + 1;

    return (
      <div
        key={index}
        className={`h-3 w-3 rounded-full transition-colors ${
          isCompleted ? "bg-primary-700" : "bg-border-strong"
        }`}
        data-testid={`cycle-dot-${String(dotNumber)}`}
        aria-label={`サイクル ${String(dotNumber)}${isCompleted ? " 完了" : ""}`}
      />
    );
  });

  return (
    <div
      className="flex items-center justify-center gap-2"
      data-testid="cycle-progress"
      role="img"
      aria-label={`長い休憩まで ${String(currentCycle)} / ${String(longBreakInterval)} サイクル完了`}
    >
      {dots}
    </div>
  );
}
