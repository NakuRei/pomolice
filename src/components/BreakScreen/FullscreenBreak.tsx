import { useState } from "react";
import { useTimerStore, SessionType } from "../../store/timerStore";
import { formatTime } from "../../formatters/timeFormatter";
import { MotivationalMessage } from "./MotivationalMessage";
import { BreakStats } from "./BreakStats";
import { SkipButton } from "./SkipButton";

/**
 * FullscreenBreak Component
 *
 * Displays during break time as a fullscreen overlay.
 * Prevents users from working by covering the entire screen.
 *
 * Features (Phase 3):
 * - Large countdown timer
 * - Motivational messages
 * - Break statistics
 * - Small skip button (triggers simple confirmation)
 *
 * Phase 4 will add: Input monitoring and penalties
 * Phase 5 will add: Multi-step skip confirmation flow
 */
export function FullscreenBreak() {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);
  const [showSkipConfirm, setShowSkipConfirm] = useState(false);

  const formattedTime = formatTime(remainingSeconds);

  // Get session type display text
  const getSessionTypeText = () => {
    switch (sessionType) {
      case SessionType.SHORT_BREAK:
        return "短い休憩";
      case SessionType.LONG_BREAK:
        return "長い休憩";
      default:
        return "休憩";
    }
  };

  // Handle skip button click
  const handleSkipClick = () => {
    setShowSkipConfirm(true);
  };

  // Handle skip confirmation
  const handleConfirmSkip = () => {
    // Phase 3: Simple skip (just end the break)
    // Phase 5 will replace this with multi-step flow
    const timerStore = useTimerStore.getState();
    timerStore.completeSession(); // Force complete the break
    setShowSkipConfirm(false);
  };

  // Handle skip cancel
  const handleCancelSkip = () => {
    setShowSkipConfirm(false);
  };

  return (
    <div
      className="fixed inset-0 z-50 bg-gray-900 flex flex-col items-center justify-center"
      data-testid="fullscreen-break"
    >
      {/* Session type */}
      <div className="text-3xl font-medium text-gray-300 uppercase tracking-wider mb-6">
        {getSessionTypeText()}
      </div>

      {/* Large countdown timer */}
      <div
        className="text-[120px] font-bold text-white tabular-nums tracking-tight mb-8"
        data-testid="break-timer-display"
        aria-live="polite"
        aria-atomic="true"
      >
        {formattedTime}
      </div>

      {/* Motivational message */}
      <MotivationalMessage />

      {/* Break statistics */}
      <div className="mt-8">
        <BreakStats />
      </div>

      {/* Skip button - positioned in bottom-right corner */}
      <div className="absolute bottom-3 right-3">
        <SkipButton onClick={handleSkipClick} />
      </div>

      {/* Skip confirmation dialog (Phase 3: simple version) */}
      {showSkipConfirm && (
        <div className="absolute inset-0 bg-black bg-opacity-50 flex items-center justify-center">
          <div className="bg-gray-800 rounded-lg p-8 max-w-md">
            <h2 className="text-2xl font-bold text-white mb-4">
              休憩をスキップしますか?
            </h2>
            <p className="text-gray-300 mb-6">
              休憩は健康と生産性のために重要です。
            </p>
            <div className="flex gap-4 justify-end">
              <button
                onClick={handleCancelSkip}
                className="px-6 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded transition-colors"
              >
                キャンセル
              </button>
              <button
                onClick={handleConfirmSkip}
                className="px-6 py-2 bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
                data-testid="confirm-skip-button"
              >
                スキップ
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
