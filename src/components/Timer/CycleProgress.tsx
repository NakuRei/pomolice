import { useTimerStore } from "@/store/timerStore";
import { useSettingsStore } from "@/store/settingsStore";
import { deriveCycleSegments } from "@/components/Timer/cycleSegments";

const SECONDS_PER_MINUTE = 60;

const SEGMENT_FILL_CLASS_NAME_BY_STATE = {
  completed: "bg-accent-deep",
  active: "bg-accent",
  upcoming: "bg-accent",
} as const;

/**
 * CycleProgress Component
 * Shows the whole pomodoro cycle as a bar: one span per work session and the
 * break that follows it, filled by how far the running session has come.
 */
export function CycleProgress(): React.ReactElement {
  const currentCycle = useTimerStore((state) => state.currentCycle);
  const sessionType = useTimerStore((state) => state.sessionType);
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const longBreakInterval = useSettingsStore(
    (state) => state.longBreakInterval,
  );
  // A pending settings edit can briefly skew the fill; the bar is decorative.
  const workDuration = useSettingsStore((state) => state.workDuration);
  const shortBreakDuration = useSettingsStore(
    (state) => state.shortBreakDuration,
  );
  const longBreakDuration = useSettingsStore(
    (state) => state.longBreakDuration,
  );

  const segments = deriveCycleSegments({
    currentCycle,
    sessionType,
    remainingSeconds,
    longBreakInterval,
    workDurationSeconds: workDuration * SECONDS_PER_MINUTE,
    shortBreakDurationSeconds: shortBreakDuration * SECONDS_PER_MINUTE,
    longBreakDurationSeconds: longBreakDuration * SECONDS_PER_MINUTE,
  });

  const ariaLabel =
    currentCycle === 0
      ? `ポモドーロ 開始前 全 ${String(longBreakInterval)} サイクル`
      : `ポモドーロ ${String(currentCycle)} / ${String(longBreakInterval)}`;

  return (
    <div className="flex flex-col gap-2">
      <div
        className="flex h-1.5 gap-1 md:h-2.25"
        data-testid="cycle-progress"
        role="img"
        aria-label={ariaLabel}
      >
        {segments.map((segment) => (
          <div
            key={segment.key}
            className="min-w-0 basis-0 overflow-hidden rounded-sm bg-accent-track"
            style={{ flexGrow: segment.flexGrow }}
            data-testid={`cycle-segment-${segment.key}`}
            data-state={segment.state}
          >
            <div
              className={`h-full ${SEGMENT_FILL_CLASS_NAME_BY_STATE[segment.state]}`}
              style={{ width: `${String(segment.fillRatio * 100)}%` }}
            />
          </div>
        ))}
      </div>
    </div>
  );
}
