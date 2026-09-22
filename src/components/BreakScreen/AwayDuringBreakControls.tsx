import { useTimerStore } from "@/store/timerStore";
import { useSettingsStore } from "@/store/settingsStore";
import { Button } from "../shared/Button";

/**
 * Lets the user declare they are away from the desk during a break.
 *
 * The break keeps running and stays locked; only the automatic start of the
 * next work session is suspended, so an unattended machine does not run
 * through work sessions nobody is doing. Available only when the next work
 * would otherwise start automatically.
 */
export function AwayDuringBreakControls(): React.ReactElement | null {
  const postBreakTransitionPolicy = useSettingsStore(
    (state) => state.postBreakTransitionPolicy,
  );
  const isAwayDuringBreak = useTimerStore((state) => state.isAwayDuringBreak);
  const setAwayDuringBreak = useTimerStore((state) => state.setAwayDuringBreak);

  if (postBreakTransitionPolicy !== "start_next_work_automatically") {
    return null;
  }

  // Errors are reported to errorStore by timerStore
  const handleLeaveClick = (): void => {
    setAwayDuringBreak(true).catch(() => undefined);
  };

  const handleReturnClick = (): void => {
    setAwayDuringBreak(false).catch(() => undefined);
  };

  if (!isAwayDuringBreak) {
    return (
      <Button appearance="subtle" size="sm" onClick={handleLeaveClick}>
        離席する
      </Button>
    );
  }

  return (
    <div className="flex items-center gap-3">
      <p role="status" className="text-sm text-text-muted">
        離席中：休憩後は自動で作業を開始しません
      </p>
      <Button appearance="secondary" size="sm" onClick={handleReturnClick}>
        戻りました
      </Button>
    </div>
  );
}
