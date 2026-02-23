import { describe, it, expect, beforeEach } from "vitest";
import {
  useSettingsStore,
  DEFAULT_SETTINGS,
  VALIDATION_RULES,
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
});
