import { describe, expect, it } from "vitest";
import { classifyBackendError } from "./classifyBackendError";

describe("classifyBackendError", () => {
  it("classifies exact-match state errors as recoverable and non-transient", () => {
    expect(classifyBackendError("Timer already running", "start_work")).toEqual(
      {
        userMessage: "すでに実行中です",
        isTransient: false,
        severity: "recoverable",
      },
    );
  });

  it("classifies rate-limit errors as transient", () => {
    expect(
      classifyBackendError("Rate limited: please wait 300ms", "start_work"),
    ).toEqual({
      userMessage: "少々お待ちください",
      isTransient: true,
      severity: "recoverable",
    });
  });

  it("treats emergency-stop lock failures as critical", () => {
    expect(
      classifyBackendError("Failed to lock state: poisoned", "emergency_stop"),
    ).toEqual({
      userMessage: "緊急停止に失敗しました。もう一度試してください",
      isTransient: true,
      severity: "critical",
    });
  });

  it("treats non-emergency lock failures as recoverable", () => {
    expect(
      classifyBackendError("Failed to lock state: poisoned", "start_work"),
    ).toEqual({
      userMessage: "内部エラーが発生しました。もう一度試してください",
      isTransient: true,
      severity: "recoverable",
    });
  });

  it("falls back to the default classification for unknown errors", () => {
    expect(
      classifyBackendError("Unknown backend panic", "get_full_state"),
    ).toEqual({
      userMessage: "予期しないエラーが発生しました",
      isTransient: false,
      severity: "recoverable",
    });
  });

  it("classifies sanitized persistence errors by prefix contract", () => {
    expect(
      classifyBackendError(
        "Persistence error: failed to save timer state",
        "start_work",
      ),
    ).toEqual({
      userMessage: "データの保存に失敗しました",
      isTransient: false,
      severity: "recoverable",
    });
  });
});
