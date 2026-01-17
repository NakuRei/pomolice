import { create } from "zustand";
import { devtools, persist } from "zustand/middleware";
import { immer } from "zustand/middleware/immer";

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
}

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
  /** Update multiple settings at once */
  updateSettings: (settings: Partial<SettingsState>) => void;
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
export const DEFAULT_SETTINGS: SettingsState = {
  workDuration: 25,
  shortBreakDuration: 5,
  longBreakDuration: 15,
  longBreakInterval: 4,
};

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
  workDuration: { min: 0.01, max: 120 },        // TEMP: 0.01 min (0.6 sec) for testing
  shortBreakDuration: { min: 0.01, max: 60 },   // TEMP: 0.01 min (0.6 sec) for testing
  longBreakDuration: { min: 0.01, max: 60 },    // TEMP: 0.01 min (0.6 sec) for testing
  longBreakInterval: { min: 2, max: 10 },
} as const;

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
const conditionalDevtools =
  process.env.NODE_ENV === 'development'
    ? devtools
    : (<T,>(config: T): T => config) as typeof devtools;

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
        ...DEFAULT_SETTINGS,

        setWorkDuration: (minutes: number) =>
          set((state) => {
            state.workDuration = validateRange(
              minutes,
              VALIDATION_RULES.workDuration.min,
              VALIDATION_RULES.workDuration.max
            );
          }),

        setShortBreakDuration: (minutes: number) =>
          set((state) => {
            state.shortBreakDuration = validateRange(
              minutes,
              VALIDATION_RULES.shortBreakDuration.min,
              VALIDATION_RULES.shortBreakDuration.max
            );
          }),

        setLongBreakDuration: (minutes: number) =>
          set((state) => {
            state.longBreakDuration = validateRange(
              minutes,
              VALIDATION_RULES.longBreakDuration.min,
              VALIDATION_RULES.longBreakDuration.max
            );
          }),

        setLongBreakInterval: (count: number) =>
          set((state) => {
            state.longBreakInterval = validateRange(
              count,
              VALIDATION_RULES.longBreakInterval.min,
              VALIDATION_RULES.longBreakInterval.max
            );
          }),

        updateSettings: (settings: Partial<SettingsState>) =>
          set((state) => {
            if (settings.workDuration !== undefined) {
              state.workDuration = validateRange(
                settings.workDuration,
                VALIDATION_RULES.workDuration.min,
                VALIDATION_RULES.workDuration.max
              );
            }
            if (settings.shortBreakDuration !== undefined) {
              state.shortBreakDuration = validateRange(
                settings.shortBreakDuration,
                VALIDATION_RULES.shortBreakDuration.min,
                VALIDATION_RULES.shortBreakDuration.max
              );
            }
            if (settings.longBreakDuration !== undefined) {
              state.longBreakDuration = validateRange(
                settings.longBreakDuration,
                VALIDATION_RULES.longBreakDuration.min,
                VALIDATION_RULES.longBreakDuration.max
              );
            }
            if (settings.longBreakInterval !== undefined) {
              state.longBreakInterval = validateRange(
                settings.longBreakInterval,
                VALIDATION_RULES.longBreakInterval.min,
                VALIDATION_RULES.longBreakInterval.max
              );
            }
          }),

        resetToDefaults: () => set(DEFAULT_SETTINGS),

        reset: () => set(DEFAULT_SETTINGS),
      })),
      {
        name: "pomolice-settings", // localStorage key
      }
    ),
    { name: "SettingsStore" }
  )
);
