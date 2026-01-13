/**
 * Notifications Service
 * Wrapper for Tauri notification commands
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Check if we're running in Tauri environment
 */
function isTauriEnvironment(): boolean {
  // Skip notifications in vitest environment
  if (typeof import.meta !== 'undefined' && import.meta.env?.VITEST) {
    return false;
  }

  // In Tauri v2, check if we're in a Tauri context by checking if window.__TAURI_INTERNALS__ exists
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Show notification when work session is complete
 */
export async function notifyWorkComplete(): Promise<void> {
  if (!isTauriEnvironment()) {
    console.log('Skipping notification (not in Tauri environment)');
    return;
  }

  try {
    console.log('Calling notify_work_complete...');
    await invoke('notify_work_complete');
    console.log('Notification sent successfully');
  } catch (error) {
    console.error('Failed to show work complete notification:', error);
  }
}

/**
 * Show notification when break session is complete
 */
export async function notifyBreakComplete(): Promise<void> {
  if (!isTauriEnvironment()) {
    console.log('Skipping notification (not in Tauri environment)');
    return;
  }

  try {
    console.log('Calling notify_break_complete...');
    await invoke('notify_break_complete');
    console.log('Notification sent successfully');
  } catch (error) {
    console.error('Failed to show break complete notification:', error);
  }
}

/**
 * Show notification when long break session is complete
 */
export async function notifyLongBreakComplete(): Promise<void> {
  if (!isTauriEnvironment()) {
    console.log('Skipping notification (not in Tauri environment)');
    return;
  }

  try {
    console.log('Calling notify_long_break_complete...');
    await invoke('notify_long_break_complete');
    console.log('Notification sent successfully');
  } catch (error) {
    console.error('Failed to show long break complete notification:', error);
  }
}
