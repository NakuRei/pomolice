import { useTimerStore, TimerStatus } from "@/store/timerStore";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { Button } from "../shared/Button";
import { InlineError } from "../shared/InlineError";

/**
 * TimerControls Component
 * Provides start button and emergency stop button for timer control
 *
 * Phase 2: Start button (IDLE) and Emergency stop button (RUNNING) shown in same position
 */
export function TimerControls(): React.ReactElement {
  const status = useTimerStore((state) => state.status);
  const start = useTimerStore((state) => state.start);

  const isIdle = status === TimerStatus.IDLE;
  const isWorking = status === TimerStatus.WORKING;

  const handleStart = (): void => {
    // Errors from start() are reported to errorStore by timerStore — InlineError handles display
    start().catch(() => undefined);
  };

  return (
    <div className="flex min-h-16 flex-col items-center gap-2">
      {isIdle && (
        <>
          <Button
            appearance="primary"
            size="lg"
            onClick={handleStart}
            testId="start-button"
            aria-label="タイマーを開始"
          >
            開始
          </Button>
          {/* Shown when start_work fails; auto-clears after 5 seconds */}
          <InlineError errorId="start_work" />
        </>
      )}
      {isWorking && <EmergencyStopButton />}
    </div>
  );
}
