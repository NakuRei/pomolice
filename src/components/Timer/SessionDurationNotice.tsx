import type { SessionType } from "@/ipc/generated";
import { useTimerStore } from "@/store/timerStore";

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
  const status = useTimerStore((state) => state.status);
  const sessionType = useTimerStore((state) => state.sessionType);
  const sessionDurationSeconds = useTimerStore(
    (state) => state.sessionDurationSeconds,
  );
  const settings = useTimerStore((state) => state.settings);

  if (settings === null || status === "idle") {
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
