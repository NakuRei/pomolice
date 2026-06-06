import { describe, expect, it } from "vitest";
import {
  classifyBackendError,
  formatBackendErrorDetail,
  formatPublicCommandErrorDetail,
  type BackendErrorClassification,
  type BackendErrorCode,
} from "./classifyBackendError";

describe("classifyBackendError", () => {
  it("classifies known structured backend error responses by code", () => {
    const classifications: {
      code: BackendErrorCode;
      expected: Omit<BackendErrorClassification, "userMessage">;
    }[] = [
      {
        code: "invalid_state_transition",
        expected: {
          isTransient: false,
          severity: "recoverable",
          canRetry: false,
        },
      },
      {
        code: "rate_limited",
        expected: {
          isTransient: true,
          severity: "recoverable",
          canRetry: true,
        },
      },
      {
        code: "window_control_error",
        expected: {
          isTransient: false,
          severity: "recoverable",
          canRetry: true,
        },
      },
      {
        code: "persistence_error",
        expected: {
          isTransient: false,
          severity: "recoverable",
          canRetry: true,
        },
      },
      {
        code: "unauthorized_window",
        expected: {
          isTransient: false,
          severity: "recoverable",
          canRetry: false,
        },
      },
      {
        code: "invalid_command_input",
        expected: {
          isTransient: false,
          severity: "recoverable",
          canRetry: false,
        },
      },
    ];
    const fallbackClassification = classifyBackendError({
      code: "unknown_error",
      message: "fallback diagnostic",
    });
    const displayIdentities = new Set<string>();

    for (const { code, expected } of classifications) {
      const diagnosticMessage = `diagnostic from C:\\secret for ${code}`;
      const classification = classifyBackendError({
        code,
        message: diagnosticMessage,
      });
      const sameCodeWithDifferentMessage = classifyBackendError({
        code,
        message: `other diagnostic from C:\\other-secret for ${code}`,
      });

      expect(classification).toMatchObject(expected);
      expect(sameCodeWithDifferentMessage).toEqual(classification);
      expect(classification.userMessage).toEqual(expect.any(String));
      expect(classification.userMessage).not.toBe("");
      expect(classification.userMessage).not.toBe(
        fallbackClassification.userMessage,
      );
      expect(classification.userMessage).not.toBe(diagnosticMessage);
      expect(classification.userMessage).not.toContain("C:\\secret");
      displayIdentities.add(classification.userMessage);
    }

    expect(displayIdentities.size).toBe(classifications.length);
  });

  it("does not use public envelope messages for presentation fields", () => {
    const first = classifyBackendError({
      code: "window_control_error",
      message: "failed for C:\\Users\\nak\\secret\\timer_state.json",
    });
    const second = classifyBackendError({
      code: "window_control_error",
      message: "panic at C:\\Users\\nak\\other-secret",
    });

    expect(first).toEqual(second);
    expect(first.userMessage).not.toContain("C:\\Users\\nak");
    expect(first.userMessage).not.toContain("timer_state.json");
  });

  it("uses the default classification for malformed values, unknown codes, and legacy strings", () => {
    const inputs: unknown[] = [
      {
        code: "unknown_error",
        message: "first diagnostic",
      },
      {
        code: "unknown_error",
        message: "should not be trusted",
      },
      { code: "rate_limited", message: 300 },
      "Rate limited: please wait 300ms",
      null,
      undefined,
      500,
    ];

    for (const input of inputs) {
      const classification = classifyBackendError(input);

      expect(classification).toMatchObject({
        isTransient: false,
        severity: "recoverable",
        canRetry: false,
      });
      expect(classification.userMessage).toEqual(expect.any(String));
      expect(classification.userMessage).not.toBe("");
      expect(classification.userMessage).not.toContain("first diagnostic");
      expect(classification.userMessage).not.toContain("should not be trusted");
      expect(classification.userMessage).not.toContain("please wait 300ms");
    }
  });
});

describe("formatPublicCommandErrorDetail", () => {
  function expectSafePublicCommandFallbackDetail(error: unknown): void {
    const detail = formatPublicCommandErrorDetail(error);

    expect(detail).toEqual(expect.any(String));
    expect(detail).not.toBe("");
    expect(detail).not.toContain("C:\\secret");
    expect(detail).not.toContain("panic");
    expect(detail).not.toContain("internal path");
  }

  it("stores diagnostics from recognized public codes without raw message text", () => {
    const cases: {
      error: Parameters<typeof formatPublicCommandErrorDetail>[0];
      expectedDetail: string;
      excludedFragments: string[];
    }[] = [
      {
        error: {
          code: "window_control_error",
          message: "failed for C:\\Users\\nak\\secret\\timer_state.json",
        },
        expectedDetail: "window_control_error",
        excludedFragments: ["failed for", "C:\\Users\\nak", "timer_state.json"],
      },
      {
        error: {
          code: "invalid_command_input",
          message: "invalid type: string C:\\secret",
        },
        expectedDetail: "invalid_command_input",
        excludedFragments: ["invalid type", "string", "C:\\secret"],
      },
    ];

    for (const { error, expectedDetail, excludedFragments } of cases) {
      const detail = formatPublicCommandErrorDetail(error);

      expect(detail).toBe(expectedDetail);
      for (const fragment of excludedFragments) {
        expect(detail).not.toContain(fragment);
      }
    }
  });

  it("uses a safe fallback for malformed command failures", () => {
    expectSafePublicCommandFallbackDetail(new Error("C:\\secret"));
    expectSafePublicCommandFallbackDetail("panic at C:\\secret");
    expectSafePublicCommandFallbackDetail({
      code: "unknown_error",
      message: "internal path C:\\secret",
    });
  });
});

describe("formatBackendErrorDetail", () => {
  it("formats known structured backend responses with code and message", () => {
    expect(
      formatBackendErrorDetail({
        code: "rate_limited",
        message: "please wait 300ms",
      }),
    ).toBe("rate_limited: please wait 300ms");
  });

  it("formats Error objects and string rejections", () => {
    expect(formatBackendErrorDetail(new Error("network unavailable"))).toBe(
      "Error: network unavailable",
    );
    expect(formatBackendErrorDetail("legacy backend error")).toBe(
      "legacy backend error",
    );
  });

  it("uses a generic detail for unknown values and untrusted object messages", () => {
    expect(
      formatBackendErrorDetail({
        code: "unknown_error",
        message: "should not be trusted",
      }),
    ).toBe("Unexpected backend error");
    expect(formatBackendErrorDetail({ message: "should not be trusted" })).toBe(
      "Unexpected backend error",
    );
    expect(formatBackendErrorDetail(null)).toBe("Unexpected backend error");
    expect(formatBackendErrorDetail(undefined)).toBe(
      "Unexpected backend error",
    );
    expect(formatBackendErrorDetail(500)).toBe("Unexpected backend error");
    expect(formatBackendErrorDetail(false)).toBe("Unexpected backend error");
    expect(formatBackendErrorDetail(Symbol("backend"))).toBe(
      "Unexpected backend error",
    );
  });
});
