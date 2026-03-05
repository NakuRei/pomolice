import { useTimerStore } from '../../store/timerStore';
import { InlineError } from '../shared/InlineError';

/**
 * ContinueDialog Component
 * Shows dialog after break completion asking if user wants to continue
 *
 * Phase 2: Displays after break session ends
 * - "続ける" button: Starts next work session
 * - "終了" button: Returns to idle state
 */
export function ContinueDialog() {
  const requiresManualContinueDecision = useTimerStore((state) => state.requiresManualContinueDecision);
  const completedPomodoros = useTimerStore((state) => state.completedPomodoros);
  const resolvePostBreakDecision = useTimerStore(
    (state) => state.resolvePostBreakDecision
  );

  if (!requiresManualContinueDecision) {
    return null;
  }

  const handleContinue = () => {
    // Errors are reported to errorStore by timerStore — InlineError handles display
    resolvePostBreakDecision("continue").catch(() => undefined);
  };

  const handleEnd = () => {
    // Errors are reported to errorStore by timerStore — InlineError handles display
    resolvePostBreakDecision("stop_cycle").catch(() => undefined);
  };

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      data-testid="continue-dialog"
    >
      <div className="bg-white rounded-lg shadow-xl p-8 max-w-md w-full mx-4">
        <h2 className="text-2xl font-bold mb-4 text-gray-900">
          1サイクル完了しました！
        </h2>
        <p className="text-gray-600 mb-2">
          お疲れ様でした。休憩が完了しました。
        </p>
        <p className="text-gray-600 mb-6">
          完了したポモドーロ: <span className="font-semibold">{completedPomodoros}</span>
        </p>
        <p className="text-gray-700 font-medium mb-4">
          次のポモドーロを続けますか？
        </p>
        <p
          className="text-sm text-amber-800 bg-amber-50 border border-amber-200 rounded-md px-3 py-2 mb-4"
          data-testid="continue-dialog-manual-decision-message"
        >
          現在、手動判断待ちです。続けるか終了を選んでください。
        </p>
        {/* Error area: shown when manual decision command fails */}
        <div className="mb-4">
          <InlineError errorId="resolve_post_break_decision" />
        </div>
        <div className="flex gap-4">
          <button
            onClick={handleEnd}
            className="flex-1 px-6 py-3 text-gray-700 bg-gray-200 rounded-lg hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-400 transition-colors"
            data-testid="continue-dialog-end"
          >
            終了
          </button>
          <button
            onClick={handleContinue}
            className="flex-1 px-6 py-3 text-white bg-blue-600 rounded-lg hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-colors"
            data-testid="continue-dialog-continue"
          >
            続ける
          </button>
        </div>
      </div>
    </div>
  );
}
