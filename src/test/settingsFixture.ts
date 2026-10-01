import type { SettingsInput } from "@/ipc/generated";
import {
  settingsChangesToPayload,
  type Settings,
} from "@/features/settings/settings";
import type {
  DayAutoRunSlot,
  WeeklyAutoRunConfig,
} from "@/features/settings/weeklyAutoRun";

const DISABLED_DAY: DayAutoRunSlot = {
  enabled: false,
  startTime: "09:00",
  endTime: "18:00",
};

export function createWeeklyAutoRunConfigForTest(
  overrides: Partial<WeeklyAutoRunConfig> = {},
): WeeklyAutoRunConfig {
  return {
    mon: { ...DISABLED_DAY },
    tue: { ...DISABLED_DAY },
    wed: { ...DISABLED_DAY },
    thu: { ...DISABLED_DAY },
    fri: { ...DISABLED_DAY },
    sat: { ...DISABLED_DAY },
    sun: { ...DISABLED_DAY },
    ...overrides,
  };
}

/** Settings as the backend would report them, for tests. */
export function createSettingsForTest(
  overrides: Partial<Settings> = {},
): Settings {
  return {
    workDurationSecs: 25 * 60,
    shortBreakDurationSecs: 5 * 60,
    longBreakDurationSecs: 15 * 60,
    longBreakInterval: 4,
    postBreakTransitionPolicy: "require_manual_continue",
    autoRunFeatureEnabled: false,
    weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest(),
    notificationSoundVolumePercent: 50,
    notificationSoundMuted: false,
    ...overrides,
  };
}

/** The full settings as the backend sends them in a full state. */
export function settingsPayloadForTest(settings: Settings): SettingsInput {
  // Every field of a complete `Settings` is present in its changes payload.
  return settingsChangesToPayload(settings) as SettingsInput;
}

/**
 * The settings the backend holds after a save of `changes` over `current`,
 * for tests that stand in for the backend.
 */
export function settingsPayloadAfterChangesForTest(
  current: Settings,
  changes: unknown,
): SettingsInput {
  return {
    ...settingsPayloadForTest(current),
    ...(changes as Partial<SettingsInput>),
  };
}
