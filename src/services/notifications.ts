/**
 * Notifications Service
 * Wrapper for Tauri notification commands
 */

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { convertFileSrc } from '@tauri-apps/api/core';
import { resolveResource } from '@tauri-apps/api/path';
import { useSettingsStore } from '../store/settingsStore';
import { SessionTypeValue } from '../store/timerStore';

const sessionSoundResourceBySessionType = {
  work: 'resources/sounds/work.wav',
  short_break: 'resources/sounds/short_break.wav',
  long_break: 'resources/sounds/long_break.wav',
} as const;

type SessionSoundType = keyof typeof sessionSoundResourceBySessionType;

const resolvedSessionSoundPathBySessionType: Partial<
  Record<SessionSoundType, string>
> = {};

const windowsSoundEventBySessionType: Record<SessionSoundType, string> = {
  work: 'ms-winsoundevent:Notification.IM',
  short_break: 'ms-winsoundevent:Notification.Mail',
  long_break: 'ms-winsoundevent:Notification.Reminder',
};

function isWindowsPlatform(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    navigator.userAgent.toLowerCase().includes('windows')
  );
}

async function resolveSessionSoundPath(
  sessionSoundType: SessionSoundType
): Promise<string | undefined> {
  const cachedPath = resolvedSessionSoundPathBySessionType[sessionSoundType];
  if (cachedPath) {
    return cachedPath;
  }

  try {
    const resourcePath = sessionSoundResourceBySessionType[sessionSoundType];
    const resolvedPath = await resolveResource(resourcePath);
    resolvedSessionSoundPathBySessionType[sessionSoundType] = resolvedPath;
    return resolvedPath;
  } catch (error) {
    console.warn('Failed to resolve notification sound path:', {
      sessionSoundType,
      error,
    });
    return undefined;
  }
}

async function resolveNotificationSound(
  sessionSoundType: SessionSoundType
): Promise<string | undefined> {
  if (useSettingsStore.getState().notificationSoundMuted) {
    return undefined;
  }

  if (isWindowsPlatform()) {
    return windowsSoundEventBySessionType[sessionSoundType];
  }

  return resolveSessionSoundPath(sessionSoundType);
}

async function playInAppSoundFallback(
  sessionSoundType: SessionSoundType
): Promise<void> {
  if (!isWindowsPlatform()) {
    return;
  }

  if (useSettingsStore.getState().notificationSoundMuted) {
    return;
  }

  try {
    const resolvedPath = await resolveSessionSoundPath(sessionSoundType);
    if (!resolvedPath) {
      return;
    }

    const volumePercent = useSettingsStore.getState().notificationSoundVolumePercent;
    const normalizedVolume = Math.max(0, Math.min(100, volumePercent)) / 100;
    const soundAssetUrl = convertFileSrc(resolvedPath.replace(/\\/g, '/'));
    const soundAudio = new Audio(soundAssetUrl);
    soundAudio.volume = normalizedVolume;
    await soundAudio.play();
  } catch (error) {
    console.warn('Failed to play in-app sound fallback:', {
      sessionSoundType,
      error,
    });
  }
}

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
export async function notifyWorkComplete(
  nextSessionType: SessionTypeValue | null
): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  const nextSessionSoundType =
    nextSessionType === 'long_break' ? 'long_break' : 'short_break';

  try {
    const soundPath = await resolveNotificationSound(nextSessionSoundType);

    sendNotification({
      title: '休憩時間です！',
      body: 'お疲れ様でした。休憩を取りましょう。',
      sound: soundPath,
    });

    await playInAppSoundFallback(nextSessionSoundType);
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
    const soundPath = await resolveNotificationSound('work');

    sendNotification({
      title: '作業に戻りましょう！',
      body: '休憩が終わりました。次のポモドーロを始めましょう。',
      sound: soundPath,
    });

    await playInAppSoundFallback('work');
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
    const soundPath = await resolveNotificationSound('work');

    sendNotification({
      title: '長い休憩完了！',
      body: 'サイクルが完了しました。次のサイクルを始めますか？',
      sound: soundPath,
    });

    await playInAppSoundFallback('work');
  } catch (error) {
    console.error('Failed to show long break complete notification:', error);
  }
}
