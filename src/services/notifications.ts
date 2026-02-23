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

const notificationSoundResourcePathByType = {
  work: 'resources/sounds/work.wav',
  short_break: 'resources/sounds/short_break.wav',
  long_break: 'resources/sounds/long_break.wav',
} as const;

/**
 * Notification sound categories used by both timer notifications and settings preview.
 *
 * Usage examples:
 * - `const type: NotificationSoundType = "work";`
 * - `await resolveNotificationSoundResourcePath("short_break");`
 */
export type NotificationSoundType = keyof typeof notificationSoundResourcePathByType;

const resolvedNotificationSoundPathByType: Partial<
  Record<NotificationSoundType, string>
> = {};
let currentNotificationPreviewAudio: HTMLAudioElement | null = null;

const windowsSoundEventByType: Record<NotificationSoundType, string> = {
  work: 'ms-winsoundevent:Notification.IM',
  short_break: 'ms-winsoundevent:Notification.Mail',
  long_break: 'ms-winsoundevent:Notification.Reminder',
};

/**
 * Input for previewing notification sounds from settings without using persisted store values.
 *
 * Usage examples:
 * - `await playNotificationSoundPreview({ soundType: "work", volumePercent: 35, muted: false });`
 * - `await playNotificationSoundPreview({ soundType: "long_break", volumePercent: 80, muted: true });`
 */
export interface NotificationSoundPreviewOptions {
  soundType: NotificationSoundType;
  volumePercent: number;
  muted: boolean;
}

export type NotificationSoundPreviewStatus =
  | 'played'
  | 'muted'
  | 'unavailable'
  | 'error';

export interface NotificationSoundPreviewResult {
  status: NotificationSoundPreviewStatus;
}

interface SessionCompleteNotificationPayload {
  title: string;
  body: string;
  soundType: NotificationSoundType;
}

function isWindowsPlatform(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    navigator.userAgent.toLowerCase().includes('windows')
  );
}

export async function resolveNotificationSoundResourcePath(
  notificationSoundType: NotificationSoundType
): Promise<string | undefined> {
  const cachedPath = resolvedNotificationSoundPathByType[notificationSoundType];
  if (cachedPath) {
    return cachedPath;
  }

  try {
    const resourcePath = notificationSoundResourcePathByType[notificationSoundType];
    const resolvedPath = await resolveResource(resourcePath);
    resolvedNotificationSoundPathByType[notificationSoundType] = resolvedPath;
    return resolvedPath;
  } catch (error) {
    console.warn('Failed to resolve notification sound path:', {
      notificationSoundType,
      error,
    });
    return undefined;
  }
}

function stopCurrentNotificationPreviewAudio(): void {
  if (!currentNotificationPreviewAudio) {
    return;
  }

  currentNotificationPreviewAudio.pause();
  currentNotificationPreviewAudio.currentTime = 0;
  currentNotificationPreviewAudio = null;
}

/**
 * Play a settings preview tone using caller-provided values instead of persisted settings state.
 */
export async function playNotificationSoundPreview(
  options: NotificationSoundPreviewOptions
): Promise<NotificationSoundPreviewResult> {
  if (options.muted) {
    stopCurrentNotificationPreviewAudio();
    return { status: 'muted' };
  }

  const resolvedPath = await resolveNotificationSoundResourcePath(options.soundType);
  if (!resolvedPath) {
    return { status: 'unavailable' };
  }

  try {
    const normalizedVolume = Math.max(0, Math.min(100, options.volumePercent)) / 100;
    const soundAssetUrl = convertFileSrc(resolvedPath.replace(/\\/g, '/'));
    const previewAudio = new Audio(soundAssetUrl);

    previewAudio.volume = normalizedVolume;
    stopCurrentNotificationPreviewAudio();
    currentNotificationPreviewAudio = previewAudio;
    await previewAudio.play();
    return { status: 'played' };
  } catch (error) {
    console.warn('Failed to play notification sound preview:', {
      notificationSoundType: options.soundType,
      error,
    });
    return { status: 'error' };
  }
}

async function resolveNotificationSound(
  notificationSoundType: NotificationSoundType
): Promise<string | undefined> {
  if (useSettingsStore.getState().notificationSoundMuted) {
    return undefined;
  }

  if (isWindowsPlatform()) {
    return windowsSoundEventByType[notificationSoundType];
  }

  return resolveNotificationSoundResourcePath(notificationSoundType);
}

async function playInAppSoundFallback(
  notificationSoundType: NotificationSoundType
): Promise<void> {
  if (!isWindowsPlatform()) {
    return;
  }

  if (useSettingsStore.getState().notificationSoundMuted) {
    return;
  }

  try {
    const resolvedPath = await resolveNotificationSoundResourcePath(notificationSoundType);
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
      notificationSoundType,
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
  const nextSessionSoundType =
    nextSessionType === 'long_break' ? 'long_break' : 'short_break';

  await sendSessionCompleteNotification({
    title: '休憩時間です！',
    body: 'お疲れ様でした。休憩を取りましょう。',
    soundType: nextSessionSoundType,
  });
}

async function sendSessionCompleteNotification(
  payload: SessionCompleteNotificationPayload
): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  try {
    const soundPath = await resolveNotificationSound(payload.soundType);

    sendNotification({
      title: payload.title,
      body: payload.body,
      sound: soundPath,
    });

    await playInAppSoundFallback(payload.soundType);
  } catch (error) {
    console.error('Failed to show session complete notification:', {
      payload,
      error,
    });
  }
}

/**
 * Show notification when break session is complete
 */
export async function notifyBreakComplete(): Promise<void> {
  await sendSessionCompleteNotification({
    title: '作業に戻りましょう！',
    body: '休憩が終わりました。次のポモドーロを始めましょう。',
    soundType: 'work',
  });
}

/**
 * Show notification when long break session is complete
 */
export async function notifyLongBreakComplete(): Promise<void> {
  await sendSessionCompleteNotification({
    title: '長い休憩完了！',
    body: 'サイクルが完了しました。次のサイクルを始めますか？',
    soundType: 'work',
  });
}
