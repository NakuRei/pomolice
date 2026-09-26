import { describe, expect, it } from "vitest";
import {
  createSettingsForTest,
  createWeeklyAutoRunConfigForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";
import { settingsChangesToPayload, settingsFromPayload } from "./settings";

describe("settings wire mapping", () => {
  const settings = createSettingsForTest({
    workDurationSecs: 1800,
    shortBreakDurationSecs: 240,
    longBreakDurationSecs: 1200,
    longBreakInterval: 3,
    postBreakTransitionPolicy: "start_next_work_automatically",
    autoRunFeatureEnabled: true,
    weeklyAutoRunConfig: createWeeklyAutoRunConfigForTest({
      fri: { enabled: true, startTime: "22:00", endTime: "02:00" },
    }),
    notificationSoundVolumePercent: 70,
    notificationSoundMuted: true,
  });

  it("sends only the changed settings, under the backend's field names", () => {
    expect(
      settingsChangesToPayload({
        notificationSoundMuted: true,
        longBreakInterval: 3,
      }),
    ).toEqual({
      notification_sound_muted: true,
      long_break_interval: 3,
    });
  });

  it("sends a changed weekly schedule as the whole week", () => {
    const payload = settingsChangesToPayload({
      weeklyAutoRunConfig: settings.weeklyAutoRunConfig,
    });

    expect(Object.keys(payload)).toEqual(["weekly_auto_run_config"]);
    expect(payload.weekly_auto_run_config?.fri).toEqual({
      enabled: true,
      start_time: "22:00",
      end_time: "02:00",
    });
    expect(payload.weekly_auto_run_config?.mon).toEqual({
      enabled: false,
      start_time: "09:00",
      end_time: "18:00",
    });
  });

  it("reads the backend's settings back field by field", () => {
    expect(settingsFromPayload(settingsPayloadForTest(settings))).toEqual(
      settings,
    );
  });
});
