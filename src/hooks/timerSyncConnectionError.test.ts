import { beforeEach, describe, expect, it, vi } from "vitest";
import { useErrorStore } from "@/store/errorStore";
import {
  clearTimerSyncConnectionError,
  reportTimerSyncConnectionError,
} from "./timerSyncConnectionError";

describe("timerSyncConnectionError", () => {
  beforeEach(() => {
    useErrorStore.getState().clearAllErrors();
  });

  it("reports the preserved connection error contract", () => {
    const retryFn = vi.fn(() => Promise.resolve());
    const error = new Error("setup failed");

    reportTimerSyncConnectionError(error, retryFn);

    expect(useErrorStore.getState().errors.connection).toMatchObject({
      id: "connection",
      message: "バックエンドに接続できません",
      technicalDetail: "Error: setup failed",
      severity: "critical",
      origin: "backend_connection",
      retryCount: 0,
      canRetry: true,
      retryFn,
    });
    expect(useErrorStore.getState().errors.connection?.timestamp).toEqual(
      expect.any(Number),
    );
  });

  it("stringifies non-error technical details", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError("listen failed", retryFn);

    expect(useErrorStore.getState().errors.connection?.technicalDetail).toBe(
      "listen failed",
    );
  });

  it("clears the connection error", () => {
    const retryFn = vi.fn(() => Promise.resolve());
    reportTimerSyncConnectionError(new Error("setup failed"), retryFn);

    clearTimerSyncConnectionError();

    expect(useErrorStore.getState().errors.connection).toBeUndefined();
  });
});
