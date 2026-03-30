import { beforeEach, describe, expect, it, vi } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";

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

const audioInstances: MockAudio[] = [];
let queuedPlayError: Error | null = null;

class MockAudio {
  public src: string;
  public volume = 1;
  public currentTime = 0;
  public pause = vi.fn();
  public play = vi.fn(() => {
    if (queuedPlayError) {
      const error = queuedPlayError;
      queuedPlayError = null;
      return Promise.reject(error);
    }

    return Promise.resolve();
  });

  public constructor(src: string) {
    this.src = src;
    audioInstances.push(this);
  }
}

function getAudioInstance(index: number): MockAudio {
  const audioInstance = audioInstances[index];
  expect(audioInstance).toBeDefined();
  if (audioInstance === undefined) {
    throw new Error(`Expected audio instance at index ${String(index)}`);
  }

  return audioInstance;
}

describe("playNotificationSoundPreview()", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    audioInstances.length = 0;
    queuedPlayError = null;
    globalThis.Audio = MockAudio as unknown as typeof Audio;
  });

  it("should play preview sound with clamped max volume", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 150,
      muted: false,
    });

    expect(result).toEqual({ status: "played" });
    expect(resolveResource).toHaveBeenCalledWith("resources/sounds/work.wav");
    expect(convertFileSrc).toHaveBeenCalledWith("C:/sounds/work.wav");
    expect(audioInstances).toHaveLength(1);
    const previewAudio = getAudioInstance(0);
    expect(previewAudio.volume).toBe(1);
    expect(previewAudio.play).toHaveBeenCalledTimes(1);
  });

  it("should play preview sound with clamped min volume", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: -10,
      muted: false,
    });

    expect(result).toEqual({ status: "played" });
    expect(audioInstances).toHaveLength(1);
    const previewAudio = getAudioInstance(0);
    expect(previewAudio.volume).toBe(0);
    expect(previewAudio.play).toHaveBeenCalledTimes(1);
  });

  it.each([1, 10, 50, 60])(
    "should play preview sound with the shared nonlinear gain curve at %s percent",
    async (volumePercent) => {
      vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

      const {
        convertNotificationVolumePercentToGain,
        playNotificationSoundPreview,
      } = await import("./notifications");
      const result = await playNotificationSoundPreview({
        soundType: "work",
        volumePercent,
        muted: false,
      });

      expect(result).toEqual({ status: "played" });
      expect(audioInstances).toHaveLength(1);
      expect(getAudioInstance(0).volume).toBeCloseTo(
        convertNotificationVolumePercentToGain(volumePercent),
        10,
      );
    },
  );

  it("should skip playback when preview is muted", async () => {
    const { playNotificationSoundPreview } = await import("./notifications");
    const result = await playNotificationSoundPreview({
      soundType: "short_break",
      volumePercent: 40,
      muted: true,
    });

    expect(result).toEqual({ status: "muted" });
    expect(resolveResource).not.toHaveBeenCalled();
    expect(audioInstances).toHaveLength(0);
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
    expect(audioInstances).toHaveLength(0);
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

    expect(audioInstances).toHaveLength(2);
    const firstPreviewAudio = getAudioInstance(0);
    const secondPreviewAudio = getAudioInstance(1);
    expect(firstPreviewAudio.pause).toHaveBeenCalledTimes(1);
    expect(firstPreviewAudio.currentTime).toBe(0);
    expect(secondPreviewAudio.play).toHaveBeenCalledTimes(1);
  });

  it("should return error when audio playback fails", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");
    queuedPlayError = new Error("autoplay blocked");

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
