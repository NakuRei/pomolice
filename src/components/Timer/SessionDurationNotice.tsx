import { useTimerObservationStore } from "@/store/timerObservationStore";
import type { SessionType } from "@/ipc/generated";

import type { Settings } from "@/types/settings";
import { formatDurationText } from "../shared/durationText";

function configuredDurationSeconds(
  settings: Settings,
  sessionType: SessionType,
): number {
  const durationSecondsBySessionType: Record<SessionType, number> = {
    work: settings.workDurationSecs,
    short_break: settings.shortBreakDurationSecs,
    long_break: settings.longBreakDurationSecs,
  };
  return durationSecondsBySessionType[sessionType];
}

/**
 * A running session keeps the length it started with. When settings saved
 * since then give the next session of this kind another length, both are
 * shown so the countdown does not look wrong.
 */
export function SessionDurationNotice(): React.ReactElement | null {
  const status = useTimerObservationStore((state) => state.status);
  const sessionType = useTimerObservationStore((state) => state.sessionType);
  const sessionDurationSeconds = useTimerObservationStore(
    (state) => state.sessionDurationSeconds,
  );
  const settings = useTimerObservationStore((state) => state.settings);

  if (
    settings === null ||
    status === null ||
    status === "idle" ||
    sessionType === null ||
    sessionDurationSeconds === null
  ) {
    return null;
  }

  const nextDurationSeconds = configuredDurationSeconds(settings, sessionType);
  if (nextDurationSeconds === sessionDurationSeconds) {
    return null;
  }

  return (
    <p
      className="text-xs text-text-muted"
      data-testid="session-duration-notice"
    >
      このセッションは{formatDurationText(sessionDurationSeconds)}
      ・次回から{formatDurationText(nextDurationSeconds)}
    </p>
  );
}
