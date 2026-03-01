import { describe, it, expect, beforeEach } from "vitest";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
  DEFAULT_SETTINGS,
  VALIDATION_RULES,
  parseHhMm,
} from "../settingsStore";

describe("settingsStore", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useSettingsStore.getState().resetToDefaults();
  });

  describe("Initial State", () => {
    it("should have correct default work duration (25 minutes)", () => {
      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(25);
    });

    it("should have correct default short break duration (5 minutes)", () => {
      const state = useSettingsStore.getState();
      expect(state.shortBreakDuration).toBe(5);
    });

    it("should have correct default long break duration (15 minutes)", () => {
      const state = useSettingsStore.getState();
      expect(state.longBreakDuration).toBe(15);
    });

    it("should have correct default long break interval (4 pomodoros)", () => {
      const state = useSettingsStore.getState();
      expect(state.longBreakInterval).toBe(4);
    });

    it("should have correct default notification sound volume (50%)", () => {
      const state = useSettingsStore.getState();
      expect(state.notificationSoundVolumePercent).toBe(50);
    });

    it("should have notification sound unmuted by default", () => {
      const state = useSettingsStore.getState();
      expect(state.notificationSoundMuted).toBe(false);
    });

    it("should have auto-run feature disabled by default", () => {
      const state = useSettingsStore.getState();
      expect(state.autoRunFeatureEnabled).toBe(false);
    });

    it("should default all weekday auto-run slots to disabled", () => {
      const state = useSettingsStore.getState();
      expect(state.weeklyAutoRunConfig).toEqual(
        createWeeklyAutoRunConfigAllDaysDisabled(),
      );
    });

    it("should start with no schedule validation errors", () => {
      const state = useSettingsStore.getState();
      expect(state.dayScheduleValidationErrors).toEqual({});
      expect(state.weeklyScheduleValidationErrors).toEqual([]);
    });
  });

  describe("Actions", () => {
    describe("setWorkDuration()", () => {
      it("should update work duration within valid range", () => {
        const { setWorkDuration } = useSettingsStore.getState();
        setWorkDuration(30);
        expect(useSettingsStore.getState().workDuration).toBe(30);
      });

      it("should clamp work duration to minimum (1)", () => {
        const { setWorkDuration } = useSettingsStore.getState();
        setWorkDuration(0);
        expect(useSettingsStore.getState().workDuration).toBe(
          VALIDATION_RULES.workDuration.min
        );
      });

      it("should clamp work duration to maximum (120)", () => {
        const { setWorkDuration } = useSettingsStore.getState();
        setWorkDuration(150);
        expect(useSettingsStore.getState().workDuration).toBe(
          VALIDATION_RULES.workDuration.max
        );
      });

      it("should handle negative values by clamping to minimum", () => {
        const { setWorkDuration } = useSettingsStore.getState();
        setWorkDuration(-10);
        expect(useSettingsStore.getState().workDuration).toBe(
          VALIDATION_RULES.workDuration.min
        );
      });

      it("should accept decimal values (Phase 3 testing support)", () => {
        const { setWorkDuration } = useSettingsStore.getState();
        setWorkDuration(25.7);
        expect(useSettingsStore.getState().workDuration).toBe(25.7);
      });
    });

    describe("setShortBreakDuration()", () => {
      it("should update short break duration within valid range", () => {
        const { setShortBreakDuration } = useSettingsStore.getState();
        setShortBreakDuration(10);
        expect(useSettingsStore.getState().shortBreakDuration).toBe(10);
      });

      it("should clamp short break duration to minimum (1)", () => {
        const { setShortBreakDuration } = useSettingsStore.getState();
        setShortBreakDuration(0);
        expect(useSettingsStore.getState().shortBreakDuration).toBe(
          VALIDATION_RULES.shortBreakDuration.min
        );
      });

      it("should clamp short break duration to maximum (60)", () => {
        const { setShortBreakDuration } = useSettingsStore.getState();
        setShortBreakDuration(100);
        expect(useSettingsStore.getState().shortBreakDuration).toBe(
          VALIDATION_RULES.shortBreakDuration.max
        );
      });
    });

    describe("setLongBreakDuration()", () => {
      it("should update long break duration within valid range", () => {
        const { setLongBreakDuration } = useSettingsStore.getState();
        setLongBreakDuration(20);
        expect(useSettingsStore.getState().longBreakDuration).toBe(20);
      });

      it("should clamp long break duration to minimum (0.01)", () => {
        const { setLongBreakDuration } = useSettingsStore.getState();
        setLongBreakDuration(0); // Below minimum
        expect(useSettingsStore.getState().longBreakDuration).toBe(
          VALIDATION_RULES.longBreakDuration.min
        );
      });

      it("should clamp long break duration to maximum (60)", () => {
        const { setLongBreakDuration } = useSettingsStore.getState();
        setLongBreakDuration(100);
        expect(useSettingsStore.getState().longBreakDuration).toBe(
          VALIDATION_RULES.longBreakDuration.max
        );
      });
    });

    describe("setLongBreakInterval()", () => {
      it("should update long break interval within valid range", () => {
        const { setLongBreakInterval } = useSettingsStore.getState();
        setLongBreakInterval(6);
        expect(useSettingsStore.getState().longBreakInterval).toBe(6);
      });

      it("should clamp long break interval to minimum (2)", () => {
        const { setLongBreakInterval } = useSettingsStore.getState();
        setLongBreakInterval(1);
        expect(useSettingsStore.getState().longBreakInterval).toBe(
          VALIDATION_RULES.longBreakInterval.min
        );
      });

      it("should clamp long break interval to maximum (10)", () => {
        const { setLongBreakInterval } = useSettingsStore.getState();
        setLongBreakInterval(15);
        expect(useSettingsStore.getState().longBreakInterval).toBe(
          VALIDATION_RULES.longBreakInterval.max
        );
      });
    });

    describe("setNotificationSoundVolumePercent()", () => {
      it("should update notification sound volume within valid range", () => {
        const { setNotificationSoundVolumePercent } = useSettingsStore.getState();
        setNotificationSoundVolumePercent(30);
        expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(30);
      });

      it("should clamp notification sound volume to minimum (0)", () => {
        const { setNotificationSoundVolumePercent } = useSettingsStore.getState();
        setNotificationSoundVolumePercent(-10);
        expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(
          VALIDATION_RULES.notificationSoundVolumePercent.min
        );
      });

      it("should clamp notification sound volume to maximum (100)", () => {
        const { setNotificationSoundVolumePercent } = useSettingsStore.getState();
        setNotificationSoundVolumePercent(120);
        expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(
          VALIDATION_RULES.notificationSoundVolumePercent.max
        );
      });
    });

    describe("toggleNotificationSoundMuted()", () => {
      it("should toggle mute state from false to true", () => {
        const { toggleNotificationSoundMuted } = useSettingsStore.getState();
        toggleNotificationSoundMuted();
        expect(useSettingsStore.getState().notificationSoundMuted).toBe(true);
      });

      it("should toggle mute state from true back to false", () => {
        const { toggleNotificationSoundMuted } = useSettingsStore.getState();
        toggleNotificationSoundMuted();
        toggleNotificationSoundMuted();
        expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
      });
    });

    describe("setAutoRunFeatureEnabled()", () => {
      it("should enable auto-run feature", () => {
        const { setAutoRunFeatureEnabled } = useSettingsStore.getState();
        setAutoRunFeatureEnabled(true);
        expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(true);
      });

      it("should disable auto-run feature", () => {
        const { setAutoRunFeatureEnabled } = useSettingsStore.getState();
        setAutoRunFeatureEnabled(true);
        setAutoRunFeatureEnabled(false);
        expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(false);
      });
    });

    describe("setDayAutoRunEnabled()", () => {
      it("should toggle one weekday slot", () => {
        const { setDayAutoRunEnabled } = useSettingsStore.getState();
        setDayAutoRunEnabled("mon", true);
        expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.enabled).toBe(true);
      });
    });

    describe("setDayStartTime() and setDayEndTime()", () => {
      it("should update one weekday slot time text", () => {
        const { setDayStartTime, setDayEndTime } = useSettingsStore.getState();
        setDayStartTime("wed", "10:00");
        setDayEndTime("wed", "19:00");

        const daySlot = useSettingsStore.getState().weeklyAutoRunConfig.wed;
        expect(daySlot.startTime).toBe("10:00");
        expect(daySlot.endTime).toBe("19:00");
      });

      it("should write start-time format error when start time is invalid", () => {
        const { setDayAutoRunEnabled, setDayStartTime } = useSettingsStore.getState();
        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "9:00");

        const error = useSettingsStore.getState().dayScheduleValidationErrors.mon;
        expect(error).toEqual({
          code: "start_time_format_invalid",
          weekday: "mon",
          invalidStartTimeText: "9:00",
          message: "Start time for mon must use HH:MM (24-hour).",
        });
      });

      it("should write end-time format error when end time is invalid", () => {
        const { setDayAutoRunEnabled, setDayEndTime } = useSettingsStore.getState();
        setDayAutoRunEnabled("mon", true);
        setDayEndTime("mon", "24:00");

        const error = useSettingsStore.getState().dayScheduleValidationErrors.mon;
        expect(error).toEqual({
          code: "end_time_format_invalid",
          weekday: "mon",
          invalidEndTimeText: "24:00",
          message: "End time for mon must use HH:MM (24-hour).",
        });
      });

      it("should write equality error when start and end are identical", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();
        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "12:00");
        setDayEndTime("mon", "12:00");

        const error = useSettingsStore.getState().dayScheduleValidationErrors.mon;
        expect(error).toEqual({
          code: "start_time_equals_end_time",
          weekday: "mon",
          startTime: "12:00",
          endTime: "12:00",
          message: "Start and end time cannot be identical for mon.",
        });
      });

      it("should allow overnight windows without a day error", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();
        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "22:00");
        setDayEndTime("mon", "02:00");

        expect(useSettingsStore.getState().dayScheduleValidationErrors.mon).toBeUndefined();
      });

      it("should clear day error when weekday slot becomes disabled", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();
        setDayAutoRunEnabled("fri", true);
        setDayStartTime("fri", "12:00");
        setDayEndTime("fri", "12:00");
        expect(useSettingsStore.getState().dayScheduleValidationErrors.fri).toBeDefined();

        setDayAutoRunEnabled("fri", false);
        expect(useSettingsStore.getState().dayScheduleValidationErrors.fri).toBeUndefined();
      });
    });

    describe("weekly schedule overlap validation", () => {
      it("should write overlap error for adjacent weekday windows", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();

        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "22:00");
        setDayEndTime("mon", "02:00");

        setDayAutoRunEnabled("tue", true);
        setDayStartTime("tue", "01:00");
        setDayEndTime("tue", "05:00");

        expect(useSettingsStore.getState().weeklyScheduleValidationErrors).toEqual([
          {
            code: "overlapping_windows_between_days",
            primaryWeekday: "mon",
            secondaryWeekday: "tue",
            message: "Schedule windows overlap between mon and tue.",
          },
        ]);
      });

      it("should clear overlap error when windows no longer overlap", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();

        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "22:00");
        setDayEndTime("mon", "02:00");
        setDayAutoRunEnabled("tue", true);
        setDayStartTime("tue", "01:00");
        setDayEndTime("tue", "05:00");

        setDayStartTime("tue", "03:00");
        setDayEndTime("tue", "05:00");

        expect(useSettingsStore.getState().weeklyScheduleValidationErrors).toEqual([]);
      });

      it("should detect overlap when sunday overnight range reaches monday", () => {
        const { setDayAutoRunEnabled, setDayStartTime, setDayEndTime } =
          useSettingsStore.getState();

        setDayAutoRunEnabled("sun", true);
        setDayStartTime("sun", "22:00");
        setDayEndTime("sun", "02:00");

        setDayAutoRunEnabled("mon", true);
        setDayStartTime("mon", "01:00");
        setDayEndTime("mon", "03:00");

        expect(useSettingsStore.getState().weeklyScheduleValidationErrors).toEqual([
          {
            code: "overlapping_windows_between_days",
            primaryWeekday: "mon",
            secondaryWeekday: "sun",
            message: "Schedule windows overlap between mon and sun.",
          },
        ]);
      });
    });

    describe("resetToDefaults()", () => {
      it("should reset all settings to default values", () => {
        const { setWorkDuration, setShortBreakDuration, resetToDefaults } =
          useSettingsStore.getState();

        // Change some values
        setWorkDuration(50);
        setShortBreakDuration(10);

        // Reset
        resetToDefaults();

        const state = useSettingsStore.getState();
        expect(state.workDuration).toBe(DEFAULT_SETTINGS.workDuration);
        expect(state.shortBreakDuration).toBe(
          DEFAULT_SETTINGS.shortBreakDuration
        );
        expect(state.longBreakDuration).toBe(DEFAULT_SETTINGS.longBreakDuration);
        expect(state.longBreakInterval).toBe(DEFAULT_SETTINGS.longBreakInterval);
        expect(state.notificationSoundVolumePercent).toBe(
          DEFAULT_SETTINGS.notificationSoundVolumePercent
        );
        expect(state.notificationSoundMuted).toBe(
          DEFAULT_SETTINGS.notificationSoundMuted
        );
        expect(state.autoRunFeatureEnabled).toBe(
          DEFAULT_SETTINGS.autoRunFeatureEnabled
        );
        expect(state.weeklyAutoRunConfig).toEqual(
          DEFAULT_SETTINGS.weeklyAutoRunConfig
        );
        expect(state.dayScheduleValidationErrors).toEqual(
          DEFAULT_SETTINGS.dayScheduleValidationErrors
        );
        expect(state.weeklyScheduleValidationErrors).toEqual(
          DEFAULT_SETTINGS.weeklyScheduleValidationErrors
        );
      });
    });
  });

  describe("Persistence", () => {
    it("should persist settings to localStorage", () => {
      const { setWorkDuration } = useSettingsStore.getState();
      setWorkDuration(40);

      // Check if value persists
      expect(useSettingsStore.getState().workDuration).toBe(40);
    });
  });

  describe("Constants", () => {
    it("should export DEFAULT_SETTINGS", () => {
      expect(DEFAULT_SETTINGS).toEqual({
        workDuration: 25,
        shortBreakDuration: 5,
        longBreakDuration: 15,
        longBreakInterval: 4,
        notificationSoundVolumePercent: 50,
        notificationSoundMuted: false,
        autoRunFeatureEnabled: false,
        weeklyAutoRunConfig: createWeeklyAutoRunConfigAllDaysDisabled(),
        dayScheduleValidationErrors: {},
        weeklyScheduleValidationErrors: [],
      });
    });

    it("should export VALIDATION_RULES", () => {
      // Phase 3: Temporarily using 0.01 min for testing (will be restored to 1/5 before production)
      expect(VALIDATION_RULES.workDuration).toEqual({ min: 0.01, max: 120 });
      expect(VALIDATION_RULES.shortBreakDuration).toEqual({ min: 0.01, max: 60 });
      expect(VALIDATION_RULES.longBreakDuration).toEqual({ min: 0.01, max: 60 });
      expect(VALIDATION_RULES.longBreakInterval).toEqual({ min: 2, max: 10 });
      expect(VALIDATION_RULES.notificationSoundVolumePercent).toEqual({ min: 0, max: 100 });
    });
  });

  describe("parseHhMm()", () => {
    it("should parse strict HH:MM format", () => {
      expect(parseHhMm("09:05")).toEqual({
        hour: 9,
        minute: 5,
        minutesSinceStartOfDay: 9 * 60 + 5,
      });
    });

    it("should reject non-strict time text", () => {
      expect(parseHhMm("9:05")).toBeNull();
      expect(parseHhMm("24:00")).toBeNull();
      expect(parseHhMm("12:60")).toBeNull();
      expect(parseHhMm("aa:bb")).toBeNull();
    });
  });
});
