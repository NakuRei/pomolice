import { useState } from "react";
import { useTimerStore, SessionType } from "@/store/timerStore";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import { formatTime } from "@/formatters/timeFormatter";
import { getRandomBreakMotivationalMessage } from "@/data/motivationalMessages";
import { BreakStats } from "./BreakStats";
import { MotivationalMessage } from "./MotivationalMessage";
import { SkipButton } from "./SkipButton";
import { AwayDuringBreakControls } from "./AwayDuringBreakControls";
import SkipFlowModal from "../SkipFlow/SkipFlowModal";
import { SettingsSyncBanner } from "../shared/SettingsSyncBanner";
import { EmergencyShortcutHint } from "../shared/EmergencyShortcutNotice";

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
  const isActive = useSkipFlowStore((state) => state.isActive);
  const startSkipFlow = useSkipFlowStore((state) => state.startSkipFlow);
  const resetSkipFlow = useSkipFlowStore((state) => state.resetSkipFlow);
  const emergencyStop = useTimerStore((state) => state.emergencyStop);
  const [motivationalMessage] = useState<string>(() =>
    getRandomBreakMotivationalMessage(),
  );

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
      className="absolute inset-0 z-50 flex flex-col items-center overflow-y-auto bg-background text-on-background"
      data-testid="fullscreen-break"
    >
      <div className="w-full shrink-0 px-4 pt-4">
        <div className="mx-auto max-w-3xl">
          <SettingsSyncBanner />
        </div>
      </div>

      <div className="my-auto flex shrink-0 flex-col items-center px-4 py-8">
        {/* Session type */}
        <div className="mb-6 text-3xl font-medium tracking-wider text-text-muted uppercase">
          {getSessionTypeText()}
        </div>

        {/* Large countdown timer */}
        <div
          className="mb-8 text-[120px] font-bold tracking-tight text-text tabular-nums"
          data-testid="break-timer-display"
          aria-live="polite"
          aria-atomic="true"
        >
          {formattedTime}
        </div>

        {/* Motivational message */}
        <MotivationalMessage message={motivationalMessage} />

        {/* Break statistics */}
        <div className="mt-8">
          <BreakStats />
        </div>
      </div>

      <div className="grid w-full shrink-0 grid-cols-[1fr_auto_1fr] items-end px-3 pb-3">
        <div className="justify-self-start">
          <EmergencyShortcutHint />
        </div>
        <AwayDuringBreakControls />
        {/* Skip button - positioned in bottom-right corner */}
        <div className="justify-self-end">
          <SkipButton onClick={handleSkipClick} />
        </div>
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
