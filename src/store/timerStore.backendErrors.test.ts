import { beforeEach, describe, expect, it, vi } from "vitest";
import type { BackendErrorResponse } from "@/errors/classifyBackendError";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

import { useErrorStore } from "./errorStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { useTimerStore } from "./timerStore";
import { useSettingsStore } from "./settingsStore";

async function expectCommandRejection(
  action: () => Promise<void>,
  backendError: BackendErrorResponse,
): Promise<void> {
  await expect(action()).rejects.toBe(backendError);
}

describe("timerStore backend command errors", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useErrorStore.getState().clearAllErrors();
    resetTimerStoreForTest();
    useSettingsStore.getState().resetToDefaults();
  });

  it("stores safe structured details for start_work rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "window_control_error",
      message: "failed to enter fullscreen",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().start(),
      backendError,
    );

    expect(useErrorStore.getState().errors.start_work).toMatchObject({
      id: "start_work",
      message: "ウィンドウ操作に失敗しました",
      technicalDetail: "window_control_error: failed to enter fullscreen",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: false,
    });
  });

  it("stores safe structured details for resolve_post_break_decision rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "invalid_state_transition",
      message: "no decision is pending",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().resolvePostBreakDecision("continue"),
      backendError,
    );

    expect(
      useErrorStore.getState().errors.resolve_post_break_decision,
    ).toMatchObject({
      id: "resolve_post_break_decision",
      message: "現在の状態では実行できません",
      technicalDetail: "invalid_state_transition: no decision is pending",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: true,
    });
  });

  it("stores safe structured details for emergency_stop rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "rate_limited",
      message: "please wait 300ms",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().emergencyStop(),
      backendError,
    );

    expect(useErrorStore.getState().errors.emergency_stop).toMatchObject({
      id: "emergency_stop",
      message: "少々お待ちください",
      technicalDetail: "rate_limited: please wait 300ms",
      severity: "critical",
      origin: "backend_command",
      canRetry: true,
    });
  });

  it("stores safe structured details for resolve_boot_auto_start_prompt rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "persistence_error",
      message: "failed to store boot prompt decision",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().resolveBootAutoStartPrompt(true),
      backendError,
    );

    expect(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
    ).toMatchObject({
      id: "resolve_boot_auto_start_prompt",
      message: "データの保存に失敗しました",
      technicalDetail:
        "persistence_error: failed to store boot prompt decision",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: true,
    });
  });

  it("uses generic details for unknown object rejections", async () => {
    const backendError = {
      code: "unknown_error",
      message: "internal path C:\\secret",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expect(useTimerStore.getState().start()).rejects.toBe(backendError);

    expect(useErrorStore.getState().errors.start_work).toMatchObject({
      message: "予期しないエラーが発生しました",
      technicalDetail: "Unexpected backend error",
    });
  });
});
