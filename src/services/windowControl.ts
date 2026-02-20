/**
 * Window Control Service
 *
 * Frontend wrapper for Tauri timer state management commands.
 * All state is managed by the backend - these are convenience wrappers.
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Start a work session with settings
 *
 * @param settings - Timer settings
 */
export async function startWork(settings: {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
}): Promise<void> {
  await invoke("start_work", settings);
}

/**
 * Continue to next work session after break
 */
export async function continueToNextSession(): Promise<void> {
  await invoke("continue_to_next_session");
}

/**
 * End the current cycle
 */
export async function endCycle(): Promise<void> {
  await invoke("end_cycle");
}

/**
 * Emergency stop - reset all state
 */
export async function emergencyStop(): Promise<void> {
  await invoke("emergency_stop");
}

/**
 * Sync settings to backend
 */
export async function syncSettings(settings: {
  workDurationSecs: number;
  shortBreakDurationSecs: number;
  longBreakDurationSecs: number;
  longBreakInterval: number;
}): Promise<void> {
  await invoke("sync_settings", settings);
}

/**
 * Get current timer status
 *
 * @returns "idle" | "working" | "on_break"
 */
export async function getTimerStatus(): Promise<string> {
  return await invoke("get_timer_status");
}

/**
 * Record a skip attempt
 *
 * @returns Number of skip attempts in current session
 */
export async function recordSkipAttempt(): Promise<number> {
  return await invoke("record_skip_attempt");
}

