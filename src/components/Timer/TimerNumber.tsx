import { useTimerStore } from "@/store/timerStore";
import { formatTime } from "@/formatters/timeFormatter";

export const TIMER_NUMBER_FONT_SIZE = "clamp(52px, min(18vw, 30vh), 220px)";

/**
 * Derived from the bundled JetBrains Mono: ascender 1020, descender -300,
 * capHeight 730, em 1000. Re-measure if the font is ever replaced.
 */
const TIMER_NUMBER_TRIM = {
  marginTop: "-0.13em",
  marginBottom: "-0.14em",
} as const;

export function TimerNumber(): React.ReactElement {
  const remainingSeconds = useTimerStore((state) => state.remainingSeconds);

  return (
    <div
      className="leading-none font-light tracking-tighter text-primary-50 tabular-nums"
      style={{ fontSize: TIMER_NUMBER_FONT_SIZE, ...TIMER_NUMBER_TRIM }}
      data-testid="timer-display"
      aria-live="polite"
      aria-atomic="true"
    >
      {formatTime(remainingSeconds)}
    </div>
  );
}
