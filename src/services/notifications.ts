/**
 * Notifications Service
 * Wrapper for Tauri notification commands
 */

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import { createLogger } from "../logging/logger";
import { FRONTEND_LOG_EVENT_IDS } from "../logging/logEvents";

const notificationSoundResourcePathByType = {
  work: "resources/sounds/work.wav",
  short_break: "resources/sounds/short_break.wav",
  long_break: "resources/sounds/long_break.wav",
} as const;

// Keep persisted 0-100% settings, but map them onto a dB curve so
// low-volume adjustments have finer audible resolution than linear gain.
const MIN_NOTIFICATION_VOLUME_DB = -45;

/**
 * Notification sound categories used by both timer notifications and settings preview.
 *
 * Usage examples:
 * - `const type: NotificationSoundType = "work";`
 * - `await resolveNotificationSoundResourcePath("short_break");`
 */
export type NotificationSoundType =
  keyof typeof notificationSoundResourcePathByType;

const resolvedNotificationSoundPathByType: Partial<
  Record<NotificationSoundType, string>
> = {};
let currentNotificationPreviewAudio: HTMLAudioElement | null = null;

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
  | "played"
  | "muted"
  | "unavailable"
  | "error";

export interface NotificationSoundPreviewResult {
  status: NotificationSoundPreviewStatus;
}

export interface NotificationSoundPreferences {
  muted: boolean;
  volumePercent: number;
}

export interface NotificationPayload {
  title: string;
  body: string;
  soundType: NotificationSoundType;
}
const logger = createLogger("notifications");

function isWindowsPlatform(): boolean {
  return (
    typeof navigator !== "undefined" &&
    navigator.userAgent.toLowerCase().includes("windows")
  );
}

/**
 * Convert notification volume percent into an Audio gain, preserving silence
 * at 0% and maximum gain at 100%.
 */
function convertNotificationVolumePercentToGain(volumePercent: number): number {
  const clampedVolumePercent = Math.max(0, Math.min(100, volumePercent));

  if (clampedVolumePercent === 0) {
    return 0;
  }

  const normalizedVolumePercent = clampedVolumePercent / 100;
  const gainDb = MIN_NOTIFICATION_VOLUME_DB * (1 - normalizedVolumePercent);
  return 10 ** (gainDb / 20);
}

export async function resolveNotificationSoundResourcePath(
  notificationSoundType: NotificationSoundType,
): Promise<string | undefined> {
  const cachedPath = resolvedNotificationSoundPathByType[notificationSoundType];
  if (cachedPath !== undefined) {
    return cachedPath;
  }

  try {
    const resourcePath =
      notificationSoundResourcePathByType[notificationSoundType];
    const resolvedPath = await resolveResource(resourcePath);
    const latestCachedPath =
      resolvedNotificationSoundPathByType[notificationSoundType];
    if (latestCachedPath !== undefined) {
      return latestCachedPath;
    }
    Object.assign(resolvedNotificationSoundPathByType, {
      [notificationSoundType]: resolvedPath,
    });
    return resolvedPath;
  } catch (error) {
    logger.warn(FRONTEND_LOG_EVENT_IDS.notificationSoundPathResolutionFailed, {
      notification_sound_type: notificationSoundType,
      failure_message: String(error),
    });
    return undefined;
  }
}

async function playNotificationAudioAtGain(
  resolvedPath: string,
  gain: number,
): Promise<HTMLAudioElement> {
  const soundAssetUrl = convertFileSrc(resolvedPath.replace(/\\/gu, "/"));
  const notificationAudio = new Audio(soundAssetUrl);
  notificationAudio.volume = gain;
  await notificationAudio.play();
  return notificationAudio;
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
  options: NotificationSoundPreviewOptions,
): Promise<NotificationSoundPreviewResult> {
  if (options.muted) {
    stopCurrentNotificationPreviewAudio();
    return { status: "muted" };
  }

  const resolvedPath = await resolveNotificationSoundResourcePath(
    options.soundType,
  );
  if (resolvedPath === undefined) {
    return { status: "unavailable" };
  }

  try {
    const previewGain = convertNotificationVolumePercentToGain(
      options.volumePercent,
    );
    stopCurrentNotificationPreviewAudio();
    const previewAudio = await playNotificationAudioAtGain(
      resolvedPath,
      previewGain,
    );
    currentNotificationPreviewAudio = previewAudio;
    return { status: "played" };
  } catch (error) {
    logger.warn(FRONTEND_LOG_EVENT_IDS.notificationSoundPreviewPlaybackFailed, {
      notification_sound_type: options.soundType,
      failure_message: String(error),
    });
    return { status: "error" };
  }
}

function showSessionCompleteNotificationBanner(
  notificationTitle: string,
  notificationBody: string,
  notificationSound?: string,
): void {
  if (notificationSound === undefined) {
    sendNotification({
      title: notificationTitle,
      body: notificationBody,
    });
    return;
  }

  sendNotification({
    title: notificationTitle,
    body: notificationBody,
    sound: notificationSound,
  });
}

/**
 * Check if we're running in Tauri environment
 */
function isTauriEnvironment(): boolean {
  // In Tauri v2, check if we're in a Tauri context by checking if window.__TAURI_INTERNALS__ exists
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
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
      permissionGranted = permission === "granted";
    }

    return permissionGranted;
  } catch (error) {
    logger.error(
      FRONTEND_LOG_EVENT_IDS.notificationPermissionRequestFailed,
      error,
      {
        failure_message: String(error),
      },
    );
    return false;
  }
}

/**
 * Show notification after a session transition that should alert the user.
 */
export async function sendSystemNotification(
  payload: NotificationPayload,
  readSoundPreferences: () => NotificationSoundPreferences | null,
): Promise<void> {
  const hasPermission = await ensureNotificationPermission();
  if (!hasPermission) {
    return;
  }

  try {
    const preferences = readSoundPreferences();
    // Without settings from the backend the user's sound choice is unknown,
    // so no sound is played.
    const soundIsMuted = preferences === null || preferences.muted;
    const shouldUseManagedWindowsSound = isWindowsPlatform() && !soundIsMuted;
    const bannerSound =
      isWindowsPlatform() || soundIsMuted
        ? undefined
        : await resolveNotificationSoundResourcePath(payload.soundType);

    showSessionCompleteNotificationBanner(
      payload.title,
      payload.body,
      bannerSound,
    );

    if (!shouldUseManagedWindowsSound) {
      return;
    }

    const resolvedPath = await resolveNotificationSoundResourcePath(
      payload.soundType,
    );
    if (resolvedPath === undefined) {
      return;
    }

    const notificationGain = convertNotificationVolumePercentToGain(
      preferences.volumePercent,
    );
    await playNotificationAudioAtGain(resolvedPath, notificationGain);
  } catch (error) {
    logger.error(
      FRONTEND_LOG_EVENT_IDS.notificationSessionCompleteNotificationFailed,
      error,
      {
        notification_sound_type: payload.soundType,
        notification_title: payload.title,
        failure_message: String(error),
      },
    );
  }
}
