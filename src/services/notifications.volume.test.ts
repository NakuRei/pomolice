import { describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  convertFileSrc: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}));

import { convertNotificationVolumePercentToGain } from "./notifications";

describe("convertNotificationVolumePercentToGain()", () => {
  it("returns exact mute and max gain values", () => {
    expect(convertNotificationVolumePercentToGain(0)).toBe(0);
    expect(convertNotificationVolumePercentToGain(100)).toBe(1);
  });

  it("matches the fixed dB-based gain checkpoints", () => {
    expect(convertNotificationVolumePercentToGain(1)).toBeCloseTo(
      0.005922430868761305,
      10,
    );
    expect(convertNotificationVolumePercentToGain(10)).toBeCloseTo(
      0.009440608762859235,
      10,
    );
    expect(convertNotificationVolumePercentToGain(50)).toBeCloseTo(
      0.07498942093324558,
      10,
    );
    expect(convertNotificationVolumePercentToGain(60)).toBeCloseTo(
      0.12589254117941673,
      10,
    );
  });

  it("clamps out-of-range percentages before converting to gain", () => {
    expect(convertNotificationVolumePercentToGain(-10)).toBe(0);
    expect(convertNotificationVolumePercentToGain(150)).toBe(1);
  });

  it("increases monotonically across the full 0-100 percent range", () => {
    let previousGain = -1;

    for (let percent = 0; percent <= 100; percent += 1) {
      const currentGain = convertNotificationVolumePercentToGain(percent);
      expect(currentGain).toBeGreaterThanOrEqual(previousGain);
      previousGain = currentGain;
    }
  });
});
