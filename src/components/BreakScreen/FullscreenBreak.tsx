import { useTimerStore, SessionType } from "@/store/timerStore";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import { formatTime } from "@/formatters/timeFormatter";
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
 * Features:
 * - Large countdown timer (state from backend)
 * - Motivational messages
 * - Break statistics
 * - Small skip button (triggers multi-step confirmation)
 *
 * Note: Input monitoring and penalties are handled entirely by the backend.
 * This component is display-only.
 */
export function FullscreenBreak(): React.ReactElement {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);
  const { isActive, startSkipFlow, resetSkipFlow } = useSkipFlowStore();
  const emergencyStop = useTimerStore((state) => state.emergencyStop);

  const formattedTime = formatTime(remainingSeconds);

  // Get session type display text
  const getSessionTypeText = (): "短い休憩" | "長い休憩" | "休憩" => {
    switch (sessionType) {
      case SessionType.WORK:
        return "休憩";
      case SessionType.SHORT_BREAK:
        return "短い休憩";
      case SessionType.LONG_BREAK:
        return "長い休憩";
      default:
        return "休憩";
    }
  };

  // Handle skip button click - start the multi-step skip flow
  const handleSkipClick = (): void => {
    startSkipFlow();
  };

  // Handle skip flow completion - user completed all steps
  const handleSkipComplete = (): void => {
    // User successfully completed all steps, skip the break via emergency stop
    // This will exit fullscreen and reset to idle state
    // Errors are reported to errorStore by timerStore
    emergencyStop().catch(() => undefined);

    // Reset skip flow
    resetSkipFlow();
  };

  // Handle skip flow cancel - user gave up and decided to take break
  const handleSkipCancel = (): void => {
    resetSkipFlow();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-background text-on-background"
      data-testid="fullscreen-break"
    >
      {/* Session type */}
      <div className="mb-6 text-3xl font-medium tracking-wider text-text-muted uppercase">
        {getSessionTypeText()}
      </div>

      {/* Large countdown timer */}
      <div
        className="mb-8 font-['Geist_Mono'] text-[120px] font-bold tracking-tight text-text tabular-nums"
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
      <div className="absolute right-3 bottom-3">
        <SkipButton onClick={handleSkipClick} />
      </div>

      {/* Multi-step skip confirmation flow */}
      {isActive && (
        <SkipFlowModal
          onComplete={handleSkipComplete}
          onCancel={handleSkipCancel}
        />
      )}
    </div>
  );
}
