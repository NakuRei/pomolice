import { beforeEach, describe, expect, it, vi } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";
import {
  getMockHtmlAudioInstance,
  getMockHtmlAudioInstanceCount,
  installMockHtmlAudio,
  queueMockHtmlAudioPlayError,
  resetMockHtmlAudio,
} from "@/test/mockHtmlAudio";

const mockNotificationsLogger = {
  trace: vi.fn(),
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn(),
};

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn((path: string) => `asset://${path}`),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

vi.mock("@/logging/logger", () => ({
  createLogger: (): typeof mockNotificationsLogger => mockNotificationsLogger,
}));

describe("playNotificationSoundPreview()", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    resetMockHtmlAudio();
    installMockHtmlAudio();
  });

  it("should play preview sound", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 50,
      muted: false,
    });

    expect(result).toEqual({ status: "played" });
    expect(resolveResource).toHaveBeenCalledWith("resources/sounds/work.wav");
    expect(convertFileSrc).toHaveBeenCalledWith("C:/sounds/work.wav");
    expect(getMockHtmlAudioInstanceCount()).toBe(1);
    const previewAudio = getMockHtmlAudioInstance(0);
    expect(previewAudio.play).toHaveBeenCalledTimes(1);
  });

  it("should skip playback when preview is muted", async () => {
    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "short_break",
      volumePercent: 40,
      muted: true,
    });

    expect(result).toEqual({ status: "muted" });
    expect(resolveResource).not.toHaveBeenCalled();
    expect(getMockHtmlAudioInstanceCount()).toBe(0);
  });

  it("should return unavailable when resource resolution fails", async () => {
    vi.mocked(resolveResource).mockRejectedValueOnce(new Error("not found"));

    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "long_break",
      volumePercent: 70,
      muted: false,
    });

    expect(result).toEqual({ status: "unavailable" });
    expect(getMockHtmlAudioInstanceCount()).toBe(0);
    expect(mockNotificationsLogger.warn).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.notificationSoundPathResolutionFailed,
      expect.objectContaining({
        notification_sound_type: "long_break",
      }),
    );
  });

  it("should stop the previous preview before playing a new one", async () => {
    vi.mocked(resolveResource)
      .mockResolvedValueOnce("C:\\sounds\\work.wav")
      .mockResolvedValueOnce("C:\\sounds\\short_break.wav");

    const { playNotificationSoundPreview } = await import("./notifications");

    await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 30,
      muted: false,
    });
    await playNotificationSoundPreview({
      soundType: "short_break",
      volumePercent: 20,
      muted: false,
    });

    expect(getMockHtmlAudioInstanceCount()).toBe(2);
    const firstPreviewAudio = getMockHtmlAudioInstance(0);
    const secondPreviewAudio = getMockHtmlAudioInstance(1);
    expect(firstPreviewAudio.pause).toHaveBeenCalledTimes(1);
    expect(firstPreviewAudio.currentTime).toBe(0);
    expect(secondPreviewAudio.play).toHaveBeenCalledTimes(1);
  });

  it("should return error when audio playback fails", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");
    queueMockHtmlAudioPlayError(new Error("autoplay blocked"));

    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 55,
      muted: false,
    });

    expect(result).toEqual({ status: "error" });
    expect(mockNotificationsLogger.warn).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.notificationSoundPreviewPlaybackFailed,
      expect.objectContaining({
        notification_sound_type: "work",
      }),
    );
  });
});
