/**
 * Window Control Service
 *
 * Frontend wrapper for Tauri window control commands.
 * Provides functions to manage fullscreen mode and window state.
 *
 * Phase 3: Basic fullscreen enforcement
 */

import { invoke } from "@tauri-apps/api/core";

/**
 * Enter fullscreen mode with always-on-top
 *
 * Makes the window fullscreen and prevents users from easily switching away.
 * Used when break time starts.
 */
export async function enterFullscreen(): Promise<void> {
  try {
    await invoke("enter_fullscreen");
  } catch (error) {
    console.error("Failed to enter fullscreen:", error);
    throw error;
  }
}

/**
 * Exit fullscreen mode and restore normal window state
 *
 * Restores the window to normal mode after break completion.
 */
export async function exitFullscreen(): Promise<void> {
  try {
    await invoke("exit_fullscreen");
  } catch (error) {
    console.error("Failed to exit fullscreen:", error);
    throw error;
  }
}

/**
 * Set always-on-top behavior
 *
 * @param alwaysOnTop - Whether the window should stay on top
 */
export async function setAlwaysOnTop(alwaysOnTop: boolean): Promise<void> {
  try {
    await invoke("set_always_on_top", { alwaysOnTop });
  } catch (error) {
    console.error("Failed to set always on top:", error);
    throw error;
  }
}

/**
 * Restore window to normal state
 *
 * Ensures the window is visible, focused, and not minimized.
 */
export async function restoreWindow(): Promise<void> {
  try {
    await invoke("restore_window");
  } catch (error) {
    console.error("Failed to restore window:", error);
    throw error;
  }
}
