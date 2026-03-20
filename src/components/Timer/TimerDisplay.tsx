import { useTimerStore, SessionType } from "../../store/timerStore";
import { formatTime } from "../../formatters/timeFormatter";

/**
 * TimerDisplay Component
 * Displays the countdown timer in large, readable format (MM:SS)
 *
 * Phase 2: Updated to support work, short break, and long break sessions
 */
export function TimerDisplay(): React.ReactElement {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);
  const status = useTimerStore((state) => state.status);

  const formattedTime = formatTime(remainingSeconds);

  // Get session type display text
  const getSessionTypeText = (): "短い休憩" | "長い休憩" | "作業時間" => {
    switch (sessionType) {
      case SessionType.WORK:
        return "作業時間";
      case SessionType.SHORT_BREAK:
        return "短い休憩";
      case SessionType.LONG_BREAK:
        return "長い休憩";
      default:
        return "作業時間";
    }
  };

  return (
    <div className="flex flex-col items-center justify-center space-y-4">
      {/* Session type indicator */}
      <div className="text-xl font-medium tracking-wider text-text-muted uppercase">
        {getSessionTypeText()}
      </div>

      {/* Large timer display */}
      <div
        className="font-['Geist_Mono'] text-8xl font-bold tracking-tight tabular-nums"
        data-testid="timer-display"
        aria-live="polite"
        aria-atomic="true"
      >
        {formattedTime}
      </div>

      {/* Status indicator */}
      <div className="text-sm tracking-wide text-text-muted uppercase">
        {status === "idle" && "待機中"}
        {status === "working" && "実行中"}
        {status === "on_break" && "休憩中"}
      </div>
    </div>
  );
}
