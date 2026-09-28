import type { SettingsFileStatus, SettingsPayload } from "./settings";
import type { WeekdayKey } from "./weeklyAutoRun";
import type { SessionTransitionOutcome } from "./sessionTransitionPolicy";

/**
 * Timer status constants matching backend TimerStatus enum
 */
export const TimerStatus = {
  IDLE: "idle",
  WORKING: "working",
  ON_BREAK: "on_break",
} as const;

export type TimerStatus = (typeof TimerStatus)[keyof typeof TimerStatus];

/**
 * Session type constants matching backend SessionType enum
 */
export const SessionType = {
  WORK: "work",
  SHORT_BREAK: "short_break",
  LONG_BREAK: "long_break",
} as const;

export type SessionTypeValue = (typeof SessionType)[keyof typeof SessionType];

/**
 * Payload received from backend timer_tick events
 */
export interface TimerTickPayload {
  status: TimerStatus;
  remaining_seconds: number;
  /** Total length of the current session; while idle, of the next work session. */
  session_duration_secs: number;
  session_type: SessionTypeValue;
  current_cycle: number;
  completed_pomodoros: number;
  break_interaction_count: number;
  away_during_break: boolean;
}

/**
 * Payload received from backend session_completed events
 */
export type SessionCompletedPayload = SessionTransitionOutcome;

/**
 * Full state payload from backend get_full_state command
 */
export interface FullStatePayload extends TimerTickPayload {
  post_break_decision_pending: boolean;
  settings: SettingsPayload;
  settings_file_status: SettingsFileStatus;
  boot_auto_start_offer: BootAutoStartOfferPayload | null;
  active_window_summary: ActiveWindowSummaryPayload | null;
}

export interface ActiveWindowSummaryPayload {
  weekday_key: WeekdayKey;
  start_time: string;
  end_time: string;
  crosses_midnight: boolean;
}

export interface BootAutoStartOfferPayload {
  anchor_local_date: string;
  window: ActiveWindowSummaryPayload;
}

/**
 * Payload received from backend active_window_changed events.
 * Null while auto-run is disabled or the time is outside every enabled slot.
 */
export interface ActiveWindowChangedPayload {
  active_window_summary: ActiveWindowSummaryPayload | null;
}

/**
 * Payload received from backend settings_changed events, sent whenever the
 * settings in effect change.
 */
export interface SettingsChangedPayload {
  settings: SettingsPayload;
  settings_file_status: SettingsFileStatus;
}

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
