import { useTimerStore, TimerStatus } from '../../store/timerStore';

/**
 * TimerControls Component
 * Provides start/stop buttons for timer control
 *
 * Phase 1: Basic start and stop functionality (stop is allowed for development)
 * Phase 2+: Stop will be removed, only emergency stop with confirmation dialog
 */
export function TimerControls() {
  const status = useTimerStore((state) => state.status);
  const start = useTimerStore((state) => state.start);
  const stop = useTimerStore((state) => state.stop);

  const isIdle = status === TimerStatus.IDLE;
  const isRunning = status === TimerStatus.RUNNING;

  const handleStart = () => {
    start();
  };

  const handleStop = () => {
    stop();
  };

  return (
    <div className="flex items-center justify-center gap-4">
      {isIdle && (
        <button
          onClick={handleStart}
          className="px-8 py-4 text-lg font-semibold text-white bg-blue-600 rounded-lg hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
          data-testid="start-button"
          aria-label="タイマーを開始"
        >
          開始
        </button>
      )}

      {isRunning && (
        <button
          onClick={handleStop}
          className="px-8 py-4 text-lg font-semibold text-white bg-red-600 rounded-lg hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 transition-colors"
          data-testid="stop-button"
          aria-label="タイマーを停止"
        >
          停止
        </button>
      )}
    </div>
  );
}
