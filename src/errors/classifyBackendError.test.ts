import { describe, expect, it } from "vitest";
import {
  classifyBackendError,
  formatBackendErrorDetail,
} from "./classifyBackendError";

describe("classifyBackendError", () => {
  it("classifies known structured backend error responses by code", () => {
    expect(
      classifyBackendError({
        code: "invalid_state_transition",
        message: "cannot start_work from idle",
      }),
    ).toEqual({
      userMessage: "現在の状態では実行できません",
      isTransient: false,
      severity: "recoverable",
    });

    expect(
      classifyBackendError({
        code: "rate_limited",
        message: "please wait 300ms",
      }),
    ).toEqual({
      userMessage: "少々お待ちください",
      isTransient: true,
      severity: "recoverable",
    });

    expect(
      classifyBackendError({
        code: "window_control_error",
        message: "failed to lock window",
      }),
    ).toEqual({
      userMessage: "ウィンドウ操作に失敗しました",
      isTransient: false,
      severity: "recoverable",
    });

    expect(
      classifyBackendError({
        code: "persistence_error",
        message: "failed to save timer state",
      }),
    ).toEqual({
      userMessage: "データの保存に失敗しました",
      isTransient: false,
      severity: "recoverable",
    });

    expect(
      classifyBackendError({
        code: "unauthorized_window",
        message: "window is not owned",
      }),
    ).toEqual({
      userMessage: "権限エラーが発生しました",
      isTransient: false,
      severity: "recoverable",
    });
  });

  it("uses the default classification for malformed values, unknown codes, and legacy strings", () => {
    const defaultClassification = {
      userMessage: "予期しないエラーが発生しました",
      isTransient: false,
      severity: "recoverable",
    };

    expect(
      classifyBackendError({
        code: "unknown_error",
        message: "should not be trusted",
      }),
    ).toEqual(defaultClassification);
    expect(
      classifyBackendError({ code: "rate_limited", message: 300 }),
    ).toEqual(defaultClassification);
    expect(classifyBackendError("Rate limited: please wait 300ms")).toEqual(
      defaultClassification,
    );
    expect(classifyBackendError(null)).toEqual(defaultClassification);
    expect(classifyBackendError(undefined)).toEqual(defaultClassification);
    expect(classifyBackendError(500)).toEqual(defaultClassification);
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
