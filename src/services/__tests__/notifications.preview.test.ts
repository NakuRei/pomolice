import { beforeEach, describe, expect, it, vi } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";

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

const audioInstances: MockAudio[] = [];
let queuedPlayError: Error | null = null;

class MockAudio {
  src: string;
  volume = 1;
  currentTime = 0;
  pause = vi.fn();
  play = vi.fn(async () => {
    if (queuedPlayError) {
      const error = queuedPlayError;
      queuedPlayError = null;
      throw error;
    }
  });

  constructor(src: string) {
    this.src = src;
    audioInstances.push(this);
  }
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

    const { playNotificationSoundPreview } = await import("../notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 150,
      muted: false,
    });

    expect(result).toEqual({ status: "played" });
    expect(resolveResource).toHaveBeenCalledWith("resources/sounds/work.wav");
    expect(convertFileSrc).toHaveBeenCalledWith("C:/sounds/work.wav");
    expect(audioInstances).toHaveLength(1);
    expect(audioInstances[0].volume).toBe(1);
    expect(audioInstances[0].play).toHaveBeenCalledTimes(1);
  });

  it("should play preview sound with clamped min volume", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { playNotificationSoundPreview } = await import("../notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: -10,
      muted: false,
    });

    expect(result).toEqual({ status: "played" });
    expect(audioInstances).toHaveLength(1);
    expect(audioInstances[0].volume).toBe(0);
    expect(audioInstances[0].play).toHaveBeenCalledTimes(1);
  });

  it("should skip playback when preview is muted", async () => {
    const { playNotificationSoundPreview } = await import("../notifications");
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

    const { playNotificationSoundPreview } = await import("../notifications");
    const result = await playNotificationSoundPreview({
      soundType: "long_break",
      volumePercent: 70,
      muted: false,
    });

    expect(result).toEqual({ status: "unavailable" });
    expect(audioInstances).toHaveLength(0);
  });

  it("should stop the previous preview before playing a new one", async () => {
    vi.mocked(resolveResource)
      .mockResolvedValueOnce("C:\\sounds\\work.wav")
      .mockResolvedValueOnce("C:\\sounds\\short_break.wav");

    const { playNotificationSoundPreview } = await import("../notifications");

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
    expect(audioInstances[0].pause).toHaveBeenCalledTimes(1);
    expect(audioInstances[0].currentTime).toBe(0);
    expect(audioInstances[1].play).toHaveBeenCalledTimes(1);
  });

  it("should return error when audio playback fails", async () => {
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");
    queuedPlayError = new Error("autoplay blocked");

    const { playNotificationSoundPreview } = await import("../notifications");
    const result = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 55,
      muted: false,
    });

    expect(result).toEqual({ status: "error" });
  });
});
