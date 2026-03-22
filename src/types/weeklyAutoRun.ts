/**
 * Weekday keys normalized for IPC payloads and frontend state.
 */
export type WeekdayKey = "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun";

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
 * const weeklyConfig = createWeeklyAutoRunConfigAllDaysDisabled();
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
 * Validation errors scoped to one weekday slot.
 *
 * Usage example:
 * ```ts
 * const equalBoundaryError: DayScheduleValidationError = {
 *   code: "start_time_equals_end_time",
 *   weekday: "wed",
 *   startTime: "12:00",
 *   endTime: "12:00",
 *   message: "Start and end time cannot be identical.",
 * };
 * ```
 */
export type DayScheduleValidationError =
  | {
      code: "start_time_format_invalid";
      weekday: WeekdayKey;
      invalidStartTimeText: string;
      message: string;
    }
  | {
      code: "end_time_format_invalid";
      weekday: WeekdayKey;
      invalidEndTimeText: string;
      message: string;
    }
  | {
      code: "start_time_equals_end_time";
      weekday: WeekdayKey;
      startTime: string;
      endTime: string;
      message: string;
    };

/**
 * Validation errors that require comparing multiple weekday slots.
 *
 * Usage example:
 * ```ts
 * const overlapError: WeeklyScheduleValidationError = {
 *   code: "overlapping_windows_between_days",
 *   primaryWeekday: "mon",
 *   secondaryWeekday: "tue",
 *   message: "Monday overnight range overlaps Tuesday daytime range.",
 * };
 * ```
 */
export interface WeeklyScheduleValidationError {
  /** Error kind used by UI to render stable messaging. */
  code: "overlapping_windows_between_days";
  /** First weekday participating in the overlap. */
  primaryWeekday: WeekdayKey;
  /** Second weekday participating in the overlap. */
  secondaryWeekday: WeekdayKey;
  /** Human-readable error text. */
  message: string;
}

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

const DEFAULT_DAY_AUTO_RUN_SLOT: DayAutoRunSlot = {
  enabled: false,
  startTime: "09:00",
  endTime: "18:00",
};

/**
 * Create a predictable default schedule with all weekdays disabled.
 */
export function createWeeklyAutoRunConfigAllDaysDisabled(): WeeklyAutoRunConfig {
  return {
    mon: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    tue: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    wed: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    thu: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    fri: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    sat: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
    sun: { ...DEFAULT_DAY_AUTO_RUN_SLOT },
  };
}

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
