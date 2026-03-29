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

  it("writes scoped info records via @tauri-apps/plugin-log without initialization", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").info("completed", {
      retry_attempt: 1,
    });

    expect(infoLog).toHaveBeenCalledTimes(1);
    expect(infoLog).toHaveBeenCalledWith("timer_sync.completed", {
      keyValues: {
        retry_attempt: "1",
      },
    });
  });

  it("writes error records via @tauri-apps/plugin-log with sanitized keyValues", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error("setup_failed", new Error("boom"), {
      retry_attempt: 1,
    });

    expect(errorLog).toHaveBeenCalledTimes(1);

    const [message, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(message).toBe("timer_sync.setup_failed");
    expect(keyValues.retry_attempt).toBe("1");
    expect(keyValues["exception.type"]).toBe("Error");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("keeps caller error detail fields separate from canonical exception fields", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error("setup_failed", new Error("boom"), {
      error_message: "caller detail",
    });

    expect(errorLog).toHaveBeenCalledTimes(1);

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues.error_message).toBe("caller detail");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("redacts sensitive field values before calling plugin-log", async () => {
    const { createLogger } = await import("./logger");

    createLogger("settings_sync").info("config_applied", {
      authorization_header: "Bearer secret",
      cookie_value: "session=abc",
      password: "super-secret",
      retry_attempt: 2,
      token: "abc",
    });

    expect(infoLog).toHaveBeenCalledTimes(1);

    const [, keyValues] = getFirstPluginWriteCall(infoLog);
    expect(keyValues.authorization_header).toBe("[REDACTED]");
    expect(keyValues.cookie_value).toBe("[REDACTED]");
    expect(keyValues.password).toBe("[REDACTED]");
    expect(keyValues.retry_attempt).toBe("2");
    expect(keyValues.token).toBe("[REDACTED]");
  });

  it.each([
    { label: "bigint", errorValue: 12n, expected: "12" },
    { label: "symbol", errorValue: Symbol("boom"), expected: "Symbol(boom)" },
    { label: "null", errorValue: null, expected: "null" },
  ])(
    "stringifies %s error values",
    async ({
      errorValue,
      expected,
    }: {
      errorValue: bigint | symbol | null;
      expected: string;
    }) => {
      const { createLogger } = await import("./logger");

      createLogger("timer_sync").error("setup_failed", errorValue);

      const [, keyValues] = getFirstPluginWriteCall(errorLog);
      expect(keyValues["exception.message"]).toBe(expected);
    },
  );

  it("stringifies function errors with the function name", async () => {
    const { createLogger } = await import("./logger");

    function namedFailure(): string {
      return "ignored";
    }

    createLogger("timer_sync").error("setup_failed", namedFailure);

    const [, keyValues] = getFirstPluginWriteCall(errorLog);
    expect(keyValues["exception.message"]).toBe("[function namedFailure]");
  });

  it("extracts error-like fields from plain objects", async () => {
    const { createLogger } = await import("./logger");

    createLogger("timer_sync").error("setup_failed", {
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

    createLogger("timer_sync").error("setup_failed", {
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

    createLogger("notifications").warn("preview_failed\nforged_entry", {
      failure_message: "line1\r\nline2",
    });

    expect(warnLog).toHaveBeenCalledWith(
      "notifications.preview_failed\\nforged_entry",
      {
        keyValues: {
          failure_message: "line1\\r\\nline2",
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
      createLogger("timer_sync").error("setup_failed", new Error("boom"));
    }).not.toThrow();

    expect(errorLog).toHaveBeenCalledTimes(1);
  });

  it("does not write logs outside the tauri environment", async () => {
    const originalTauriInternals = getTauriInternals();
    Reflect.deleteProperty(globalThis, "__TAURI_INTERNALS__");

    try {
      const { createLogger } = await import("./logger");

      createLogger("bootstrap").info("completed");

      expect(traceLog).not.toHaveBeenCalled();
      expect(debugLog).not.toHaveBeenCalled();
      expect(infoLog).not.toHaveBeenCalled();
      expect(warnLog).not.toHaveBeenCalled();
      expect(errorLog).not.toHaveBeenCalled();
    } finally {
      setTauriInternals(originalTauriInternals);
    }
  });

  it("normalizes blank scope and event identifiers", async () => {
    const { createLogger } = await import("./logger");

    createLogger("   ").info("   ");

    expect(infoLog).toHaveBeenCalledWith("unknown_scope.unknown_event", {
      keyValues: {},
    });
  });
});
