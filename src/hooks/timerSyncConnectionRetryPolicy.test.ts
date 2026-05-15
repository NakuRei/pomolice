import { describe, expect, it } from "vitest";
import {
  shouldRetryTimerSyncConnection,
  TIMER_SYNC_MAX_CONNECTION_RETRIES,
  timerSyncRetryDelayMs,
} from "./timerSyncConnectionRetryPolicy";

describe("timerSyncConnectionRetryPolicy", () => {
  it("uses the existing connection retry limit", () => {
    expect(TIMER_SYNC_MAX_CONNECTION_RETRIES).toBe(3);
  });

  it("preserves exponential retry delays for the first three attempts", () => {
    expect(timerSyncRetryDelayMs(0)).toBe(1000);
    expect(timerSyncRetryDelayMs(1)).toBe(2000);
    expect(timerSyncRetryDelayMs(2)).toBe(4000);
  });

  it("continues retrying only while the attempt is below the limit", () => {
    expect(shouldRetryTimerSyncConnection(0)).toBe(true);
    expect(shouldRetryTimerSyncConnection(1)).toBe(true);
    expect(shouldRetryTimerSyncConnection(2)).toBe(true);
    expect(shouldRetryTimerSyncConnection(3)).toBe(false);
  });
});
