import { useState } from 'react';
import { useTimerStore, TimerStatus } from '../../store/timerStore';
import { InlineError } from '../shared/InlineError';

/**
 * TimerControls Component
 * Provides start button and emergency stop button for timer control
 *
 * Phase 2: Start button (IDLE) and Emergency stop button (RUNNING) shown in same position
 */
export function TimerControls() {
  const status = useTimerStore((state) => state.status);
  const start = useTimerStore((state) => state.start);
  const emergencyStop = useTimerStore((state) => state.emergencyStop);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);

  const isIdle = status === TimerStatus.IDLE;
  const isWorking = status === TimerStatus.WORKING;

  const handleStart = () => {
    // Errors from start() are reported to errorStore by timerStore — InlineError handles display
    start().catch(() => undefined);
  };

  const handleEmergencyStopClick = () => {
    setShowConfirmDialog(true);
  };

  const handleConfirm = () => {
    // Keep the dialog open on failure so the user can see the error and retry
    emergencyStop().then(() => setShowConfirmDialog(false)).catch(() => undefined);
  };

  const handleCancel = () => {
    setShowConfirmDialog(false);
  };

  return (
    <>
      <div className="flex flex-col items-center gap-2 min-h-[4rem]">
        {isIdle && (
          <>
            <button
              onClick={handleStart}
              className="px-8 py-4 text-lg font-semibold text-white bg-blue-600 rounded-lg hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 transition-colors"
              data-testid="start-button"
              aria-label="タイマーを開始"
            >
              開始
            </button>
            {/* Shown when start_work fails; auto-clears after 5 seconds */}
            <InlineError errorId="start_work" />
          </>
        )}
        {isWorking && (
          <button
            onClick={handleEmergencyStopClick}
            className="text-xs text-gray-400 hover:text-red-600 transition-colors underline"
            data-testid="emergency-stop-button"
            aria-label="緊急停止"
          >
            緊急停止
          </button>
        )}
      </div>

      {showConfirmDialog && (
        <div
          className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
          data-testid="emergency-stop-dialog"
        >
          <div className="bg-white rounded-lg shadow-xl p-6 max-w-md w-full mx-4">
            <h2 className="text-xl font-bold mb-4 text-gray-900">
              緊急停止しますか？
            </h2>
            <p className="text-gray-600 mb-4">
              現在のポモドーロセッションは記録されません。
              <br />
              緊急時以外は、セッション完了までお待ちください。
            </p>
            {/* Dialog stays open on failure; error shown here with retry button */}
            <div className="mb-4">
              <InlineError errorId="emergency_stop" onRetry={handleConfirm} />
            </div>
            <div className="flex gap-3 justify-end">
              <button
                onClick={handleCancel}
                className="px-4 py-2 text-gray-700 bg-gray-200 rounded hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-400 transition-colors"
                data-testid="emergency-stop-cancel"
              >
                キャンセル
              </button>
              <button
                onClick={handleConfirm}
                className="px-4 py-2 text-white bg-red-600 rounded hover:bg-red-700 focus:outline-none focus:ring-2 focus:ring-red-500 transition-colors"
                data-testid="emergency-stop-confirm"
              >
                停止する
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
