import { beforeEach, describe, expect, it, vi } from "vitest";

const traceLog = vi.fn().mockResolvedValue(undefined);
const debugLog = vi.fn().mockResolvedValue(undefined);
const infoLog = vi.fn().mockResolvedValue(undefined);
const warnLog = vi.fn().mockResolvedValue(undefined);
const errorLog = vi.fn().mockResolvedValue(undefined);

vi.mock("@tauri-apps/plugin-log", () => ({
  trace: traceLog,
  debug: debugLog,
  info: infoLog,
  warn: warnLog,
  error: errorLog,
}));

function expectRecord(
  value: unknown,
  description: string,
): asserts value is Record<string, unknown> {
  expect(value).not.toBeNull();
  expect(typeof value).toBe("object");
  if (value === null || typeof value !== "object") {
    throw new Error(`Expected ${description} to be an object`);
  }
}

function getFirstPluginWriteCall(
  pluginLogMock: ReturnType<typeof vi.fn>,
): [unknown, Record<string, unknown>] {
  const firstCall = pluginLogMock.mock.calls[0];
  expect(firstCall).toBeDefined();
  if (firstCall === undefined) {
    throw new Error("Expected the logger to write at least one record");
  }

  const options: unknown = firstCall[1];
  expectRecord(options, "plugin log options");
  const keyValues: unknown = options.keyValues;
  expectRecord(keyValues, "plugin log keyValues");
  return [firstCall[0], keyValues];
}

function getTauriInternals(): unknown {
  return Reflect.get(globalThis, "__TAURI_INTERNALS__");
}

function setTauriInternals(value: unknown): void {
  Reflect.set(globalThis, "__TAURI_INTERNALS__", value);
}

describe("createLogger()", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it("writes registered info events via @tauri-apps/plugin-log", async () => {
    const { createLogger } = await import("./logger");

    createLogger("bootstrap").info("app.startup.completed");

    expect(infoLog).toHaveBeenCalledTimes(1);
    expect(infoLog).toHaveBeenCalledWith("app.startup.completed", {
      keyValues: {
        logger_scope: "bootstrap",
      },
    });
  });

  it("writes error events with normalized exception fields", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error(
      "timer.sync.setup_failed",
      new Error("boom"),
      {
        max_connection_retries: 3,
        retry_attempt: 1,
      },
    );

    expect(errorLog).toHaveBeenCalledTimes(1);

    const [message, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(message).toBe("timer.sync.setup_failed");
    expect(keyValues.logger_scope).toBe("timer_sync");
    expect(keyValues.max_connection_retries).toBe("3");
    expect(keyValues.retry_attempt).toBe("1");
    expect(keyValues["exception.type"]).toBe("Error");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("keeps caller detail fields separate from canonical exception fields", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error(
      "timer.sync.setup_failed",
      new Error("boom"),
      {
        failure_message: "caller detail",
      } as never,
    );

    expect(errorLog).toHaveBeenCalledTimes(1);

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues.failure_message).toBe("caller detail");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("throws when runtime fields include keys outside the event catalog in development", async () => {
    const { createLogger } = await import("./logger");

    expect(() => {
      createLogger("settings_sync").warn("settings.sync.failed", {
        timer_status: "idle",
        unknown_field: "boom",
      } as never);
    }).toThrow(/Unknown frontend log field\(s\)/u);

    expect(warnLog).not.toHaveBeenCalled();
  });

  it("does not pass unknown fields to plugin-log in production", async () => {
    const { setProductionEnvironmentOverrideForTest } =
      await import("./redaction");
    setProductionEnvironmentOverrideForTest(true);
    const { createLogger } = await import("./logger");

    createLogger("settings_sync").warn("settings.sync.failed", {
      timer_status: "idle",
      unknown_field: "boom",
    } as never);

    expect(warnLog).toHaveBeenCalledTimes(1);

    const [, keyValues] = getFirstPluginWriteCall(warnLog);
    expect(keyValues.logger_scope).toBe("settings_sync");
    expect(keyValues.timer_status).toBe("idle");
    expect(keyValues.unknown_field).toBeUndefined();
  });

  it("masks api_key in sensitive and exception fields before calling plugin-log", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error(
      "timer.sync.setup_failed",
      new Error("request failed with api_key=exception-secret"),
      {
        failure_message: "request failed with api_key=field-secret",
      },
    );

    expect(errorLog).toHaveBeenCalledTimes(1);

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues.failure_message).toBe("request failed with [REDACTED]");
    expect(keyValues["exception.message"]).toBe(
      "request failed with [REDACTED]",
    );
  });

  it.each([
    { label: "bigint", errorValue: 12n, expected: "12" },
    { label: "symbol", errorValue: Symbol("boom"), expected: "Symbol(boom)" },
    { label: "null", errorValue: null, expected: "null" },
  ])(
    "stringifies $label error values",
    async ({
      errorValue,
      expected,
    }: {
      errorValue: bigint | symbol | null;
      expected: string;
    }) => {
      const { createLogger } = await import("./logger");

      createLogger("timer_sync").error("timer.sync.setup_failed", errorValue);

      const [, keyValues] = getFirstPluginWriteCall(errorLog);
      expect(keyValues["exception.message"]).toBe(expected);
    },
  );

  it("stringifies function errors with the function name", async () => {
    const { createLogger } = await import("./logger");

    function namedFailure(): string {
      return "ignored";
    }

    createLogger("timer_sync").error("timer.sync.setup_failed", namedFailure);

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues["exception.message"]).toBe("[function namedFailure]");
  });

  it("extracts error-like fields from plain objects", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error("timer.sync.setup_failed", {
      name: 404,
      message: 123n,
      stack: Symbol("trace"),
    });

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues["exception.type"]).toBe("404");
    expect(keyValues["exception.message"]).toBe("123");
    expect(keyValues["exception.stacktrace"]).toBe("Symbol(trace)");
  });

  it("falls back to stringifying plain objects without error-like fields", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error("timer.sync.setup_failed", {
      reason: "boom",
      retryable: true,
    });

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues["exception.message"]).toBe(
      JSON.stringify({
        reason: "boom",
        retryable: true,
      }),
    );
  });

  it("escapes CRLF in messages and keyValues before calling plugin-log", async () => {
    const { createLogger } = await import("./logger");

    createLogger("notifications").warn(
      "notification.sound_preview_playback_failed",
      {
        failure_message: "line1\r\nline2",
      },
    );

    expect(warnLog).toHaveBeenCalledWith(
      "notification.sound_preview_playback_failed",
      {
        keyValues: {
          failure_message: "line1\\r\\nline2",
          logger_scope: "notifications",
        },
      },
    );
  });

  it("swallows plugin write failures", async () => {
    errorLog.mockImplementationOnce(() => {
      throw new Error("plugin down");
    });
    const { createLogger } = await import("./logger");

    expect(() => {
      createLogger("timer_sync").error(
        "timer.sync.setup_failed",
        new Error("boom"),
      );
    }).not.toThrow();

    expect(errorLog).toHaveBeenCalledTimes(1);
  });

  it("rejects event severity mismatches before transport", async () => {
    const { createLogger } = await import("./logger");

    expect(() => {
      createLogger("bootstrap").warn("app.startup.completed" as never);
    }).toThrow(/must be logged at info/u);

    expect(warnLog).not.toHaveBeenCalled();
  });

  it("rejects unregistered event ids", async () => {
    const { createLogger } = await import("./logger");

    expect(() => {
      createLogger("bootstrap").info("missing.event" as never);
    }).toThrow(/Unknown frontend log event/u);
  });

  it("does not write logs outside the tauri environment", async () => {
    const originalTauriInternals = getTauriInternals();
    Reflect.deleteProperty(globalThis, "__TAURI_INTERNALS__");

    try {
      const { createLogger } = await import("./logger");

      createLogger("bootstrap").info("app.startup.completed");

      expect(traceLog).not.toHaveBeenCalled();
      expect(debugLog).not.toHaveBeenCalled();
      expect(infoLog).not.toHaveBeenCalled();
      expect(warnLog).not.toHaveBeenCalled();
      expect(errorLog).not.toHaveBeenCalled();
    } finally {
      setTauriInternals(originalTauriInternals);
    }
  });

  it("normalizes blank scopes to unknown_scope", async () => {
    const { createLogger } = await import("./logger");

    createLogger("   " as never).info("app.startup.completed");

    expect(infoLog).toHaveBeenCalledWith("app.startup.completed", {
      keyValues: {
        logger_scope: "unknown_scope",
      },
    });
  });
});
