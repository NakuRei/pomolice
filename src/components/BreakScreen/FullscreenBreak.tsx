import { useEffect } from "react";
import { useTimerStore, SessionType } from "../../store/timerStore";
import { useSkipFlowStore } from "../../store/skipFlowStore";
import { formatTime } from "../../formatters/timeFormatter";
import { MotivationalMessage } from "./MotivationalMessage";
import { BreakStats } from "./BreakStats";
import { SkipButton } from "./SkipButton";
import SkipFlowModal from "../SkipFlow/SkipFlowModal";

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
 * Phase 4: Input monitoring and penalties
 * Phase 5: Multi-step skip confirmation flow (10 steps)
 */
export function FullscreenBreak() {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);
  const { isActive, startSkipFlow, resetSkipFlow } = useSkipFlowStore();
  const handleInputDuringBreak = useTimerStore((state) => state.handleInputDuringBreak);


  const formattedTime = formatTime(remainingSeconds);

  useEffect(() => {
    const handleKeyDown = () => {
      handleInputDuringBreak();
    };

    const handleMouseMove = () => {
      handleInputDuringBreak();
    };

    const handleMouseDown = () => {
      handleInputDuringBreak();
    };

    window.addEventListener("keydown", handleKeyDown);
    window.addEventListener("mousemove", handleMouseMove);
    window.addEventListener("mousedown", handleMouseDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
      window.removeEventListener("mousemove", handleMouseMove);
      window.removeEventListener("mousedown", handleMouseDown);
    };
  }, [handleInputDuringBreak]);

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

  // Handle skip button click - start the 10-step skip flow
  const handleSkipClick = () => {
    startSkipFlow();
  };

  // Handle skip flow completion - user completed all 10 steps
  const handleSkipComplete = () => {
    // User successfully completed all 10 steps, skip the break
    const timerStore = useTimerStore.getState();

    // TODO Phase 6: Record skip in statistics
    // statisticsStore.recordBreakSkip(skipFlowStore.totalTimeSpent, skipFlowStore.stepRetries);

    // Exit break mode and complete session
    timerStore.exitBreakMode().then(() => {
      timerStore.completeSession();
    });

    // Reset skip flow
    resetSkipFlow();
  };

  // Handle skip flow cancel - user gave up and decided to take break
  const handleSkipCancel = () => {
    resetSkipFlow();
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

      {/* Phase 5: Multi-step skip confirmation flow (10 steps) */}
      {isActive && (
        <SkipFlowModal
          onComplete={handleSkipComplete}
          onCancel={handleSkipCancel}
        />
      )}
    </div>
  );
}
