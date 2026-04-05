import { beforeEach, describe, expect, it, vi } from "vitest";
import { FRONTEND_LOG_EVENT_IDS } from "./logEvents";

const { createLoggerMock, errorLoggerMock, warnLoggerMock } = vi.hoisted(() => {
  const debugLogSpy = vi.fn();
  const errorLogSpy = vi.fn();
  const infoLogSpy = vi.fn();
  const traceLogSpy = vi.fn();
  const warnLogSpy = vi.fn();
  const loggerFactorySpy = vi.fn(() => ({
    debug: debugLogSpy,
    error: errorLogSpy,
    info: infoLogSpy,
    trace: traceLogSpy,
    warn: warnLogSpy,
  }));

  return {
    createLoggerMock: loggerFactorySpy,
    errorLoggerMock: errorLogSpy,
    warnLoggerMock: warnLogSpy,
  };
});

vi.mock("./logger", () => ({
  createLogger: createLoggerMock,
}));

import {
  createRootErrorCallbacks,
  resolveCreateRootOptions,
} from "./createRootErrorCallbacks";

describe("createRootErrorCallbacks()", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("logs caught render errors with the root callback event id", () => {
    const callbacks = createRootErrorCallbacks();
    const error = new Error("caught crash");

    callbacks.onCaughtError?.(error, {
      componentStack: "\n    at RootCrash",
      errorBoundary: undefined,
    });

    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRootRenderErrorCaught,
      error,
      {
        component_stack: "\n    at RootCrash",
        failure_message: "caught crash",
      },
    );
  });

  it("logs uncaught render errors with the uncaught event id", () => {
    const callbacks = createRootErrorCallbacks();
    const error = new Error("uncaught crash");

    callbacks.onUncaughtError?.(error, {
      componentStack: "\n    at MissingBoundary",
    });

    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryUncaughtRenderError,
      error,
      {
        component_stack: "\n    at MissingBoundary",
        failure_message: "uncaught crash",
      },
    );
  });

  it("logs recoverable render errors with component_stack when present", () => {
    const callbacks = createRootErrorCallbacks();

    callbacks.onRecoverableError?.("recoverable crash", {
      componentStack: "\n    at RecoverableRoot",
    });

    expect(warnLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRecoverableRenderError,
      {
        component_stack: "\n    at RecoverableRoot",
        failure_message: "recoverable crash",
      },
    );
  });

  it.each([
    {
      description: "string errors",
      expectedFailureMessage: "plain string failure",
      value: "plain string failure",
    },
    {
      description: "plain object errors",
      expectedFailureMessage: JSON.stringify({
        reason: "boom",
        retryable: true,
      }),
      value: { reason: "boom", retryable: true },
    },
    {
      description: "null errors",
      expectedFailureMessage: "null",
      value: null,
    },
  ])(
    "normalizes $description into failure_message",
    ({ expectedFailureMessage, value }) => {
      const callbacks = createRootErrorCallbacks();

      callbacks.onCaughtError?.(value, {
        componentStack: undefined,
        errorBoundary: undefined,
      });

      expect(errorLoggerMock).toHaveBeenCalledWith(
        FRONTEND_LOG_EVENT_IDS.errorBoundaryRootRenderErrorCaught,
        value,
        {
          failure_message: expectedFailureMessage,
        },
      );
    },
  );

  it("returns callbacks only for production createRoot options", () => {
    const productionOptions = resolveCreateRootOptions(true);

    expect(productionOptions).toBeDefined();
    expect(productionOptions?.onCaughtError).toEqual(expect.any(Function));
    expect(productionOptions?.onRecoverableError).toEqual(expect.any(Function));
    expect(productionOptions?.onUncaughtError).toEqual(expect.any(Function));
    expect(resolveCreateRootOptions(false)).toBeUndefined();
  });
});
