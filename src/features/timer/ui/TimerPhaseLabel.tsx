import type { SessionType } from "@/ipc/generated";

import { TIMER_NUMBER_FONT_SIZE } from "./TimerNumber";

const SESSION_PHASE_ACCESSIBLE_NAME = {
  work: "作業時間",
  short_break: "短い休憩",
  long_break: "長い休憩",
} as const;

interface TimerPhaseLabelProps {
  sessionType: SessionType;
  className?: string;
  /** Short on-screen text; the accessible name comes from the session type. */
  children: string;
}

export function TimerPhaseLabel({
  sessionType,
  className = "",
  children,
}: TimerPhaseLabelProps): React.ReactElement {
  return (
    <div
      className={`font-normal text-accent ${className}`}
      style={{ fontSize: `calc(${TIMER_NUMBER_FONT_SIZE} / 6)` }}
      aria-label={SESSION_PHASE_ACCESSIBLE_NAME[sessionType]}
    >
      {children}
    </div>
  );
}
