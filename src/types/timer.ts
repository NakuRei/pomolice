/**
 * Timer-related type definitions
 * Phase 0: Basic type definitions for timer functionality
 */

/**
 * Timer configuration interface
 * Used for storing and managing timer duration settings
 */
export interface TimerConfig {
  /** Work session duration in minutes */
  workDuration: number;
  /** Short break duration in minutes */
  shortBreakDuration: number;
  /** Long break duration in minutes */
  longBreakDuration: number;
  /** Number of pomodoros before a long break */
  longBreakInterval: number;
}

/**
 * Default timer configuration
 * Following the standard Pomodoro Technique defaults
 */
export const DEFAULT_TIMER_CONFIG: TimerConfig = {
  workDuration: 25,
  shortBreakDuration: 5,
  longBreakDuration: 15,
  longBreakInterval: 4,
};
