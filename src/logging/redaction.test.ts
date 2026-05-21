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

  it("redacts Windows profile paths in sensitive and exception messages", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");
    const windowsProfilePath =
      "C:\\Users\\Alice\\AppData\\Local\\pomolice\\sound.wav";

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: `sound playback failed at ${windowsProfilePath}`,
      },
      new Error(`sound playback failed at ${windowsProfilePath}`),
    );

    expect(record.keyValues.failure_message).toBe(
      "sound playback failed at [REDACTED]",
    );
    expect(record.keyValues.failure_message).not.toContain("Alice");
    expect(record.keyValues.failure_message).not.toContain("C:\\Users\\Alice");
    expect(record.keyValues["exception.message"]).toBe(
      "sound playback failed at [REDACTED]",
    );
    expect(record.keyValues["exception.message"]).not.toContain("Alice");
    expect(record.keyValues["exception.message"]).not.toContain(
      "C:\\Users\\Alice",
    );
  });

  it.each([
    {
      label: "forward slashes",
      profilePath: "C:/Users/Alice/AppData/Local/pomolice/sound.wav",
      profileRoot: "C:/Users/Alice",
    },
    {
      label: "lowercase Users segment",
      profilePath: "C:\\users\\Alice\\AppData\\Local\\pomolice\\sound.wav",
      profileRoot: "C:\\users\\Alice",
    },
  ])(
    "redacts Windows profile paths with $label in sensitive and exception messages",
    async ({
      profilePath,
      profileRoot,
    }: {
      profilePath: string;
      profileRoot: string;
    }) => {
      const { buildSanitizedFrontendLogRecord } = await import("./redaction");

      const record = buildSanitizedFrontendLogRecord(
        "timer.sync.setup_failed",
        "timer_sync",
        {
          failure_message: `sound playback failed at ${profilePath}`,
        },
        new Error(`sound playback failed at ${profilePath}`),
      );

      expect(record.keyValues.failure_message).toContain("[REDACTED]");
      expect(record.keyValues.failure_message).not.toContain("Alice");
      expect(record.keyValues.failure_message).not.toContain(profileRoot);
      expect(record.keyValues["exception.message"]).toContain("[REDACTED]");
      expect(record.keyValues["exception.message"]).not.toContain("Alice");
      expect(record.keyValues["exception.message"]).not.toContain(profileRoot);
    },
  );

  it("preserves context after a redacted Windows profile root", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: "failed at C:\\Users\\Alice, retry later",
      },
      new Error('path="C:\\Users\\Alice" retry'),
    );

    expect(record.keyValues.failure_message).toBe(
      "failed at [REDACTED], retry later",
    );
    expect(record.keyValues.failure_message).not.toContain("Alice");
    expect(record.keyValues["exception.message"]).toBe(
      'path="[REDACTED]" retry',
    );
    expect(record.keyValues["exception.message"]).not.toContain("Alice");
  });

  it("preserves context after a redacted Unix profile root", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: "failed at /Users/alice, retry later",
      },
      new Error('path="/home/alice" retry'),
    );

    expect(record.keyValues.failure_message).toBe(
      "failed at [REDACTED], retry later",
    );
    expect(record.keyValues.failure_message).not.toContain("alice");
    expect(record.keyValues["exception.message"]).toBe(
      'path="[REDACTED]" retry',
    );
    expect(record.keyValues["exception.message"]).not.toContain("alice");
  });

  it("redacts colon-prefixed profile paths in sensitive and exception messages", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message:
          "path:C:\\Users\\Alice\\AppData\\Local\\pomolice\\sound.wav",
      },
      new Error("open:/Users/alice/Library/Caches/pomolice/sound.wav"),
    );

    expect(record.keyValues.failure_message).toBe("path:[REDACTED]");
    expect(record.keyValues.failure_message).not.toContain("Alice");
    expect(record.keyValues.failure_message).not.toContain("C:\\Users\\Alice");
    expect(record.keyValues["exception.message"]).toBe("open:[REDACTED]");
    expect(record.keyValues["exception.message"]).not.toContain("alice");
    expect(record.keyValues["exception.message"]).not.toContain("/Users/alice");
  });

  it.each([
    {
      label: "macOS",
      username: "alice",
      profilePath: "/Users/alice/Library/Caches/pomolice/sound.wav",
      profileRoot: "/Users/alice",
    },
    {
      label: "Linux",
      username: "alice",
      profilePath: "/home/alice/.local/share/pomolice/sound.wav",
      profileRoot: "/home/alice",
    },
  ])(
    "redacts $label home paths in sensitive and exception messages",
    async ({
      profilePath,
      profileRoot,
      username,
    }: {
      profilePath: string;
      profileRoot: string;
      username: string;
    }) => {
      const { buildSanitizedFrontendLogRecord } = await import("./redaction");

      const record = buildSanitizedFrontendLogRecord(
        "timer.sync.setup_failed",
        "timer_sync",
        {
          failure_message: `sound playback failed at ${profilePath}`,
        },
        new Error(`sound playback failed at ${profilePath}`),
      );

      expect(record.keyValues.failure_message).toBe(
        "sound playback failed at [REDACTED]",
      );
      expect(record.keyValues.failure_message).not.toContain(username);
      expect(record.keyValues.failure_message).not.toContain(profileRoot);
      expect(record.keyValues["exception.message"]).toBe(
        "sound playback failed at [REDACTED]",
      );
      expect(record.keyValues["exception.message"]).not.toContain(username);
      expect(record.keyValues["exception.message"]).not.toContain(profileRoot);
    },
  );

  it.each([
    {
      label: "Windows",
      profilePath: "C:\\Users\\Jane Doe\\AppData\\Local\\pomolice\\sound.wav",
      forbiddenFragments: ["Jane", "Doe", "Jane Doe", "C:\\Users\\Jane Doe"],
    },
    {
      label: "macOS",
      profilePath: "/Users/Jane Doe/Library/Caches/pomolice/sound.wav",
      forbiddenFragments: ["Jane", "Doe", "Jane Doe", "/Users/Jane Doe"],
    },
    {
      label: "Linux",
      profilePath: "/home/Jane Doe/.local/share/pomolice/sound.wav",
      forbiddenFragments: ["Jane", "Doe", "Jane Doe", "/home/Jane Doe"],
    },
  ])(
    "redacts $label home paths with spaces in sensitive and exception messages",
    async ({
      forbiddenFragments,
      profilePath,
    }: {
      forbiddenFragments: readonly string[];
      profilePath: string;
    }) => {
      const { buildSanitizedFrontendLogRecord } = await import("./redaction");

      const record = buildSanitizedFrontendLogRecord(
        "timer.sync.setup_failed",
        "timer_sync",
        {
          failure_message: `sound playback failed at ${profilePath}`,
        },
        new Error(`sound playback failed at ${profilePath}`),
      );

      const redactedMessages = [
        record.keyValues.failure_message,
        record.keyValues["exception.message"],
      ];

      for (const redactedMessage of redactedMessages) {
        expect(redactedMessage).toContain("[REDACTED]");
        for (const forbiddenFragment of forbiddenFragments) {
          expect(redactedMessage).not.toContain(forbiddenFragment);
        }
      }
    },
  );

  it("redacts profile paths inside high-risk assignment values before token masking", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");
    const windowsProfilePath =
      "C:\\Users\\Jane Doe\\AppData\\Local\\pomolice\\sound.wav";

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      {
        failure_message: `api_key=${windowsProfilePath}`,
      },
      new Error(`api_key=${windowsProfilePath}`),
    );

    expect(record.keyValues.failure_message).toBe("[REDACTED]");
    expect(record.keyValues.failure_message).not.toContain("Jane");
    expect(record.keyValues.failure_message).not.toContain("Doe");
    expect(record.keyValues.failure_message).not.toContain("Jane Doe");
    expect(record.keyValues.failure_message).not.toContain(
      "C:\\Users\\Jane Doe",
    );
    expect(record.keyValues["exception.message"]).toBe("[REDACTED]");
    expect(record.keyValues["exception.message"]).not.toContain("Jane");
    expect(record.keyValues["exception.message"]).not.toContain("Doe");
    expect(record.keyValues["exception.message"]).not.toContain("Jane Doe");
    expect(record.keyValues["exception.message"]).not.toContain(
      "C:\\Users\\Jane Doe",
    );
  });

  it("redacts JSON-stringified Windows profile paths from object errors", async () => {
    const { buildSanitizedFrontendLogRecord } = await import("./redaction");

    const record = buildSanitizedFrontendLogRecord(
      "timer.sync.setup_failed",
      "timer_sync",
      undefined,
      {
        path: "C:\\Users\\Alice\\AppData\\Local\\pomolice\\sound.wav",
        retryable: true,
      },
    );

    expect(record.keyValues["exception.message"]).toBe(
      JSON.stringify({
        path: "[REDACTED]",
        retryable: true,
      }),
    );
    expect(record.keyValues["exception.message"]).not.toContain("Alice");
    expect(record.keyValues["exception.message"]).not.toContain(
      "C:\\\\Users\\\\Alice",
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
