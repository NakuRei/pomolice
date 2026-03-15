import { useTimerStore } from "../../store/timerStore";

/**
 * BreakStats Component
 *
 * Displays statistics during break screen:
 * - Current session count
 * - Current cycle number
 */
export function BreakStats() {
  const { completedPomodoros, currentCycle } = useTimerStore();

  return (
    <div className="text-center space-y-3">
      <div className="text-lg text-text-muted">
        <span className="font-semibold">今日のポモドーロ:</span>{" "}
        <span className="text-text">{completedPomodoros}</span>
      </div>
      <div className="text-lg text-text-muted">
        <span className="font-semibold">現在のサイクル:</span>{" "}
        <span className="text-text">{currentCycle}</span>
      </div>
    </div>
  );
}
