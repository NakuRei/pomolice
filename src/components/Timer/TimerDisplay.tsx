import { useTimerStore } from '../../store/timerStore';
import { formatTime } from '../../formatters/timeFormatter';

/**
 * TimerDisplay Component
 * Displays the countdown timer in large, readable format (MM:SS)
 *
 * Phase 1: Basic timer display with session type indicator
 */
export function TimerDisplay() {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);
  const status = useTimerStore((state) => state.status);

  const formattedTime = formatTime(remainingSeconds);

  return (
    <div className="flex flex-col items-center justify-center space-y-4">
      {/* Session type indicator */}
      <div className="text-xl font-medium text-gray-600 uppercase tracking-wider">
        {sessionType === 'work' ? '作業時間' : '休憩時間'}
      </div>

      {/* Large timer display */}
      <div
        className="text-8xl font-bold tabular-nums tracking-tight"
        data-testid="timer-display"
        aria-live="polite"
        aria-atomic="true"
      >
        {formattedTime}
      </div>

      {/* Status indicator */}
      <div className="text-sm text-gray-500 uppercase tracking-wide">
        {status === 'idle' && '待機中'}
        {status === 'running' && '実行中'}
        {status === 'paused' && '一時停止'}
      </div>
    </div>
  );
}
