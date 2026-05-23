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

  it("stores structured backend responses with safe technical detail", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      {
        code: "window_control_error",
        message: "failed to subscribe to window events",
      },
      retryFn,
    );

    expect(useErrorStore.getState().errors.connection?.technicalDetail).toBe(
      "window_control_error: failed to subscribe to window events",
    );
  });

  it("does not stringify unknown object technical details", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      {
        code: "unknown_error",
        message: "internal path C:\\secret",
      },
      retryFn,
    );

    expect(useErrorStore.getState().errors.connection?.technicalDetail).toBe(
      "Unexpected backend error",
    );
  });

  it("clears the connection error", () => {
    const retryFn = vi.fn(() => Promise.resolve());
    reportTimerSyncConnectionError(new Error("setup failed"), retryFn);

    clearTimerSyncConnectionError();

    expect(useErrorStore.getState().errors.connection).toBeUndefined();
  });
});
