import { useTimerStore } from "../../store/timerStore";

/**
 * BreakStats Component
 *
 * Displays statistics during break screen:
 * - Current session count
 * - Today's break completion rate (placeholder for Phase 6)
 */
export function BreakStats() {
  const { completedPomodoros, currentCycle } = useTimerStore();

  // Phase 3: Break completion rate is placeholder
  // Will be implemented with statistics store in Phase 6
  const breakCompletionRate = 100; // Placeholder

  return (
    <div className="text-center space-y-3">
      <div className="text-lg text-gray-300">
        <span className="font-semibold">今日のポモドーロ:</span>{" "}
        <span className="text-white">{completedPomodoros}</span>
      </div>
      <div className="text-lg text-gray-300">
        <span className="font-semibold">現在のサイクル:</span>{" "}
        <span className="text-white">{currentCycle}</span>
      </div>
      <div className="text-lg text-gray-300">
        <span className="font-semibold">休憩達成率:</span>{" "}
        <span className="text-white">{breakCompletionRate}%</span>
      </div>
    </div>
  );
}
