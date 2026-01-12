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
  /** Reset all settings to default values */
  resetToDefaults: () => void;
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
 */
export const VALIDATION_RULES = {
  workDuration: { min: 1, max: 120 },
  shortBreakDuration: { min: 1, max: 60 },
  longBreakDuration: { min: 5, max: 60 },
  longBreakInterval: { min: 2, max: 10 },
} as const;

/**
 * Validate and clamp a value within min/max range
 * @param value - Value to validate
 * @param min - Minimum allowed value
 * @param max - Maximum allowed value
 * @returns Clamped value within valid range
 */
function validateRange(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, Math.floor(value)));
}

/**
 * Zustand store for settings management
 * Uses immer middleware for immutable state updates
 * Uses devtools middleware for Redux DevTools integration
 * Uses persist middleware for localStorage persistence
 */
export const useSettingsStore = create<SettingsStore>()(
  devtools(
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

        resetToDefaults: () => set(DEFAULT_SETTINGS),
      })),
      {
        name: "pomolice-settings", // localStorage key
      }
    ),
    { name: "SettingsStore" }
  )
);
