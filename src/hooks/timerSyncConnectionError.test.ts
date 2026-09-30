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

  function expectTechnicalDetailExcludesDiagnostics(
    technicalDetail: string | undefined,
    diagnosticFragments: string[],
  ): void {
    expect(technicalDetail).toEqual(expect.any(String));
    expect(technicalDetail).not.toBe("");

    for (const fragment of diagnosticFragments) {
      expect(technicalDetail).not.toContain(fragment);
    }
  }

  it("reports the preserved connection error contract", () => {
    const retryFn = vi.fn(() => Promise.resolve());
    const error = new Error("setup failed");

    reportTimerSyncConnectionError({ status: "setup", cause: error }, retryFn);

    expect(useErrorStore.getState().errors.connection).toMatchObject({
      id: "connection",
      message:
        "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
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

    reportTimerSyncConnectionError(
      { status: "setup", cause: "listen failed" },
      retryFn,
    );

    expect(useErrorStore.getState().errors.connection?.technicalDetail).toBe(
      "listen failed",
    );
  });

  it("stores command-source public code details without raw backend messages", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      {
        status: "rejected",
        error: {
          code: "window_control_error",
        },
      },
      retryFn,
    );

    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      technicalDetail: "window_control_error",
      severity: "recoverable",
      origin: "backend_connection",
      canRetry: true,
      retryFn,
    });
    expect(connectionError?.message).toEqual(expect.any(String));
    expect(connectionError?.message).not.toBe("");
    expect(connectionError?.message).not.toContain("failed to read state");
    expect(connectionError?.message).not.toContain("C:\\Users\\nak");
    expect(connectionError?.message).not.toContain("secret");
  });

  it("derives command-source connection presentation from public code", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      {
        status: "rejected",
        error: {
          code: "window_control_error",
        },
      },
      retryFn,
    );
    const retryableMessage =
      useErrorStore.getState().errors.connection?.message;
    expect(useErrorStore.getState().errors.connection).toMatchObject({
      id: "connection",
      severity: "recoverable",
      canRetry: true,
      retryFn,
    });

    reportTimerSyncConnectionError(
      {
        status: "rejected",
        error: {
          code: "invalid_state_transition",
        },
      },
      retryFn,
    );
    const nonRetryableMessage =
      useErrorStore.getState().errors.connection?.message;

    expect(useErrorStore.getState().errors.connection).toMatchObject({
      id: "connection",
      severity: "recoverable",
      canRetry: false,
      retryFn: null,
    });
    expect(nonRetryableMessage).toEqual(expect.any(String));
    expect(nonRetryableMessage).not.toBe("");
    expect(nonRetryableMessage).not.toBe(retryableMessage);
    expect(nonRetryableMessage).not.toContain("invalid type");
    expect(nonRetryableMessage).not.toContain("C:\\secret");
  });

  it("does not stringify unknown object technical details", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      {
        status: "setup",
        cause: {
          code: "unknown_error",
          message: "internal path C:\\secret",
        },
      },
      retryFn,
    );

    expect(useErrorStore.getState().errors.connection?.technicalDetail).toBe(
      "Unexpected backend error",
    );
  });

  it("uses safe fallback presentation for malformed command-source failures", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    reportTimerSyncConnectionError(
      { status: "unavailable", cause: "panic at C:\\secret" },
      retryFn,
    );

    const connectionError = useErrorStore.getState().errors.connection;
    expect(connectionError).toMatchObject({
      id: "connection",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
      retryFn,
    });
    expectTechnicalDetailExcludesDiagnostics(connectionError?.technicalDetail, [
      "panic",
      "C:\\secret",
    ]);
    expect(connectionError?.message).toEqual(expect.any(String));
    expect(connectionError?.message).not.toBe("");
    expect(connectionError?.message).not.toContain("panic");
    expect(connectionError?.message).not.toContain("C:\\secret");
  });

  it("clears the connection error", () => {
    const retryFn = vi.fn(() => Promise.resolve());
    reportTimerSyncConnectionError(
      { status: "setup", cause: new Error("setup failed") },
      retryFn,
    );

    clearTimerSyncConnectionError();

    expect(useErrorStore.getState().errors.connection).toBeUndefined();
  });
});
