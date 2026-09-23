import { beforeEach, describe, expect, it, vi } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { FRONTEND_LOG_EVENT_IDS } from "@/logging/logEvents";
import {
  getMockHtmlAudioInstance,
  getMockHtmlAudioInstanceCount,
  installMockHtmlAudio,
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

function getSendNotificationPayload(callIndex: number): unknown {
  const notificationCall = vi.mocked(sendNotification).mock.calls[callIndex];
  expect(notificationCall).toBeDefined();
  if (notificationCall === undefined) {
    throw new Error(
      `Expected sendNotification call at index ${String(callIndex)}`,
    );
  }

  return notificationCall[0];
}

function setNavigatorUserAgent(userAgent: string): void {
  Object.defineProperty(window.navigator, "userAgent", {
    configurable: true,
    value: userAgent,
  });
}

async function playWindowsManagedBreakCompleteNotificationAtVolumePercent(
  volumePercent: number,
): Promise<number> {
  vi.mocked(isPermissionGranted).mockResolvedValue(true);
  vi.mocked(resolveResource).mockResolvedValue("C:\\sounds\\work.wav");

  const { useSettingsStore } = await import("@/store/settingsStore");
  const { resetSettingsStoreForTest } =
    await import("@/test/resetSettingsStore");
  resetSettingsStoreForTest();
  useSettingsStore.setState({
    notificationSoundMuted: false,
    notificationSoundVolumePercent: volumePercent,
  });

  const audioInstanceCountBeforeNotification = getMockHtmlAudioInstanceCount();
  const { notifyBreakComplete } = await import("./notifications");
  await notifyBreakComplete();

  expect(getMockHtmlAudioInstanceCount()).toBe(
    audioInstanceCountBeforeNotification + 1,
  );
  return getMockHtmlAudioInstance(audioInstanceCountBeforeNotification).volume;
}

describe("session complete notifications", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    resetMockHtmlAudio();
    installMockHtmlAudio();
    setNavigatorUserAgent("Windows NT 10.0");
  });

  it("shows a silent Windows banner and plays one managed notification sound with the configured volume", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { useSettingsStore } = await import("@/store/settingsStore");
    const { resetSettingsStoreForTest } =
      await import("@/test/resetSettingsStore");
    resetSettingsStoreForTest();
    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 60,
    });

    const { notifyBreakComplete } = await import("./notifications");

    await notifyBreakComplete();

    expect(sendNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification).toHaveBeenCalledWith({
      title: "作業に戻りましょう！",
      body: "休憩が終わりました。次のポモドーロを始めましょう。",
    });

    const bannerCall = getSendNotificationPayload(0);
    expect(typeof bannerCall).toBe("object");
    expect(bannerCall).not.toBeNull();
    expect(Object.prototype.hasOwnProperty.call(bannerCall, "sound")).toBe(
      false,
    );

    expect(resolveResource).toHaveBeenCalledWith("resources/sounds/work.wav");
    expect(convertFileSrc).toHaveBeenCalledWith("C:/sounds/work.wav");
    expect(getMockHtmlAudioInstanceCount()).toBe(1);
    const managedNotificationAudio = getMockHtmlAudioInstance(0);
    expect(managedNotificationAudio.volume).toBeGreaterThan(0);
    expect(managedNotificationAudio.volume).toBeLessThan(1);
    expect(managedNotificationAudio.play).toHaveBeenCalledTimes(1);
  });

  it("plays Windows managed notification louder when saved volume percent is higher", async () => {
    const gainAt30 =
      await playWindowsManagedBreakCompleteNotificationAtVolumePercent(30);
    const gainAt80 =
      await playWindowsManagedBreakCompleteNotificationAtVolumePercent(80);

    expect(gainAt30).toBeGreaterThan(0);
    expect(gainAt80).toBeGreaterThan(gainAt30);
    expect(gainAt80).toBeLessThan(1);
  });

  it("uses the same gain for preview and Windows managed notification at the same volume percent", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(resolveResource).mockResolvedValue("C:\\sounds\\work.wav");

    const { playNotificationSoundPreview, notifyBreakComplete } =
      await import("./notifications");
    const previewResult = await playNotificationSoundPreview({
      soundType: "work",
      volumePercent: 60,
      muted: false,
    });
    expect(previewResult).toEqual({ status: "played" });
    const previewVolume = getMockHtmlAudioInstance(0).volume;

    const { useSettingsStore } = await import("@/store/settingsStore");
    const { resetSettingsStoreForTest } =
      await import("@/test/resetSettingsStore");
    resetSettingsStoreForTest();
    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 60,
    });

    await notifyBreakComplete();

    expect(getMockHtmlAudioInstance(1).volume).toBe(previewVolume);
  });

  it("keeps non-Windows notifications on the native notification sound path", async () => {
    setNavigatorUserAgent("Macintosh");
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { useSettingsStore } = await import("@/store/settingsStore");
    const { resetSettingsStoreForTest } =
      await import("@/test/resetSettingsStore");
    resetSettingsStoreForTest();
    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 60,
    });

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(sendNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification).toHaveBeenCalledWith({
      title: "作業に戻りましょう！",
      body: "休憩が終わりました。次のポモドーロを始めましょう。",
      sound: "C:\\sounds\\work.wav",
    });
    expect(getMockHtmlAudioInstanceCount()).toBe(0);
  });

  it("does not show a banner or play sound when notification permission is denied", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockResolvedValue("denied");

    const { resetSettingsStoreForTest } =
      await import("@/test/resetSettingsStore");
    resetSettingsStoreForTest();

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(sendNotification).not.toHaveBeenCalled();
    expect(resolveResource).not.toHaveBeenCalled();
    expect(getMockHtmlAudioInstanceCount()).toBe(0);
  });

  it("logs an error when notification permission lookup fails", async () => {
    vi.mocked(isPermissionGranted).mockRejectedValueOnce(
      new Error("permission lookup failed"),
    );

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(mockNotificationsLogger.error).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.notificationPermissionRequestFailed,
      expect.any(Error),
      expect.objectContaining({
        failure_message: "Error: permission lookup failed",
      }),
    );
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("keeps the Windows banner but suppresses all notification sound when muted", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);

    const { useSettingsStore } = await import("@/store/settingsStore");
    const { resetSettingsStoreForTest } =
      await import("@/test/resetSettingsStore");
    resetSettingsStoreForTest();
    useSettingsStore.setState({
      notificationSoundMuted: true,
      notificationSoundVolumePercent: 60,
    });

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(sendNotification).toHaveBeenCalledTimes(1);
    expect(sendNotification).toHaveBeenCalledWith({
      title: "作業に戻りましょう！",
      body: "休憩が終わりました。次のポモドーロを始めましょう。",
    });
    expect(resolveResource).not.toHaveBeenCalled();
    expect(getMockHtmlAudioInstanceCount()).toBe(0);
  });

  it("logs an error when notification display fails", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(sendNotification).mockImplementationOnce(() => {
      throw new Error("display failed");
    });

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(mockNotificationsLogger.error).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.notificationSessionCompleteNotificationFailed,
      expect.any(Error),
      expect.objectContaining({
        notification_sound_type: "work",
        notification_title: "作業に戻りましょう！",
        failure_message: "Error: display failed",
      }),
    );
  });
});
