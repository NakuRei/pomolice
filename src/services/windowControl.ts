/**
 * Window Control Service
 *
 * Frontend wrapper for Tauri timer state management commands.
 * Security: Uses high-level commands that validate state in Rust backend.
 *
 * Phase 3: Secure fullscreen enforcement through timer state management
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Start a break session
 *
 * Requests the backend to start a break session.
 * The backend will validate the state and automatically enter fullscreen on all monitors.
 *
 * @param duration - Break duration in seconds
 * @throws Error if not in valid state or window control fails
 */
export async function startBreak(duration: number): Promise<void> {
  try {
    await invoke("start_break", { duration });
  } catch (error) {
    console.error("Failed to start break:", error);
    throw error;
  }
}

/**
 * Complete a break session
 *
 * Requests the backend to complete the current break session.
 * The backend will validate the state and automatically exit fullscreen on all monitors.
 *
 * @throws Error if not on break or window control fails
 */
export async function completeBreak(): Promise<void> {
  try {
    await invoke("complete_break");
  } catch (error) {
    console.error("Failed to complete break:", error);
    throw error;
  }
}

/**
 * Start a work session
 *
 * @param duration - Work duration in seconds
 */
export async function startWork(duration: number): Promise<void> {
  try {
    await invoke("start_work", { duration });
  } catch (error) {
    console.error("Failed to start work:", error);
    throw error;
  }
}

/**
 * Complete a work session
 */
export async function completeWork(): Promise<void> {
  try {
    await invoke("complete_work");
  } catch (error) {
    console.error("Failed to complete work:", error);
    throw error;
  }
}

/**
 * Get current timer status
 *
 * @returns "idle" | "working" | "on_break"
 */
export async function getTimerStatus(): Promise<string> {
  try {
    return await invoke("get_timer_status");
  } catch (error) {
    console.error("Failed to get timer status:", error);
    throw error;
  }
}

/**
 * Record a skip attempt
 *
 * Notifies the backend that the user attempted to skip.
 * Used for analytics and penalty calculation.
 *
 * @returns Number of skip attempts in current session
 */
export async function recordSkipAttempt(): Promise<number> {
  try {
    return await invoke("record_skip_attempt");
  } catch (error) {
    console.error("Failed to record skip attempt:", error);
    throw error;
  }
}

// ============================================================================
// DEPRECATED FUNCTIONS - DO NOT USE
// These low-level functions are no longer available from the backend
// ============================================================================

/**
 * @deprecated Use startBreak() instead - direct window control is disabled for security
 */
export async function enterFullscreen(): Promise<void> {
  throw new Error("enterFullscreen is deprecated. Use startBreak() instead.");
}

/**
 * @deprecated Use completeBreak() instead - direct window control is disabled for security
 */
export async function exitFullscreen(): Promise<void> {
  throw new Error("exitFullscreen is deprecated. Use completeBreak() instead.");
}

/**
 * @deprecated Direct window control is disabled for security
 */
export async function setAlwaysOnTop(_alwaysOnTop: boolean): Promise<void> {
  throw new Error("setAlwaysOnTop is deprecated and disabled for security.");
}

/**
 * @deprecated Direct window control is disabled for security
 */
export async function restoreWindow(): Promise<void> {
  throw new Error("restoreWindow is deprecated and disabled for security.");
}

/**
 * @deprecated Use startBreak() instead - direct window control is disabled for security
 */
export async function enterFullscreenAllMonitors(): Promise<void> {
  throw new Error("enterFullscreenAllMonitors is deprecated. Use startBreak() instead.");
}

/**
 * @deprecated Use completeBreak() instead - direct window control is disabled for security
 */
export async function exitFullscreenAllMonitors(): Promise<void> {
  throw new Error("exitFullscreenAllMonitors is deprecated. Use completeBreak() instead.");
}
