import { useTimerStore, TimerStatus } from '../../store/timerStore';

/**
 * TimerControls Component
 * Provides start button for timer control
 *
 * Phase 2: Only start button shown. Stop button removed per specification.
 * Emergency stop is available via separate component (EmergencyStopButton)
 */
export function TimerControls() {
  const status = useTimerStore((state) => state.status);
  const start = useTimerStore((state) => state.start);

  const isIdle = status === TimerStatus.IDLE;

  const handleStart = () => {
    start();
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
    </div>
  );
}
