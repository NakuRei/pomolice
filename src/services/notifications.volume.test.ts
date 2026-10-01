import { beforeEach, describe, expect, it, vi } from "vitest";
import { resolveResource } from "@tauri-apps/api/path";
import {
  getMockHtmlAudioInstance,
  getMockHtmlAudioInstanceCount,
  installMockHtmlAudio,
  type MockHtmlAudio,
  resetMockHtmlAudio,
} from "@/test/mockHtmlAudio";
import type { Settings } from "@/types/settings";

async function setSavedSettings(
  overrides: Partial<Settings> | null,
): Promise<void> {
  const { useTimerObservationStore } =
    await import("@/store/timerObservationStore");
  const { createSettingsForTest } = await import("@/test/settingsFixture");
  useTimerObservationStore.setState({
    settings: overrides === null ? null : createSettingsForTest(overrides),
  });
}

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

async function playPreviewAtVolumePercent(
  volumePercent: number,
): Promise<MockHtmlAudio> {
  const audioInstanceCountBeforePreview = getMockHtmlAudioInstanceCount();
  vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

  const { playNotificationSoundPreview } = await import("./notifications");
  const result = await playNotificationSoundPreview({
    soundType: "work",
    volumePercent,
    muted: false,
  });

  expect(result).toEqual({ status: "played" });
  expect(getMockHtmlAudioInstanceCount()).toBe(
    audioInstanceCountBeforePreview + 1,
  );
  return getMockHtmlAudioInstance(audioInstanceCountBeforePreview);
}

describe("notification volume gain", { concurrent: false }, () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    resetMockHtmlAudio();
    installMockHtmlAudio();
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
    [-10, 0],
    [150, 1],
  ])(
    "clamps %s percent before applying gain",
    async (volumePercent, expectedGain) => {
      const previewAudio = await playPreviewAtVolumePercent(volumePercent);

      expect(previewAudio.volume).toBe(expectedGain);
    },
  );

  it("does not decrease gain when volume percent increases", async () => {
    const gainAt10 = (await playPreviewAtVolumePercent(10)).volume;
    const gainAt50 = (await playPreviewAtVolumePercent(50)).volume;
    const gainAt60 = (await playPreviewAtVolumePercent(60)).volume;

    expect(gainAt10).toBeGreaterThan(0);
    expect(gainAt50).toBeGreaterThanOrEqual(gainAt10);
    expect(gainAt60).toBeGreaterThanOrEqual(gainAt50);
    expect(gainAt60).toBeLessThan(1);
  });

  it("keeps intermediate volume between silence and maximum", async () => {
    const previewAudio = await playPreviewAtVolumePercent(60);

    expect(previewAudio.volume).toBeGreaterThan(0);
    expect(previewAudio.volume).toBeLessThan(1);
  });

  it("uses caller-provided volume percent instead of persisted settings", async () => {
    await setSavedSettings({ notificationSoundVolumePercent: 100 });

    const silentPreviewAudio = await playPreviewAtVolumePercent(0);
    const audiblePreviewAudio = await playPreviewAtVolumePercent(60);

    expect(silentPreviewAudio.volume).toBe(0);
    expect(audiblePreviewAudio.volume).toBeGreaterThan(0);
    expect(audiblePreviewAudio.volume).toBeLessThan(1);
  });
});
