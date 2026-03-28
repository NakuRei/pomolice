import { beforeEach, describe, expect, it, vi } from "vitest";

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

function getFirstWriteLogCall(
  writeLogMock: ReturnType<typeof vi.fn>,
): [unknown, unknown, Record<string, unknown>] {
  const firstCall = writeLogMock.mock.calls[0];
  expect(firstCall).toBeDefined();
  if (firstCall === undefined) {
    throw new Error("Expected the logger to write at least one record");
  }

  const keyValues: unknown = firstCall[2];
  expectRecord(keyValues, "writeLog keyValues");
  return [firstCall[0], firstCall[1], keyValues];
}

describe("createLogger()", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
    vi.doUnmock("./logTransport");
  });

  it("writes normalized error records through writeLog", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();
    createLogger("timer_sync").error("setup_failed", new Error("boom"), {
      retry_attempt: 1,
    });

    expect(writeLogMock).toHaveBeenCalledTimes(1);

    const [level, message, keyValues] = getFirstWriteLogCall(writeLogMock);
    expect(level).toBe("error");
    expect(message).toBe("timer_sync.setup_failed");
    expect(keyValues.retry_attempt).toBe("1");
    expect(keyValues["exception.type"]).toBe("Error");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("keeps caller error detail fields separate from canonical exception fields", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();
    createLogger("timer_sync").error("setup_failed", new Error("boom"), {
      error_message: "caller detail",
    });

    expect(writeLogMock).toHaveBeenCalledTimes(1);

    const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
    expect(keyValues.error_message).toBe("caller detail");
    expect(keyValues["exception.message"]).toBe("boom");
  });

  it("redacts sensitive fields before writing them", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();
    createLogger("settings_sync").info("config_applied", {
      authorization_header: "Bearer secret",
      cookie_value: "session=abc",
      password: "super-secret",
      retry_attempt: 2,
      token: "abc",
    });

    expect(writeLogMock).toHaveBeenCalledTimes(1);

    const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
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
      const writeLogMock = vi.fn();
      vi.doMock("./logTransport", () => ({
        writeLog: writeLogMock,
      }));

      const { createLogger, initializeFrontendLogging } =
        await import("./logger");

      await initializeFrontendLogging();
      createLogger("timer_sync").error("setup_failed", errorValue);

      const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
      expect(keyValues["exception.message"]).toBe(expected);
    },
  );

  it("stringifies function errors with the function name", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    function namedFailure(): string {
      return "ignored";
    }

    await initializeFrontendLogging();
    createLogger("timer_sync").error("setup_failed", namedFailure);

    const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
    expect(keyValues["exception.message"]).toBe("[function namedFailure]");
  });

  it("extracts error-like fields from plain objects", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();
    createLogger("timer_sync").error("setup_failed", {
      name: 404,
      message: 123n,
      stack: Symbol("trace"),
    });

    const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
    expect(keyValues["exception.type"]).toBe("404");
    expect(keyValues["exception.message"]).toBe("123");
    expect(keyValues["exception.stacktrace"]).toBe("Symbol(trace)");
  });

  it("falls back to stringifying plain objects without error-like fields", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();
    createLogger("timer_sync").error("setup_failed", {
      reason: "boom",
      retryable: true,
    });

    const [, , keyValues] = getFirstWriteLogCall(writeLogMock);
    expect(keyValues["exception.message"]).toBe(
      JSON.stringify({
        reason: "boom",
        retryable: true,
      }),
    );
  });

  it("swallows writeLog failures", async () => {
    const writeLogMock = vi.fn(() => {
      throw new Error("transport down");
    });
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await initializeFrontendLogging();

    expect(() => {
      createLogger("timer_sync").error("setup_failed", new Error("boom"));
    }).not.toThrow();

    expect(writeLogMock).toHaveBeenCalledTimes(1);
  });

  it("does not write before initialization", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger } = await import("./logger");

    createLogger("bootstrap").info("completed");

    expect(writeLogMock).not.toHaveBeenCalled();
  });

  it("writes through the transport after initialization", async () => {
    const writeLogMock = vi.fn();
    vi.doMock("./logTransport", () => ({
      writeLog: writeLogMock,
    }));

    const { createLogger, initializeFrontendLogging } =
      await import("./logger");

    await expect(initializeFrontendLogging()).resolves.toBeUndefined();
    expect(() => {
      createLogger("bootstrap").info("completed");
    }).not.toThrow();
    expect(writeLogMock).toHaveBeenCalledWith(
      "info",
      "bootstrap.completed",
      {},
    );
  });
});
