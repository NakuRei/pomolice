import { useBackendObservationStore } from "@/backend/observationStore";
import { deriveCycleSegments } from "./cycleSegments";

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
export function CycleProgress(): React.ReactElement | null {
  const currentCycle = useBackendObservationStore(
    (state) => state.currentCycle,
  );
  const sessionType = useBackendObservationStore((state) => state.sessionType);
  const remainingSeconds = useBackendObservationStore(
    (state) => state.remainingSeconds,
  );
  const sessionDurationSeconds = useBackendObservationStore(
    (state) => state.sessionDurationSeconds,
  );
  const longBreakInterval = useBackendObservationStore(
    (state) => state.settings?.longBreakInterval ?? null,
  );

  if (
    longBreakInterval === null ||
    currentCycle === null ||
    sessionType === null ||
    remainingSeconds === null ||
    sessionDurationSeconds === null
  ) {
    return null;
  }

  const segments = deriveCycleSegments({
    currentCycle,
    sessionType,
    remainingSeconds,
    sessionDurationSeconds,
    longBreakInterval,
  });
  const displayedCycle = Math.min(currentCycle, longBreakInterval);

  const ariaLabel =
    currentCycle === 0
      ? `ポモドーロ 開始前 全 ${String(longBreakInterval)} サイクル`
      : `ポモドーロ ${String(displayedCycle)} / ${String(longBreakInterval)}`;

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
