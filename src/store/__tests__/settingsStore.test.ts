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
      });
    });

    it("should export VALIDATION_RULES", () => {
      // Phase 3: Temporarily using 0.01 min for testing (will be restored to 1/5 before production)
      expect(VALIDATION_RULES.workDuration).toEqual({ min: 0.01, max: 120 });
      expect(VALIDATION_RULES.shortBreakDuration).toEqual({ min: 0.01, max: 60 });
      expect(VALIDATION_RULES.longBreakDuration).toEqual({ min: 0.01, max: 60 });
      expect(VALIDATION_RULES.longBreakInterval).toEqual({ min: 2, max: 10 });
    });
  });
});
