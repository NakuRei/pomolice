import { beforeEach, describe, expect, it, vi } from "vitest";
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

class MockAudio {
  public src: string;
  public volume = 1;
  public currentTime = 0;
  public pause = vi.fn();
  public play = vi.fn(() => Promise.resolve());

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

async function playPreviewAtVolumePercent(
  volumePercent: number,
): Promise<MockAudio> {
  const audioInstanceCountBeforePreview = audioInstances.length;
  vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

  const { playNotificationSoundPreview } = await import("./notifications");
  const result = await playNotificationSoundPreview({
    soundType: "work",
    volumePercent,
    muted: false,
  });

  expect(result).toEqual({ status: "played" });
  expect(audioInstances).toHaveLength(audioInstanceCountBeforePreview + 1);
  return getAudioInstance(audioInstanceCountBeforePreview);
}

describe("notification volume gain", { concurrent: false }, () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    audioInstances.length = 0;
    globalThis.Audio = MockAudio as unknown as typeof Audio;
  });

  it.each([
    [0, 0],
    [100, 1],
  ])(
    "maps %s percent to exact gain %s",
    async (volumePercent, expectedGain) => {
      const previewAudio = await playPreviewAtVolumePercent(volumePercent);

      expect(previewAudio.volume).toBe(expectedGain);
    },
  );

  it.each([
    [1, 0.005922430868761305],
    [10, 0.009440608762859235],
    [50, 0.07498942093324558],
    [60, 0.12589254117941673],
  ])(
    "maps %s percent to notification gain %s",
    async (volumePercent, expectedGain) => {
      const previewAudio = await playPreviewAtVolumePercent(volumePercent);

      expect(previewAudio.volume).toBeCloseTo(expectedGain, 10);
    },
  );

  it.each([
    [-10, 0],
    [150, 1],
  ])(
    "clamps %s percent before applying gain",
    async (volumePercent, expectedGain) => {
      const previewAudio = await playPreviewAtVolumePercent(volumePercent);

      expect(previewAudio.volume).toBe(expectedGain);
    },
  );
});
