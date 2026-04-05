import { beforeEach, describe, expect, it, vi } from "vitest";

type FrontendRedactionClassification = "public" | "sensitive" | "secret";

interface FrontendLogEventDefinitionForMock {
  allowedFields: readonly string[];
  redactionClassification: Readonly<
    Partial<Record<string, FrontendRedactionClassification>>
  >;
}

interface LogEventsModuleForMock {
  getFrontendLogEventDefinition(
    eventId: string,
  ): FrontendLogEventDefinitionForMock;
}

describe("buildSanitizedFrontendLogRecord()", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("rejects unknown fields in development", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    expect(() => {
      buildSanitizedFrontendLogRecord(
        "settings.sync.failed",
        "settings_sync",
        {
          timer_status: "idle",
          unknown_field: "boom",
        } as never,
        undefined,
      );
    }).toThrow(/Unknown frontend log field\(s\)/u);
  });

  it("drops unknown fields in production", async () => {
    const {
      setProductionEnvironmentOverrideForTest,
      buildSanitizedFrontendLogRecord,
    } = await import("./redaction");
    setProductionEnvironmentOverrideForTest(true);

    const record = buildSanitizedFrontendLogRecord(
      "settings.sync.failed",
      "settings_sync",
      {
        timer_status: "idle",
        unknown_field: "boom",
      } as never,
      undefined,
    );

    expect(record).toEqual({
      keyValues: {
        logger_scope: "settings_sync",
        timer_status: "idle",
      },
    });
  });

  it("redacts secret fields to exact marker", async () => {
    vi.doMock("./logEvents", async () => {
      const actualLogEvents: LogEventsModuleForMock =
        await vi.importActual("./logEvents");
      const originalDefinition = actualLogEvents.getFrontendLogEventDefinition(
        "settings.sync.failed",
      );
      const patchedDefinition = {
        ...originalDefinition,
        allowedFields: [...originalDefinition.allowedFields, "secret_preview"],
        redactionClassification: {
          ...originalDefinition.redactionClassification,
          secret_preview: "secret" as const,
        },
      } as typeof originalDefinition;

      return {
        ...actualLogEvents,
        getFrontendLogEventDefinition(
          eventId: string,
        ): FrontendLogEventDefinitionForMock {
          return eventId === "settings.sync.failed"
            ? patchedDefinition
            : actualLogEvents.getFrontendLogEventDefinition(eventId);
        },
      };
    });

    try {
      const { buildSanitizedFrontendLogRecord } = await import("./redaction");
      const record = buildSanitizedFrontendLogRecord(
        "settings.sync.failed",
        "settings_sync",
        {
          secret_preview: "super-secret-value",
        } as never,
        undefined,
      );

      expect(record.keyValues.secret_preview).toBe("[REDACTED]");
    } finally {
      vi.doUnmock("./logEvents");
    }
  });

  it("emits reserved logger and exception fields independently from caller fields", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "   " as never,
      {
        failure_message: "caller detail",
        max_connection_retries: 3,
        retry_attempt: 1,
      },
      new Error("boom"),
    );

    expect(record.keyValues).toMatchObject({
      failure_message: "caller detail",
      logger_scope: "unknown_scope",
      max_connection_retries: "3",
      retry_attempt: "1",
      "exception.type": "Error",
      "exception.message": "boom",
    });
  });

  it("masks api_key in sensitive and exception contexts", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: "request failed with api_key=field-secret",
      },
      new Error("request failed with api_key=exception-secret"),
    );

    expect(record.keyValues.failure_message).toBe(
      "request failed with [REDACTED]",
    );
    expect(record.keyValues["exception.message"]).toBe(
      "request failed with [REDACTED]",
    );
  });

  it("truncates sensitive and exception fields after token masking", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");
    const longSensitiveValue = `${"x".repeat(320)} api_key=field-secret`;
    const longStacktrace = `${"x".repeat(2300)} api_key=stack-secret`;

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: longSensitiveValue,
      },
      {
        message: longSensitiveValue,
        name: "RetryError",
        stack: longStacktrace,
      },
    );

    expect(record.keyValues.failure_message).toMatch(/\.\.\.\[TRUNCATED\]$/u);
    expect(record.keyValues.failure_message).not.toContain("field-secret");
    expect(record.keyValues["exception.message"]).toMatch(
      /\.\.\.\[TRUNCATED\]$/u,
    );
    expect(record.keyValues["exception.message"]).not.toContain("field-secret");
    expect(record.keyValues["exception.stacktrace"]).toMatch(
      /\.\.\.\[TRUNCATED\]$/u,
    );
    expect(record.keyValues["exception.stacktrace"]).not.toContain(
      "stack-secret",
    );
  });

  it("treats component_stack as a stack-like sensitive field", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");
    const longComponentStack = `stack api_key=stack-secret ${"x".repeat(2300)}`;
    const longFailureMessage = `message api_key=field-secret ${"x".repeat(400)}`;

    const record = buildSanitizedFrontendLogRecord(
      "error_boundary.boundary_render_error_caught",
      "error_boundary",
      {
        boundary_scope: "settings",
        component_stack: longComponentStack,
        current_screen: "settingsTimer",
        failure_message: longFailureMessage,
      },
      undefined,
    );

    expect(record.keyValues.component_stack).toContain("[REDACTED]");
    expect(record.keyValues.component_stack).not.toContain("stack-secret");
    expect(record.keyValues.component_stack).toMatch(/\.\.\.\[TRUNCATED\]$/u);
    expect(record.keyValues.component_stack).toBeDefined();
    expect(record.keyValues.failure_message).not.toContain("field-secret");
    expect(record.keyValues.failure_message).toMatch(/\.\.\.\[TRUNCATED\]$/u);
    expect(record.keyValues.failure_message).toBeDefined();

    const componentStack = record.keyValues.component_stack;
    const failureMessage = record.keyValues.failure_message;

    if (componentStack === undefined || failureMessage === undefined) {
      throw new Error("Expected redacted error-boundary fields to be defined");
    }

    expect(componentStack.length).toBeGreaterThan(256);
    expect(failureMessage.length).toBeLessThan(componentStack.length);
  });
});
