import {
  type SettingsChanges as SettingsWireChanges,
  type PostBreakTransitionPolicy,
  type WeekdayKey,
  type SettingsInput,
  type DaySlotInput,
} from "@/ipc/generated";
import type { WeeklyAutoRunConfig } from "./weeklyAutoRun";

/**
 * User settings as the backend owns them. Durations are in seconds.
 *
 * The backend is the only place settings are stored and validated; the
 * frontend shows the values it receives and, when the user saves, sends
 * only the settings that changed (`SettingsChanges`).
 */
export interface Settings {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
  postBreakTransitionPolicy: PostBreakTransitionPolicy;
  autoRunFeatureEnabled: boolean;
  weeklyAutoRunConfig: WeeklyAutoRunConfig;
  notificationSoundVolumePercent: number;
  notificationSoundMuted: boolean;
}

function mapWeekdays<T>(
  build: (weekday: WeekdayKey) => T,
): Record<WeekdayKey, T> {
  return {
    mon: build("mon"),
    tue: build("tue"),
    wed: build("wed"),
    thu: build("thu"),
    fri: build("fri"),
    sat: build("sat"),
    sun: build("sun"),
  };
}

export function settingsFromPayload(payload: SettingsInput): Settings {
  const weeklyAutoRunConfig = mapWeekdays((weekday) => {
    const slot = payload.weekly_auto_run_config[weekday];
    return {
      enabled: slot.enabled,
      startTime: slot.start_time,
      endTime: slot.end_time,
    };
  });

  return {
    workDurationSecs: payload.work_duration_secs,
    shortBreakDurationSecs: payload.short_break_duration_secs,
    longBreakDurationSecs: payload.long_break_duration_secs,
    longBreakInterval: payload.long_break_interval,
    postBreakTransitionPolicy: payload.post_break_transition_policy,
    autoRunFeatureEnabled: payload.auto_run_feature_enabled,
    weeklyAutoRunConfig,
    notificationSoundVolumePercent: payload.notification_sound_volume_percent,
    notificationSoundMuted: payload.notification_sound_muted,
  };
}

/**
 * The settings a save changes; everything left out keeps the value the
 * backend holds. The weekly schedule is always sent as a whole week.
 */
export type SettingsChanges = Partial<Settings>;

function weeklyAutoRunConfigToPayload(
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
): Record<WeekdayKey, DaySlotInput> {
  return mapWeekdays((weekday) => {
    const slot = weeklyAutoRunConfig[weekday];
    return {
      enabled: slot.enabled,
      start_time: slot.startTime,
      end_time: slot.endTime,
    };
  });
}

/** Wire shape of `SettingsChanges`: only the changed fields are present. */
export function settingsChangesToPayload(
  changes: SettingsChanges,
): SettingsWireChanges {
  const payload: SettingsWireChanges = {};
  if (changes.workDurationSecs !== undefined) {
    payload.work_duration_secs = changes.workDurationSecs;
  }
  if (changes.shortBreakDurationSecs !== undefined) {
    payload.short_break_duration_secs = changes.shortBreakDurationSecs;
  }
  if (changes.longBreakDurationSecs !== undefined) {
    payload.long_break_duration_secs = changes.longBreakDurationSecs;
  }
  if (changes.longBreakInterval !== undefined) {
    payload.long_break_interval = changes.longBreakInterval;
  }
  if (changes.postBreakTransitionPolicy !== undefined) {
    payload.post_break_transition_policy = changes.postBreakTransitionPolicy;
  }
  if (changes.autoRunFeatureEnabled !== undefined) {
    payload.auto_run_feature_enabled = changes.autoRunFeatureEnabled;
  }
  if (changes.weeklyAutoRunConfig !== undefined) {
    payload.weekly_auto_run_config = weeklyAutoRunConfigToPayload(
      changes.weeklyAutoRunConfig,
    );
  }
  if (changes.notificationSoundVolumePercent !== undefined) {
    payload.notification_sound_volume_percent =
      changes.notificationSoundVolumePercent;
  }
  if (changes.notificationSoundMuted !== undefined) {
    payload.notification_sound_muted = changes.notificationSoundMuted;
  }
  return payload;
}
