import { beforeEach, describe, expect, it, vi } from "vitest";
import { convertFileSrc } from "@tauri-apps/api/core";
import { resolveResource } from "@tauri-apps/api/path";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";

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

describe("session complete notifications", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
    audioInstances.length = 0;
    globalThis.Audio = MockAudio as unknown as typeof Audio;
    setNavigatorUserAgent("Windows NT 10.0");
  });

  it("shows a silent Windows banner and plays one managed notification sound with the shared gain curve", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { useSettingsStore } = await import("@/store/settingsStore");
    useSettingsStore.getState().reset();
    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 60,
    });

    const { convertNotificationVolumePercentToGain, notifyBreakComplete } =
      await import("./notifications");

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
    expect(audioInstances).toHaveLength(1);
    expect(getAudioInstance(0).volume).toBeCloseTo(
      convertNotificationVolumePercentToGain(60),
      10,
    );
    expect(getAudioInstance(0).play).toHaveBeenCalledTimes(1);
  });

  it("keeps non-Windows notifications on the native notification sound path", async () => {
    setNavigatorUserAgent("Macintosh");
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(resolveResource).mockResolvedValueOnce("C:\\sounds\\work.wav");

    const { useSettingsStore } = await import("@/store/settingsStore");
    useSettingsStore.getState().reset();
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
    expect(audioInstances).toHaveLength(0);
  });

  it("does not show a banner or play sound when notification permission is denied", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(false);
    vi.mocked(requestPermission).mockResolvedValue("denied");

    const { useSettingsStore } = await import("@/store/settingsStore");
    useSettingsStore.getState().reset();

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(requestPermission).toHaveBeenCalledTimes(1);
    expect(sendNotification).not.toHaveBeenCalled();
    expect(resolveResource).not.toHaveBeenCalled();
    expect(audioInstances).toHaveLength(0);
  });

  it("logs an error when notification permission lookup fails", async () => {
    vi.mocked(isPermissionGranted).mockRejectedValueOnce(
      new Error("permission lookup failed"),
    );

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(mockNotificationsLogger.error).toHaveBeenCalledWith(
      "permission_request_failed",
      expect.any(Error),
    );
    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("keeps the Windows banner but suppresses all notification sound when muted", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);

    const { useSettingsStore } = await import("@/store/settingsStore");
    useSettingsStore.getState().reset();
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
    expect(audioInstances).toHaveLength(0);
  });

  it("logs an error when notification display fails", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.mocked(sendNotification).mockImplementationOnce(() => {
      throw new Error("display failed");
    });

    const { notifyBreakComplete } = await import("./notifications");
    await notifyBreakComplete();

    expect(mockNotificationsLogger.error).toHaveBeenCalledWith(
      "session_complete_notification_failed",
      expect.any(Error),
      expect.objectContaining({
        notification_sound_type: "work",
        notification_title: "作業に戻りましょう！",
      }),
    );
  });
});
