import { useTimerStore, SessionType } from "@/store/timerStore";
import { formatTime } from "@/formatters/timeFormatter";

const TIMER_NUMBER_FONT_SIZE = "clamp(52px, min(18vw, 30vh), 220px)";

/**
 * Derived from the bundled JetBrains Mono: ascender 1020, descender -300,
 * capHeight 730, em 1000. Re-measure if the font is ever replaced.
 */
const TIMER_NUMBER_TRIM = {
  marginTop: "-0.13em",
  marginBottom: "-0.14em",
} as const;

const SESSION_PHASE_LABEL = {
  [SessionType.WORK]: "FOCUS",
  [SessionType.SHORT_BREAK]: "BREAK",
  [SessionType.LONG_BREAK]: "LONG",
} as const;

const SESSION_PHASE_ACCESSIBLE_NAME = {
  [SessionType.WORK]: "作業時間",
  [SessionType.SHORT_BREAK]: "短い休憩",
  [SessionType.LONG_BREAK]: "長い休憩",
} as const;

/**
 * TimerDisplay Component
 * Shows the remaining time next to the phase it belongs to.
 */
export function TimerDisplay(): React.ReactElement {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);
  const sessionType = useTimerStore((state) => state.sessionType);

  return (
    <div className="flex items-baseline justify-between gap-4">
      <div
        className="leading-none font-light tracking-tighter text-primary-50 tabular-nums"
        style={{ fontSize: TIMER_NUMBER_FONT_SIZE, ...TIMER_NUMBER_TRIM }}
        data-testid="timer-display"
        aria-live="polite"
        aria-atomic="true"
      >
        {formatTime(remainingSeconds)}
      </div>

      <div
        className="font-normal tracking-[0.2em] text-primary-400"
        style={{ fontSize: `calc(${TIMER_NUMBER_FONT_SIZE} / 6)` }}
        data-testid="timer-phase-label"
        aria-label={SESSION_PHASE_ACCESSIBLE_NAME[sessionType]}
      >
        {SESSION_PHASE_LABEL[sessionType]}
      </div>
    </div>
  );
}
