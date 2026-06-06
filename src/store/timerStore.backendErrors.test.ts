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

import { useErrorStore, type AppError } from "./errorStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { useTimerStore } from "./timerStore";
import { useSettingsStore } from "./settingsStore";

async function expectCommandRejection(
  action: () => Promise<void>,
  backendError: BackendErrorResponse,
): Promise<void> {
  await expect(action()).rejects.toBe(backendError);
}

function expectUserMessageExcludesDiagnostics(
  error: AppError | undefined,
  diagnosticFragments: string[],
): void {
  expect(error?.message).toEqual(expect.any(String));
  expect(error?.message).not.toBe("");

  for (const fragment of diagnosticFragments) {
    expect(error?.message).not.toContain(fragment);
  }
}

function expectTechnicalDetailExcludesDiagnostics(
  error: AppError | undefined,
  diagnosticFragments: string[],
): void {
  expect(error?.technicalDetail).toEqual(expect.any(String));
  expect(error?.technicalDetail).not.toBe("");

  for (const fragment of diagnosticFragments) {
    expect(error?.technicalDetail).not.toContain(fragment);
  }
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
      message: "failed to enter fullscreen at C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().start(),
      backendError,
    );

    expect(useErrorStore.getState().errors.start_work).toMatchObject({
      id: "start_work",
      technicalDetail: "window_control_error",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: true,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.start_work,
      ["failed to enter fullscreen", "C:\\Users\\nak", "secret"],
    );
    expect(useErrorStore.getState().errors.start_work?.retryFn).toEqual(
      expect.any(Function),
    );
  });

  it("stores safe structured details for resolve_post_break_decision rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "invalid_state_transition",
      message: "no decision is pending at C:\\Users\\nak\\secret",
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
      technicalDetail: "invalid_state_transition",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: false,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.resolve_post_break_decision,
      ["no decision is pending", "C:\\Users\\nak", "secret"],
    );
    expect(
      useErrorStore.getState().errors.resolve_post_break_decision?.retryFn,
    ).toBeNull();
  });

  it("stores safe structured details for emergency_stop rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "rate_limited",
      message: "please wait 300ms near C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().emergencyStop(),
      backendError,
    );

    expect(useErrorStore.getState().errors.emergency_stop).toMatchObject({
      id: "emergency_stop",
      technicalDetail: "rate_limited",
      severity: "critical",
      origin: "backend_command",
      canRetry: true,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.emergency_stop,
      ["please wait 300ms", "C:\\Users\\nak", "secret"],
    );
    expect(useErrorStore.getState().errors.emergency_stop?.retryFn).toEqual(
      expect.any(Function),
    );
  });

  it("keeps emergency_stop non-retryable for non-retryable backend errors", async () => {
    const backendError: BackendErrorResponse = {
      code: "invalid_state_transition",
      message: "already idle at C:\\Users\\nak\\secret",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().emergencyStop(),
      backendError,
    );

    expect(useErrorStore.getState().errors.emergency_stop).toMatchObject({
      id: "emergency_stop",
      technicalDetail: "invalid_state_transition",
      severity: "critical",
      origin: "backend_command",
      canRetry: false,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.emergency_stop,
      ["already idle", "C:\\Users\\nak", "secret"],
    );
    expect(useErrorStore.getState().errors.emergency_stop?.retryFn).toBeNull();
  });

  it("stores safe details for invalid boot prompt command input", async () => {
    const backendError: BackendErrorResponse = {
      code: "invalid_command_input",
      message: "invalid type: string C:\\secret",
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
      technicalDetail: "invalid_command_input",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: false,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
      ["invalid type", "C:\\secret"],
    );
    expect(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt?.retryFn,
    ).toBeNull();
  });

  it("stores retry actions for retryable boot prompt rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "rate_limited",
      message: "please wait 300ms before resolving boot prompt",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expectCommandRejection(
      () => useTimerStore.getState().resolveBootAutoStartPrompt(false),
      backendError,
    );

    expect(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
    ).toMatchObject({
      id: "resolve_boot_auto_start_prompt",
      technicalDetail: "rate_limited",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: true,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
      ["please wait 300ms"],
    );
    expect(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt?.retryFn,
    ).toEqual(expect.any(Function));
  });

  it("stores retry actions for boot prompt persistence rejections", async () => {
    const backendError: BackendErrorResponse = {
      code: "persistence_error",
      message: "failed to store boot prompt decision at C:\\secret",
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
      technicalDetail: "persistence_error",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: true,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt,
      ["failed to store boot prompt decision", "C:\\secret"],
    );
    expect(
      useErrorStore.getState().errors.resolve_boot_auto_start_prompt?.retryFn,
    ).toEqual(expect.any(Function));
  });

  it("uses generic details for unknown object rejections", async () => {
    const backendError = {
      code: "unknown_error",
      message: "internal path C:\\secret",
    };
    mockInvoke.mockRejectedValueOnce(backendError);

    await expect(useTimerStore.getState().start()).rejects.toBe(backendError);

    expect(useErrorStore.getState().errors.start_work).toMatchObject({
      canRetry: false,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.start_work,
      ["internal path", "C:\\secret"],
    );
    expectTechnicalDetailExcludesDiagnostics(
      useErrorStore.getState().errors.start_work,
      ["internal path", "C:\\secret"],
    );
    expect(useErrorStore.getState().errors.start_work?.retryFn).toBeNull();
  });

  it("uses generic details for raw command rejection values", async () => {
    const backendError = new Error("panic at C:\\Users\\nak\\secret");
    mockInvoke.mockRejectedValueOnce(backendError);

    await expect(useTimerStore.getState().start()).rejects.toBe(backendError);

    expect(useErrorStore.getState().errors.start_work).toMatchObject({
      canRetry: false,
    });
    expectUserMessageExcludesDiagnostics(
      useErrorStore.getState().errors.start_work,
      ["panic", "C:\\Users\\nak", "secret"],
    );
    expectTechnicalDetailExcludesDiagnostics(
      useErrorStore.getState().errors.start_work,
      ["panic", "C:\\Users\\nak", "secret"],
    );
    expect(useErrorStore.getState().errors.start_work?.retryFn).toBeNull();
  });
});
