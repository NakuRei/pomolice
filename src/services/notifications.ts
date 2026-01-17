/**
 * Notifications Service
 * Wrapper for Tauri notification commands
 */

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';

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
 * Ensure notification permission is granted
 * @returns true if permission is granted, false otherwise
 */
async function ensureNotificationPermission(): Promise<boolean> {
  if (!isTauriEnvironment()) {
    return false;
  }

  try {
    let permissionGranted = await isPermissionGranted();

    if (!permissionGranted) {
      const permission = await requestPermission();
      permissionGranted = permission === 'granted';
    }

    return permissionGranted;
  } catch (error) {
    console.error('Failed to request notification permission:', error);
    return false;
  }
}

/**
 * Show notification when work session is complete
 */
export async function notifyWorkComplete(): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  try {
    sendNotification({
      title: '休憩時間です！',
      body: 'お疲れ様でした。休憩を取りましょう。',
    });
  } catch (error) {
    console.error('Failed to show work complete notification:', error);
  }
}

/**
 * Show notification when break session is complete
 */
export async function notifyBreakComplete(): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  try {
    sendNotification({
      title: '作業に戻りましょう！',
      body: '休憩が終わりました。次のポモドーロを始めましょう。',
    });
  } catch (error) {
    console.error('Failed to show break complete notification:', error);
  }
}

/**
 * Show notification when long break session is complete
 */
export async function notifyLongBreakComplete(): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  try {
    sendNotification({
      title: '長い休憩完了！',
      body: 'サイクルが完了しました。次のサイクルを始めますか？',
    });
  } catch (error) {
    console.error('Failed to show long break complete notification:', error);
  }
}
