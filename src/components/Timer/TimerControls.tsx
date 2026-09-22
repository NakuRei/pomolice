import { useTimerStore, TimerStatus } from "@/store/timerStore";
import { PlayIcon } from "@/components/icons/AppIcons";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { Button } from "../shared/Button";

/**
 * TimerControls Component
 * Holds the single footer action: start while idle, emergency stop while running.
 */
export function TimerControls(): React.ReactElement {
  const status = useTimerStore((state) => state.status);
  const start = useTimerStore((state) => state.start);

  const isIdle = status === TimerStatus.IDLE;
  const isWorking = status === TimerStatus.WORKING;

  const handleStart = (): void => {
    // Errors from start() are reported to errorStore by timerStore — the home
    // screen's toast stack handles display.
    start().catch(() => undefined);
  };

  return (
    <div className="flex min-h-14 items-center">
      {isIdle && (
        <Button
          appearance="primary"
          size="md"
          shape="pill"
          onClick={handleStart}
          testId="start-button"
          aria-label="タイマーを開始"
          className="gap-2.5 tracking-wider shadow-2xl shadow-primary-800/45"
        >
          <PlayIcon className="size-5" />
          開始
        </Button>
      )}
      {isWorking && <EmergencyStopButton />}
    </div>
  );
}
