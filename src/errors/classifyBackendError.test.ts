import type { BackendErrorResponse } from "@/ipc/generated";
import { describe, expect, it } from "vitest";
import {
  classifyCommandFailure,
  formatCommandFailureDetail,
  formatBackendErrorDetail,
} from "./classifyBackendError";

describe("command failure presentation", () => {
  const errors: { error: BackendErrorResponse; canRetry: boolean }[] = [
    { error: { code: "unauthorized_window" }, canRetry: false },
    { error: { code: "invalid_state_transition" }, canRetry: false },
    { error: { code: "rate_limited" }, canRetry: true },
    {
      error: {
        code: "invalid_settings",
        issues: [{ code: "empty_time_range", weekday: "wed" }],
      },
      canRetry: false,
    },
    { error: { code: "window_control_error" }, canRetry: true },
    { error: { code: "persistence_error" }, canRetry: true },
  ];
  it.each(errors)("classifies delivered $error.code", ({ error, canRetry }) => {
    const failure = { status: "rejected", error } as const;
    expect(classifyCommandFailure(failure)).toMatchObject({
      canRetry,
      severity: "recoverable",
    });
    expect(classifyCommandFailure(failure).userMessage).not.toBe("");
    expect(formatCommandFailureDetail(failure)).toBe(error.code);
  });

  it.each([
    { code: "rate_limited", message: "private path" },
    { code: "invalid_command_input", message: "private path" },
    { code: "unknown_error", message: "private path" },
    new Error("private path"),
    "private path",
    null,
    undefined,
    500,
  ])(
    "keeps unavailable responses non-retryable and hides their details",
    (cause) => {
      const failure = { status: "unavailable", cause } as const;
      expect(classifyCommandFailure(failure)).toEqual({
        userMessage: "操作の結果を確認できませんでした",
        severity: "recoverable",
        canRetry: false,
      });
      expect(formatCommandFailureDetail(failure)).toBe(
        "Command response unavailable",
      );
    },
  );

  it("gives each rejection a distinct presentation", () => {
    expect(
      new Set(
        errors.map(
          ({ error }) =>
            classifyCommandFailure({ status: "rejected", error }).userMessage,
        ),
      ).size,
    ).toBe(errors.length);
  });
});

describe("local setup diagnostics", () => {
  it("formats errors and strings", () => {
    expect(formatBackendErrorDetail(new Error("network unavailable"))).toBe(
      "Error: network unavailable",
    );
    expect(formatBackendErrorDetail("listen failed")).toBe("listen failed");
  });
  it.each([
    { code: "rate_limited", message: "private" },
    { message: "private" },
    null,
    undefined,
    500,
    false,
    Symbol("backend"),
  ])("does not trust arbitrary object messages", (error) => {
    expect(formatBackendErrorDetail(error)).toBe("Unexpected backend error");
  });
});
