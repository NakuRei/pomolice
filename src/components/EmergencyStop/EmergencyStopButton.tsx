import { useState } from "react";
import { useTimerStore, TimerStatus } from "../../store/timerStore";

/**
 * EmergencyStopButton Component
 * Provides emergency stop functionality with confirmation dialog
 *
 * Phase 2: Emergency stop is available at any time during timer execution
 * - Shows confirmation dialog before stopping
 * - Current session is invalidated (not recorded in statistics)
 * - No penalty applied (emergency situations are respected)
 */
export function EmergencyStopButton() {
  const status = useTimerStore((state) => state.status);
  const emergencyStop = useTimerStore((state) => state.emergencyStop);
  const [showConfirmDialog, setShowConfirmDialog] = useState(false);

  const isRunning = status === TimerStatus.RUNNING;

  const handleEmergencyStopClick = () => {
    setShowConfirmDialog(true);
  };

  const handleConfirm = () => {
    emergencyStop();
    setShowConfirmDialog(false);
  };

  const handleCancel = () => {
    setShowConfirmDialog(false);
  };

  // Don't show button if timer is not running
  if (!isRunning) {
    return null;
  }

  return (
    <>
      <button
        onClick={handleEmergencyStopClick}
        className="text-xs text-gray-400 hover:text-red-600 transition-colors underline"
        data-testid="emergency-stop-button"
        aria-label="緊急停止"
      >
        緊急停止
      </button>

      {showConfirmDialog && (
        <div
          className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
          data-testid="emergency-stop-dialog"
        >
          <div className="bg-white rounded-lg shadow-xl p-6 max-w-md w-full mx-4">
            <h2 className="text-xl font-bold mb-4 text-gray-900">
              緊急停止しますか？
            </h2>
            <p className="text-gray-600 mb-6">
              現在のポモドーロセッションは記録されません。
              <br />
              緊急時以外は、セッション完了までお待ちください。
            </p>
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
