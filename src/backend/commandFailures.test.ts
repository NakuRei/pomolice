import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useBackendObservationStore } from "@/backend/observationStore";
import { useOperationStore } from "./operationStore";
import {
  startWork,
  emergencyStop,
  resolvePostBreakDecision,
  resolveBootAutoStartPrompt,
} from "@/features/timer/commands";
import { canRetryOperation, retryRejectedOperation } from "./retryOperation";
import {
  classifyCommandFailure,
  formatCommandFailureDetail,
} from "@/errors/classifyBackendError";
import type { TimerSnapshot } from "@/ipc/generated";
import type { OperationResult } from "./operation";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer command failures", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    resetAppStateForTest();
    useBackendObservationStore.setState({
      bootAutoStartOffer: createBootAutoStartOfferForTest(),
      requiresManualContinueDecision: true,
    });
  });

  it.each([
    { command: "start_work", run: startWork, args: undefined },
    { command: "emergency_stop", run: emergencyStop, args: undefined },
    {
      command: "resolve_post_break_decision",
      run: (): Promise<OperationResult> => resolvePostBreakDecision("continue"),
      args: { action: "continue" },
    },
    {
      command: "resolve_post_break_decision",
      run: (): Promise<OperationResult> =>
        resolvePostBreakDecision("stop_cycle"),
      args: { action: "stop_cycle" },
    },
    {
      command: "resolve_boot_auto_start_prompt",
      run: (): Promise<OperationResult> => resolveBootAutoStartPrompt(true),
      args: { startNow: true },
    },
    {
      command: "resolve_boot_auto_start_prompt",
      run: (): Promise<OperationResult> => resolveBootAutoStartPrompt(false),
      args: { startNow: false },
    },
  ])(
    "sends $command with the selected answer",
    async ({ command, run, args }) => {
      await run();
      expect(invoke).toHaveBeenCalledTimes(1);
      expect(vi.mocked(invoke).mock.calls[0]?.[0]).toBe(command);
      expect(vi.mocked(invoke).mock.calls[0]?.[1]).toEqual(args);
    },
  );

  it.each([
    {
      kind: "start_work",
      run: startWork,
      code: "window_control_error",
      retryable: true,
    },
    {
      kind: "resolve_post_break_decision",
      run: (): Promise<OperationResult> => resolvePostBreakDecision("continue"),
      code: "invalid_state_transition",
      retryable: false,
    },
    {
      kind: "emergency_stop",
      run: emergencyStop,
      code: "rate_limited",
      retryable: true,
    },
    {
      kind: "emergency_stop",
      run: emergencyStop,
      code: "invalid_state_transition",
      retryable: false,
    },
    {
      kind: "resolve_boot_auto_start_prompt",
      run: (): Promise<OperationResult> => resolveBootAutoStartPrompt(false),
      code: "rate_limited",
      retryable: true,
    },
  ] as const)(
    "retains $kind rejection and derives retry eligibility from $code",
    async ({ kind, run, code, retryable }) => {
      const response = { status: "rejected", error: { code } } as const;
      vi.mocked(invoke).mockResolvedValueOnce(response);
      await expect(run()).resolves.toEqual(response);
      const record = useOperationStore.getState().operations[kind];
      expect(record?.outcome).toEqual(response);
      expect(record && canRetryOperation(record, true)).toBe(retryable);
      expect(classifyCommandFailure(response).userMessage).not.toBe("");
      expect(formatCommandFailureDetail(response)).toBe(code);
      expect(
        useBackendObservationStore.getState().requiresManualContinueDecision,
      ).toBe(true);
    },
  );

  it.each([
    { code: "invalid_command_input" },
    { code: "unknown_error", message: "internal path C:/secret" },
    new Error("panic at C:/Users/nak/secret"),
  ])(
    "keeps an undelivered response unknown without exposing diagnostics",
    async (cause) => {
      vi.mocked(invoke).mockRejectedValueOnce(cause);
      const result = await resolveBootAutoStartPrompt(true);
      expect(result).toEqual({ status: "unavailable", cause });
      if (result.status !== "unavailable")
        throw new Error("Expected unavailable response");
      const record =
        useOperationStore.getState().operations.resolve_boot_auto_start_prompt;
      expect(record?.outcome).toEqual(result);
      expect(record && canRetryOperation(record, true)).toBe(false);
      expect(classifyCommandFailure(result).userMessage).toBe(
        "操作の結果を確認できませんでした",
      );
      expect(formatCommandFailureDetail(result)).toBe(
        "Command response unavailable",
      );
      expect(
        useBackendObservationStore.getState().bootAutoStartOffer,
      ).not.toBeNull();
    },
  );

  it("keeps a failed save separate from an accepted startup answer", async () => {
    const effects = createTimerEffectsForTest({
      revision: "1",
      last_timer_save: "failed",
    });
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "accepted",
      value: createTimerSnapshotForTest({ effects }),
    });
    await resolveBootAutoStartPrompt(true);
    expect(useBackendObservationStore.getState().bootAutoStartOffer).toBeNull();
    expect(
      useOperationStore.getState().operations.resolve_boot_auto_start_prompt
        ?.outcome,
    ).toEqual({ status: "accepted" });
    expect(useBackendObservationStore.getState().effects).toEqual(effects);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it("does not retry a rejection arriving after the startup offer closed", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({
      status: "rejected",
      error: { code: "rate_limited" },
    });
    const pending = resolveBootAutoStartPrompt(true);
    useBackendObservationStore.getState().clearBootAutoStartOffer();
    await pending;
    const record =
      useOperationStore.getState().operations.resolve_boot_auto_start_prompt;
    if (!record) throw new Error("Expected startup operation");
    expect(canRetryOperation(record, false)).toBe(false);
    await retryRejectedOperation(record);
    expect(invoke).toHaveBeenCalledTimes(1);
  });

  it.each(["before failure", "after failure"])(
    "keeps failed effects when a duplicate answer sent %s completes",
    async (order) => {
      let completeDuplicate: (value: TimerSnapshot) => void = () => {
        throw new Error("Expected pending response");
      };
      const duplicate = new Promise<TimerSnapshot>((resolve) => {
        completeDuplicate = resolve;
      });
      const snapshot = createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          revision: "1",
          last_timer_save: "failed",
        }),
      });
      vi.mocked(invoke)
        .mockResolvedValueOnce({ status: "accepted", value: snapshot })
        .mockReturnValueOnce(
          duplicate.then((value) => ({ status: "accepted", value })),
        );
      const first = resolveBootAutoStartPrompt(true);
      const overlapping =
        order === "before failure"
          ? resolveBootAutoStartPrompt(true)
          : undefined;
      await first;
      const second = overlapping ?? resolveBootAutoStartPrompt(true);
      completeDuplicate(snapshot);
      await second;
      expect(
        useBackendObservationStore.getState().effects?.last_timer_save,
      ).toBe("failed");
      expect(
        useOperationStore.getState().operations.resolve_boot_auto_start_prompt
          ?.outcome,
      ).toEqual({ status: "accepted" });
    },
  );
});
