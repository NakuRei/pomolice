import { useBackendObservationStore } from "@/backend/observationStore";
import { startWork } from "@/features/timer/commands";

import { PlayIcon } from "@/components/icons/AppIcons";
import { EmergencyStopButton } from "./EmergencyStopButton";
import { Button } from "@/components/shared/Button";

/**
 * TimerControls Component
 * Holds the single footer action: start while idle, emergency stop while running.
 */
export function TimerControls(): React.ReactElement {
  const status = useBackendObservationStore((state) => state.status);

  const isIdle = status === "idle";
  const isWorking = status === "working";

  const handleStart = (): void => {
    void startWork();
  };

  return (
    <div className="flex min-h-14 items-center">
      {isIdle && (
        <Button
          appearance="primary"
          onClick={handleStart}
          testId="start-button"
          aria-label="タイマーを開始"
          className="shadow-2xl shadow-accent-glow"
        >
          <PlayIcon className="size-5" />
          開始
        </Button>
      )}
      {isWorking && <EmergencyStopButton />}
    </div>
  );
}
