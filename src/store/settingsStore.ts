import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";
import type { PostBreakTransitionPolicy } from "../types/sessionTransitionPolicy";
import {
  WEEKDAY_KEYS,
  convertChronoWeekdayTokenToWeekdayKey,
  convertDateDayIndexToWeekdayKey,
  convertWeekdayKeyToChronoWeekdayToken,
  convertWeekdayKeyToDateDayIndex,
  createWeeklyAutoRunConfigAllDaysDisabled,
  type DayAutoRunSlot,
  type DayScheduleValidationError,
  type TimerStartSource,
  type WeeklyAutoRunConfig,
  type WeeklyScheduleValidationError,
  type WeekdayKey,
} from "../types/weeklyAutoRun";

export type {
  DayAutoRunSlot,
  DayScheduleValidationError,
  TimerStartSource,
  WeeklyAutoRunConfig,
  WeeklyScheduleValidationError,
  WeekdayKey,
};
export {
  WEEKDAY_KEYS,
  convertChronoWeekdayTokenToWeekdayKey,
  convertDateDayIndexToWeekdayKey,
  convertWeekdayKeyToChronoWeekdayToken,
  convertWeekdayKeyToDateDayIndex,
  createWeeklyAutoRunConfigAllDaysDisabled,
};

/**
 * Settings state interface
 * Manages timer duration configurations
 */
interface SettingsState {
  /** Work session duration in minutes (1-120) */
  workDuration: number;
  /** Short break duration in minutes (1-60) */
  shortBreakDuration: number;
  /** Long break duration in minutes (5-60) */
  longBreakDuration: number;
  /** Number of pomodoros before a long break (2-10) */
  longBreakInterval: number;
  /** Notification sound volume percentage (0-100) */
  notificationSoundVolumePercent: number;
  /** Whether notification sound is muted */
  notificationSoundMuted: boolean;
  /** Global toggle for scheduled auto-run behavior. */
  autoRunFeatureEnabled: boolean;
  /** Policy used when a break session completes. */
  postBreakTransitionPolicy: PostBreakTransitionPolicy;
  /** Weekly auto-run schedule keyed by weekday. */
  weeklyAutoRunConfig: WeeklyAutoRunConfig;
  /** Validation errors scoped to each weekday row. */
  dayScheduleValidationErrors: Partial<
    Record<WeekdayKey, DayScheduleValidationError>
  >;
  /** Validation errors that compare multiple weekdays. */
  weeklyScheduleValidationErrors: WeeklyScheduleValidationError[];
}

/**
 * Settings fields that users can edit directly.
 */
type SettingsEditableValues = Pick<
  SettingsState,
  | "workDuration"
  | "shortBreakDuration"
  | "longBreakDuration"
  | "longBreakInterval"
  | "notificationSoundVolumePercent"
  | "notificationSoundMuted"
  | "autoRunFeatureEnabled"
  | "postBreakTransitionPolicy"
  | "weeklyAutoRunConfig"
>;

/**
 * Settings actions interface
 */
interface SettingsActions {
  /** Set work duration with validation */
  setWorkDuration: (minutes: number) => void;
  /** Set short break duration with validation */
  setShortBreakDuration: (minutes: number) => void;
  /** Set long break duration with validation */
  setLongBreakDuration: (minutes: number) => void;
  /** Set long break interval with validation */
  setLongBreakInterval: (count: number) => void;
  /** Set notification sound volume percentage with validation */
  setNotificationSoundVolumePercent: (percent: number) => void;
  /** Toggle notification sound mute state */
  toggleNotificationSoundMuted: () => void;
  /** Enable or disable scheduled auto-run globally. */
  setAutoRunFeatureEnabled: (enabled: boolean) => void;
  /** Set policy for transition behavior after break completion. */
  setPostBreakTransitionPolicy: (policy: PostBreakTransitionPolicy) => void;
  /** Enable or disable one weekday slot. */
  setDayAutoRunEnabled: (weekday: WeekdayKey, enabled: boolean) => void;
  /** Update one weekday start time in `HH:MM` format. */
  setDayStartTime: (weekday: WeekdayKey, time: string) => void;
  /** Update one weekday end time in `HH:MM` format. */
  setDayEndTime: (weekday: WeekdayKey, time: string) => void;
  /** Update multiple settings at once */
  updateSettings: (settings: Partial<SettingsEditableValues>) => void;
  /** Reset all settings to default values */
  resetToDefaults: () => void;
  /** Reset to defaults (alias for testing compatibility) */
  reset: () => void;
}

/**
 * Combined settings store type
 */
type SettingsStore = SettingsState & SettingsActions;

/**
 * Default settings following Pomodoro Technique standards
 */
function createDefaultSettingsState(): SettingsState {
  return {
    workDuration: 25,
    shortBreakDuration: 5,
    longBreakDuration: 15,
    longBreakInterval: 4,
    notificationSoundVolumePercent: 50,
    notificationSoundMuted: false,
    autoRunFeatureEnabled: false,
    postBreakTransitionPolicy: "require_manual_continue",
    weeklyAutoRunConfig: createWeeklyAutoRunConfigAllDaysDisabled(),
    dayScheduleValidationErrors: {},
    weeklyScheduleValidationErrors: [],
  };
}

export const DEFAULT_SETTINGS: SettingsState = createDefaultSettingsState();

/**
 * Validation constraints for settings
 *
 * DEVELOPMENT NOTE: Min values are currently set low for testing Phase 3 fullscreen breaks.
 * TODO: Before production release, restore min values to:
 * - workDuration: { min: 1 }
 * - shortBreakDuration: { min: 1 }
 * - longBreakDuration: { min: 5 }
 */
export const VALIDATION_RULES = {
  workDuration: { min: 0.01, max: 120 }, // TEMP: 0.01 min (0.6 sec) for testing
  shortBreakDuration: { min: 0.01, max: 60 }, // TEMP: 0.01 min (0.6 sec) for testing
  longBreakDuration: { min: 0.01, max: 60 }, // TEMP: 0.01 min (0.6 sec) for testing
  longBreakInterval: { min: 2, max: 10 },
  notificationSoundVolumePercent: { min: 0, max: 100 },
} as const;

const HH_MM_TIME_TEXT_PATTERN = /^(?:[01]\d|2[0-3]):[0-5]\d$/u;
const MINUTES_PER_DAY = 24 * 60;
const WEEKDAY_INDEX_BY_KEY: Record<WeekdayKey, number> = {
  mon: 0,
  tue: 1,
  wed: 2,
  thu: 3,
  fri: 4,
  sat: 5,
  sun: 6,
};

interface ParsedHhMm {
  hour: number;
  minute: number;
  minutesSinceStartOfDay: number;
}

interface WeekMinuteRange {
  startMinuteOfWeek: number;
  endMinuteOfWeek: number;
}

interface ComparableWeekdaySchedule {
  weekday: WeekdayKey;
  ranges: WeekMinuteRange[];
}

/**
 * Result of validating a weekly schedule before save.
 *
 * Usage example:
 * ```ts
 * const result = validateWeeklyAutoRunConfig(config);
 * if (Object.keys(result.dayScheduleValidationErrors).length > 0) {
 *   // render per-day validation messages
 * }
 * ```
 */
export interface WeeklyAutoRunValidationResult {
  dayScheduleValidationErrors: Partial<
    Record<WeekdayKey, DayScheduleValidationError>
  >;
  weeklyScheduleValidationErrors: WeeklyScheduleValidationError[];
}

export function parseHhMm(timeText: string): ParsedHhMm | null {
  if (!HH_MM_TIME_TEXT_PATTERN.test(timeText)) {
    return null;
  }

  const [hourText, minuteText] = timeText.split(":");
  const hour = Number(hourText);
  const minute = Number(minuteText);

  return {
    hour,
    minute,
    minutesSinceStartOfDay: hour * 60 + minute,
  };
}

function cloneWeeklyAutoRunConfig(
  config: WeeklyAutoRunConfig,
): WeeklyAutoRunConfig {
  return {
    mon: { ...config.mon },
    tue: { ...config.tue },
    wed: { ...config.wed },
    thu: { ...config.thu },
    fri: { ...config.fri },
    sat: { ...config.sat },
    sun: { ...config.sun },
  };
}

function createDayScheduleValidationError(
  weekday: WeekdayKey,
  daySlot: DayAutoRunSlot,
): DayScheduleValidationError | undefined {
  if (!daySlot.enabled) {
    return undefined;
  }

  const parsedStart = parseHhMm(daySlot.startTime);
  if (!parsedStart) {
    return {
      code: "start_time_format_invalid",
      weekday,
      invalidStartTimeText: daySlot.startTime,
      message: `Start time for ${weekday} must use HH:MM (24-hour).`,
    };
  }

  const parsedEnd = parseHhMm(daySlot.endTime);
  if (!parsedEnd) {
    return {
      code: "end_time_format_invalid",
      weekday,
      invalidEndTimeText: daySlot.endTime,
      message: `End time for ${weekday} must use HH:MM (24-hour).`,
    };
  }

  if (parsedStart.minutesSinceStartOfDay === parsedEnd.minutesSinceStartOfDay) {
    return {
      code: "start_time_equals_end_time",
      weekday,
      startTime: daySlot.startTime,
      endTime: daySlot.endTime,
      message: `Start and end time cannot be identical for ${weekday}.`,
    };
  }

  return undefined;
}

function createWeekMinuteRanges(
  weekday: WeekdayKey,
  daySlot: DayAutoRunSlot,
): WeekMinuteRange[] {
  const parsedStart = parseHhMm(daySlot.startTime);
  const parsedEnd = parseHhMm(daySlot.endTime);

  if (!parsedStart || !parsedEnd) {
    return [];
  }

  if (parsedStart.minutesSinceStartOfDay === parsedEnd.minutesSinceStartOfDay) {
    return [];
  }

  const weekdayIndex = WEEKDAY_INDEX_BY_KEY[weekday];
  const dayStartMinuteOfWeek = weekdayIndex * MINUTES_PER_DAY;
  const startMinuteOfWeek =
    dayStartMinuteOfWeek + parsedStart.minutesSinceStartOfDay;
  const endMinuteOfWeek =
    dayStartMinuteOfWeek + parsedEnd.minutesSinceStartOfDay;

  if (startMinuteOfWeek < endMinuteOfWeek) {
    return [{ startMinuteOfWeek, endMinuteOfWeek }];
  }

  const nextWeekdayIndex = (weekdayIndex + 1) % WEEKDAY_KEYS.length;
  return [
    {
      startMinuteOfWeek,
      endMinuteOfWeek: dayStartMinuteOfWeek + MINUTES_PER_DAY,
    },
    {
      startMinuteOfWeek: nextWeekdayIndex * MINUTES_PER_DAY,
      endMinuteOfWeek:
        nextWeekdayIndex * MINUTES_PER_DAY + parsedEnd.minutesSinceStartOfDay,
    },
  ];
}

function weekMinuteRangesOverlap(
  firstRange: WeekMinuteRange,
  secondRange: WeekMinuteRange,
): boolean {
  return (
    firstRange.startMinuteOfWeek < secondRange.endMinuteOfWeek &&
    secondRange.startMinuteOfWeek < firstRange.endMinuteOfWeek
  );
}

function collectComparableWeekdaySchedules(
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
  dayScheduleValidationErrors: Partial<
    Record<WeekdayKey, DayScheduleValidationError>
  >,
): ComparableWeekdaySchedule[] {
  const comparableSchedules: ComparableWeekdaySchedule[] = [];

  for (const weekday of WEEKDAY_KEYS) {
    if (!weeklyAutoRunConfig[weekday].enabled) {
      continue;
    }
    if (dayScheduleValidationErrors[weekday] !== undefined) {
      continue;
    }

    const ranges = createWeekMinuteRanges(
      weekday,
      weeklyAutoRunConfig[weekday],
    );
    if (ranges.length === 0) {
      continue;
    }

    comparableSchedules.push({ weekday, ranges });
  }

  return comparableSchedules;
}

function hasOverlappingWeekMinuteRanges(
  firstRanges: WeekMinuteRange[],
  secondRanges: WeekMinuteRange[],
): boolean {
  return firstRanges.some((firstRange) =>
    secondRanges.some((secondRange) =>
      weekMinuteRangesOverlap(firstRange, secondRange),
    ),
  );
}

function createWeeklyScheduleValidationErrors(
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
  dayScheduleValidationErrors: Partial<
    Record<WeekdayKey, DayScheduleValidationError>
  >,
): WeeklyScheduleValidationError[] {
  const comparableSchedules = collectComparableWeekdaySchedules(
    weeklyAutoRunConfig,
    dayScheduleValidationErrors,
  );
  const weeklyValidationErrors: WeeklyScheduleValidationError[] = [];

  for (
    let primaryIndex = 0;
    primaryIndex < comparableSchedules.length;
    primaryIndex += 1
  ) {
    const primarySchedule = comparableSchedules[primaryIndex];

    for (
      let secondaryIndex = primaryIndex + 1;
      secondaryIndex < comparableSchedules.length;
      secondaryIndex += 1
    ) {
      const secondarySchedule = comparableSchedules[secondaryIndex];
      if (
        !hasOverlappingWeekMinuteRanges(
          primarySchedule.ranges,
          secondarySchedule.ranges,
        )
      ) {
        continue;
      }

      weeklyValidationErrors.push({
        code: "overlapping_windows_between_days",
        primaryWeekday: primarySchedule.weekday,
        secondaryWeekday: secondarySchedule.weekday,
        message: `Schedule windows overlap between ${primarySchedule.weekday} and ${secondarySchedule.weekday}.`,
      });
    }
  }

  return weeklyValidationErrors;
}

/**
 * Validate all weekday slots and return both per-day and cross-weekday errors.
 */
export function validateWeeklyAutoRunConfig(
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
): WeeklyAutoRunValidationResult {
  const dayScheduleValidationErrors: Partial<
    Record<WeekdayKey, DayScheduleValidationError>
  > = {};

  for (const weekday of WEEKDAY_KEYS) {
    const dayError = createDayScheduleValidationError(
      weekday,
      weeklyAutoRunConfig[weekday],
    );
    if (dayError) {
      dayScheduleValidationErrors[weekday] = dayError;
    }
  }

  return {
    dayScheduleValidationErrors,
    weeklyScheduleValidationErrors: createWeeklyScheduleValidationErrors(
      weeklyAutoRunConfig,
      dayScheduleValidationErrors,
    ),
  };
}

function writeScheduleValidationResults(settingsState: SettingsState): void {
  const validationResult = validateWeeklyAutoRunConfig(
    settingsState.weeklyAutoRunConfig,
  );
  settingsState.dayScheduleValidationErrors =
    validationResult.dayScheduleValidationErrors;
  settingsState.weeklyScheduleValidationErrors =
    validationResult.weeklyScheduleValidationErrors;
}

/**
 * Validate and clamp a value within min/max range
 * @param value - Value to validate
 * @param min - Minimum allowed value
 * @param max - Maximum allowed value
 * @returns Clamped value within valid range
 *
 * TEMP: Removed Math.floor() to support decimal values for Phase 3 testing
 * TODO: Restore Math.floor() before production if integer-only durations are required
 */
function validateRange(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

/**
 * Helper to conditionally apply devtools middleware only in development
 */
const noopDevtools = (<T>(config: T): T => config) as typeof devtools;
const conditionalDevtools = import.meta.env.DEV ? devtools : noopDevtools;

/**
 * Zustand store for settings management
 * Uses immer middleware for immutable state updates
 * Uses devtools middleware for Redux DevTools integration (development only)
 * Uses persist middleware for localStorage persistence
 */
export const useSettingsStore = create<SettingsStore>()(
  conditionalDevtools(
    persist(
      immer((set) => ({
        ...createDefaultSettingsState(),

        setWorkDuration: (minutes: number): void => {
          set((state) => {
            state.workDuration = validateRange(
              minutes,
              VALIDATION_RULES.workDuration.min,
              VALIDATION_RULES.workDuration.max,
            );
          });
        },

        setShortBreakDuration: (minutes: number): void => {
          set((state) => {
            state.shortBreakDuration = validateRange(
              minutes,
              VALIDATION_RULES.shortBreakDuration.min,
              VALIDATION_RULES.shortBreakDuration.max,
            );
          });
        },

        setLongBreakDuration: (minutes: number): void => {
          set((state) => {
            state.longBreakDuration = validateRange(
              minutes,
              VALIDATION_RULES.longBreakDuration.min,
              VALIDATION_RULES.longBreakDuration.max,
            );
          });
        },

        setLongBreakInterval: (count: number): void => {
          set((state) => {
            state.longBreakInterval = validateRange(
              count,
              VALIDATION_RULES.longBreakInterval.min,
              VALIDATION_RULES.longBreakInterval.max,
            );
          });
        },

        setNotificationSoundVolumePercent: (percent: number): void => {
          set((state) => {
            state.notificationSoundVolumePercent = validateRange(
              percent,
              VALIDATION_RULES.notificationSoundVolumePercent.min,
              VALIDATION_RULES.notificationSoundVolumePercent.max,
            );
          });
        },

        toggleNotificationSoundMuted: (): void => {
          set((state) => {
            state.notificationSoundMuted = !state.notificationSoundMuted;
          });
        },

        setAutoRunFeatureEnabled: (enabled: boolean): void => {
          set((state) => {
            state.autoRunFeatureEnabled = enabled;
          });
        },

        setPostBreakTransitionPolicy: (
          policy: PostBreakTransitionPolicy,
        ): void => {
          set((state) => {
            state.postBreakTransitionPolicy = policy;
          });
        },

        setDayAutoRunEnabled: (weekday: WeekdayKey, enabled: boolean): void => {
          set((state) => {
            state.weeklyAutoRunConfig[weekday].enabled = enabled;
            writeScheduleValidationResults(state);
          });
        },

        setDayStartTime: (weekday: WeekdayKey, time: string): void => {
          set((state) => {
            state.weeklyAutoRunConfig[weekday].startTime = time;
            writeScheduleValidationResults(state);
          });
        },

        setDayEndTime: (weekday: WeekdayKey, time: string): void => {
          set((state) => {
            state.weeklyAutoRunConfig[weekday].endTime = time;
            writeScheduleValidationResults(state);
          });
        },

        updateSettings: (settings: Partial<SettingsEditableValues>): void => {
          set((state) => {
            if (settings.workDuration !== undefined) {
              state.workDuration = validateRange(
                settings.workDuration,
                VALIDATION_RULES.workDuration.min,
                VALIDATION_RULES.workDuration.max,
              );
            }
            if (settings.shortBreakDuration !== undefined) {
              state.shortBreakDuration = validateRange(
                settings.shortBreakDuration,
                VALIDATION_RULES.shortBreakDuration.min,
                VALIDATION_RULES.shortBreakDuration.max,
              );
            }
            if (settings.longBreakDuration !== undefined) {
              state.longBreakDuration = validateRange(
                settings.longBreakDuration,
                VALIDATION_RULES.longBreakDuration.min,
                VALIDATION_RULES.longBreakDuration.max,
              );
            }
            if (settings.longBreakInterval !== undefined) {
              state.longBreakInterval = validateRange(
                settings.longBreakInterval,
                VALIDATION_RULES.longBreakInterval.min,
                VALIDATION_RULES.longBreakInterval.max,
              );
            }
            if (settings.notificationSoundVolumePercent !== undefined) {
              state.notificationSoundVolumePercent = validateRange(
                settings.notificationSoundVolumePercent,
                VALIDATION_RULES.notificationSoundVolumePercent.min,
                VALIDATION_RULES.notificationSoundVolumePercent.max,
              );
            }
            if (settings.notificationSoundMuted !== undefined) {
              state.notificationSoundMuted = settings.notificationSoundMuted;
            }

            if (settings.autoRunFeatureEnabled !== undefined) {
              state.autoRunFeatureEnabled = settings.autoRunFeatureEnabled;
            }
            if (settings.postBreakTransitionPolicy !== undefined) {
              state.postBreakTransitionPolicy =
                settings.postBreakTransitionPolicy;
            }

            if (settings.weeklyAutoRunConfig !== undefined) {
              state.weeklyAutoRunConfig = cloneWeeklyAutoRunConfig(
                settings.weeklyAutoRunConfig,
              );
              writeScheduleValidationResults(state);
            }
          });
        },

        resetToDefaults: (): void => {
          set(createDefaultSettingsState());
        },

        reset: (): void => {
          set(createDefaultSettingsState());
        },
      })),
      {
        name: "pomolice-settings", // localStorage key
      },
    ),
    { name: "SettingsStore" },
  ),
);
