import { describe, expect, it } from "vitest";
import {
  describeTimerSyncFailure,
  formatTimerSyncConnectionErrorDetail,
} from "./timerSyncConnectionError";
describe("connection failure presentation", () => {
  it.each([
    {
      failure: { status: "rejected", error: { code: "rate_limited" } },
      canRetry: true,
      detail: "rate_limited",
    },
    {
      failure: {
        status: "rejected",
        error: { code: "invalid_state_transition" },
      },
      canRetry: false,
      detail: "invalid_state_transition",
    },
    {
      failure: { status: "unavailable", cause: { code: "rate_limited" } },
      canRetry: true,
      detail: "Command response unavailable",
    },
    {
      failure: { status: "setup", cause: new Error("listener setup") },
      canRetry: true,
      detail: "Error: listener setup",
    },
  ] as const)(
    "describes $detail without exposing a command transport cause",
    ({ failure, canRetry, detail }) => {
      expect(describeTimerSyncFailure(failure).canRetry).toBe(canRetry);
      expect(describeTimerSyncFailure(failure).userMessage).not.toBe("");
      expect(formatTimerSyncConnectionErrorDetail(failure)).toBe(detail);
    },
  );
});
