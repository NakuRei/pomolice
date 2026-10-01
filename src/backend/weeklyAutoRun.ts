import type { WeekdayKey } from "@/ipc/generated";

/**
 * Rust `chrono::Weekday` tokens used in serialized payloads.
 */
export type ChronoWeekdayToken =
  | "Mon"
  | "Tue"
  | "Wed"
  | "Thu"
  | "Fri"
  | "Sat"
  | "Sun";

type LowercaseChronoWeekdayToken = Lowercase<ChronoWeekdayToken>;

/**
 * A single weekday slot for scheduled auto-run.
 *
 * Usage example:
 * ```ts
 * const mondaySlot: DayAutoRunSlot = {
 *   enabled: true,
 *   startTime: "09:00",
 *   endTime: "18:00",
 * };
 * ```
 */
export interface DayAutoRunSlot {
  /** Whether this weekday slot can start/stop timers automatically. */
  enabled: boolean;
  /** Start time in strict `HH:MM` 24-hour format. */
  startTime: string;
  /** End time in strict `HH:MM` 24-hour format. */
  endTime: string;
}

/**
 * Weekly schedule keyed by normalized weekday strings.
 *
 * Usage examples:
 * ```ts
 * const weeklyConfig: WeeklyAutoRunConfig = { ...currentConfig };
 *
 * // Normal daytime schedule
 * weeklyConfig.mon = { enabled: true, startTime: "09:00", endTime: "18:00" };
 *
 * // Overnight schedule (crosses midnight)
 * weeklyConfig.fri = { enabled: true, startTime: "22:00", endTime: "02:00" };
 *
 * // Disabled day (times are preserved but ignored)
 * weeklyConfig.sun = { enabled: false, startTime: "09:00", endTime: "18:00" };
 * ```
 */
export type WeeklyAutoRunConfig = Record<WeekdayKey, DayAutoRunSlot>;

/**
 * Source that triggered a work-session start.
 *
 * Usage example:
 * ```ts
 * const source: TimerStartSource = "manual_inside_window";
 * ```
 */
export type TimerStartSource =
  | "manual_outside_window"
  | "manual_inside_window"
  | "scheduled_auto_start";

/**
 * Normalized weekday list used for deterministic iteration order.
 */
export const WEEKDAY_KEYS: readonly WeekdayKey[] = [
  "mon",
  "tue",
  "wed",
  "thu",
  "fri",
  "sat",
  "sun",
];

const WEEKDAY_CONVERSION_MAPPER = {
  dateDayIndexToWeekdayKey: ["sun", "mon", "tue", "wed", "thu", "fri", "sat"],
  weekdayKeyToDateDayIndex: {
    mon: 1,
    tue: 2,
    wed: 3,
    thu: 4,
    fri: 5,
    sat: 6,
    sun: 0,
  },
  chronoWeekdayTokenToWeekdayKey: {
    Mon: "mon",
    Tue: "tue",
    Wed: "wed",
    Thu: "thu",
    Fri: "fri",
    Sat: "sat",
    Sun: "sun",
    mon: "mon",
    tue: "tue",
    wed: "wed",
    thu: "thu",
    fri: "fri",
    sat: "sat",
    sun: "sun",
  },
  weekdayKeyToChronoWeekdayToken: {
    mon: "Mon",
    tue: "Tue",
    wed: "Wed",
    thu: "Thu",
    fri: "Fri",
    sat: "Sat",
    sun: "Sun",
  },
} as const satisfies {
  dateDayIndexToWeekdayKey: readonly WeekdayKey[];
  weekdayKeyToDateDayIndex: Record<WeekdayKey, number>;
  chronoWeekdayTokenToWeekdayKey: Record<
    ChronoWeekdayToken | LowercaseChronoWeekdayToken,
    WeekdayKey
  >;
  weekdayKeyToChronoWeekdayToken: Record<WeekdayKey, ChronoWeekdayToken>;
};

/**
 * Convert `Date.getDay()` output (0-6, Sunday-first) to `WeekdayKey`.
 */
export function convertDateDayIndexToWeekdayKey(dayIndex: number): WeekdayKey {
  if (!Number.isInteger(dayIndex) || dayIndex < 0 || dayIndex > 6) {
    throw new RangeError(
      `Date.getDay() index must be 0-6, received ${String(dayIndex)}.`,
    );
  }

  const weekday = WEEKDAY_CONVERSION_MAPPER.dateDayIndexToWeekdayKey[dayIndex];
  if (weekday === undefined) {
    throw new RangeError(
      `Date.getDay() index must be 0-6, received ${String(dayIndex)}.`,
    );
  }

  return weekday;
}

/**
 * Convert `WeekdayKey` to `Date.getDay()` output (0-6, Sunday-first).
 */
export function convertWeekdayKeyToDateDayIndex(weekday: WeekdayKey): number {
  return WEEKDAY_CONVERSION_MAPPER.weekdayKeyToDateDayIndex[weekday];
}

/**
 * Convert serialized `chrono::Weekday` tokens to normalized `WeekdayKey`.
 */
export function convertChronoWeekdayTokenToWeekdayKey(
  weekdayToken: ChronoWeekdayToken | LowercaseChronoWeekdayToken,
): WeekdayKey {
  return WEEKDAY_CONVERSION_MAPPER.chronoWeekdayTokenToWeekdayKey[weekdayToken];
}

/**
 * Convert normalized `WeekdayKey` to serialized `chrono::Weekday` token.
 */
export function convertWeekdayKeyToChronoWeekdayToken(
  weekday: WeekdayKey,
): ChronoWeekdayToken {
  return WEEKDAY_CONVERSION_MAPPER.weekdayKeyToChronoWeekdayToken[weekday];
}
