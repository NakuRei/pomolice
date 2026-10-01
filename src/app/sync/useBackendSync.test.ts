import { useBackendObservationStore } from "@/backend/observationStore";
import { acceptedCommand } from "@/test/commandResponseFixture";
import type {
  TimerSnapshot,
  TimerEffectsState,
  FullStatePayload,
  SessionCompletedPayload,
  TimerTickPayload,
} from "@/ipc/generated";

/**
 * useBackendSync Hook Tests
 *
 * Note: Timer tick logic is handled by the Rust backend.
 * These tests verify state synchronization and connection recovery.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import {
  isPermissionGranted,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createBootAutoStartOfferForTest } from "@/test/bootAutoStartOfferFixture";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

import { useBackendSyncStore } from "@/backend/syncStore";
import { useTimerNoticeStore } from "@/features/timer/noticeStore";
import {
  describeTimerSyncFailure,
  formatTimerSyncConnectionErrorDetail,
} from "../../backend/connectionError";
import {
  createSettingsForTest,
  settingsPayloadForTest,
} from "@/test/settingsFixture";

// Mock Tauri APIs
type TimerEventHandler = (event: { payload: unknown }) => void;
type ListenerRegistration = (
  event: string,
  handler: TimerEventHandler | (() => void),
) => Promise<() => void>;
type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockListen = vi.fn<ListenerRegistration>();
const mockInvoke = vi.fn<InvokeCommand>();
const noopUnlisten = (): void => undefined;

vi.mock("@tauri-apps/api/event", () => ({
  listen: (
    event: string,
    handler: TimerEventHandler | (() => void),
  ): Promise<() => void> => mockListen(event, handler),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

vi.mock("@tauri-apps/plugin-log", () => ({
  trace: vi.fn().mockResolvedValue(undefined),
  debug: vi.fn().mockResolvedValue(undefined),
  info: vi.fn().mockResolvedValue(undefined),
  warn: vi.fn().mockResolvedValue(undefined),
  error: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("@tauri-apps/api/path", () => ({
  resolveResource: vi.fn((path: string) => Promise.resolve(`/app/${path}`)),
}));

// Import after mocking
import { useBackendSync } from "./useBackendSync";

function createFullStatePayload(
  overrides: Partial<FullStatePayload> = {},
  effects?: TimerEffectsState,
): TimerSnapshot {
  const snapshot = createTimerSnapshotForTest();
  return {
    timer: { ...snapshot.timer, ...overrides },
    effects: effects ?? snapshot.effects,
  };
}

function renderTimerSync(): ReturnType<typeof renderHook> {
  return renderHook(() => {
    useBackendSync();
  });
}

function getRequiredTimerEventHandler(
  handler: TimerEventHandler | undefined,
  description: string,
): TimerEventHandler {
  if (!handler) {
    throw new Error(`Expected ${description} to be defined`);
  }

  return handler;
}

async function renderAndGetTimerEventHandler(
  eventName: string,
): Promise<TimerEventHandler> {
  let capturedHandler: TimerEventHandler | undefined = undefined;
  mockListen.mockImplementation((event, handler) => {
    if (event === eventName) {
      capturedHandler = handler;
    }
    return Promise.resolve(noopUnlisten);
  });

  renderTimerSync();
  await vi.waitFor(() => {
    expect(capturedHandler).toBeDefined();
  });
  return getRequiredTimerEventHandler(capturedHandler, `${eventName} handler`);
}

function emitTimerTick(
  handler: TimerEventHandler,
  overrides: Partial<TimerTickPayload> = {},
): void {
  handler({
    payload: {
      status: "working",
      remaining_seconds: 1499,
      session_duration_secs: 1500,
      session_type: "work",
      current_cycle: 1,
      completed_pomodoros: 0,
      break_interaction_count: 0,
      away_during_break: false,
      ...overrides,
    },
  });
}

function emitSessionCompleted(
  handler: TimerEventHandler,
  overrides: Partial<SessionCompletedPayload> = {},
): void {
  handler({
    payload: {
      transition_reason: "break_waiting_decision",
      completed_session_type: "short_break",
      next_status: "idle",
      next_session_type: "work",
      requires_manual_continue_decision: true,
      ...overrides,
    },
  });
}

async function flushTimerSyncSetup(): Promise<void> {
  await vi.advanceTimersByTimeAsync(0);
}

async function advanceThroughTimerSyncConnectionRetries(): Promise<void> {
  await vi.advanceTimersByTimeAsync(7000);
}

function getCommandInvokeCount(commandName: string): number {
  return mockInvoke.mock.calls.filter(([command]) => command === commandName)
    .length;
}

function getTimerTickListenCount(): number {
  return mockListen.mock.calls.filter(([event]) => event === "timer_tick")
    .length;
}

function invokedCommandNames(): string[] {
  return mockInvoke.mock.calls.map(([command]) => command);
}

function mockCommandFailure(commandName: string, error: unknown): void {
  const rejectCommand = vi.fn<() => Promise<never>>().mockRejectedValue(error);
  mockInvoke.mockImplementation((command) =>
    command === commandName
      ? rejectCommand()
      : Promise.resolve(acceptedCommand(createFullStatePayload())),
  );
}

describe("useBackendSync", () => {
  beforeEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
    vi.mocked(isPermissionGranted).mockResolvedValue(false);

    resetAppStateForTest();

    // Default mock implementations
    mockListen.mockResolvedValue(noopUnlisten);
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createFullStatePayload(),
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("connects by reading the backend state and adopts its settings without sending any", async () => {
    vi.useFakeTimers();
    const backendSettings = createSettingsForTest({
      workDurationSecs: 50 * 60,
    });
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createFullStatePayload({
        settings: settingsPayloadForTest(backendSettings),
      }),
    });

    renderTimerSync();
    await flushTimerSyncSetup();

    expect(useBackendSyncStore.getState().connection.status).toBe("ready");
    expect(useBackendObservationStore.getState().settings).toEqual(
      backendSettings,
    );
    expect(invokedCommandNames()).toEqual(["get_full_state"]);
  });

  it("subscribes before reading the startup offer from the backend", async () => {
    const offer = createBootAutoStartOfferForTest();
    mockInvoke.mockImplementation(() => {
      expect(mockListen.mock.calls.map(([event]) => event)).toEqual(
        expect.arrayContaining([
          "timer_tick",
          "session_completed",
          "boot_auto_start_offer_closed",
        ]),
      );
      return Promise.resolve(
        acceptedCommand(
          createFullStatePayload({ boot_auto_start_offer: offer }),
        ),
      );
    });

    renderTimerSync();
    await waitFor(() => {
      expect(useBackendSyncStore.getState().connection.status).toBe("ready");
    });
    expect(useBackendObservationStore.getState().bootAutoStartOffer).toEqual(
      offer,
    );
    expect(invokedCommandNames()).toEqual(["get_full_state"]);
  });

  it("keeps effect recovery newer than an earlier initial snapshot", async () => {
    let resolveState: (payload: TimerSnapshot) => void = () => {
      throw new Error("Expected a pending snapshot");
    };
    const pending = new Promise<TimerSnapshot>((resolve) => {
      resolveState = resolve;
    });
    mockInvoke.mockReturnValue(pending.then(acceptedCommand));
    const effectsChanged = await renderAndGetTimerEventHandler(
      "timer_effects_changed",
    );
    await waitFor(() => {
      expect(getCommandInvokeCount("get_full_state")).toBe(1);
    });
    act(() => {
      effectsChanged({
        payload: createTimerEffectsForTest({
          revision: "2",
          last_timer_save: "succeeded",
        }),
      });
    });
    await act(async () => {
      resolveState(
        createFullStatePayload(
          {},
          createTimerEffectsForTest({
            revision: "1",
            last_timer_save: "failed",
          }),
        ),
      );
      await pending;
    });
    expect(useBackendObservationStore.getState().effects?.last_timer_save).toBe(
      "succeeded",
    );
  });

  it("closes the startup offer when the backend expires it", async () => {
    const offer = createBootAutoStartOfferForTest();
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createFullStatePayload({ boot_auto_start_offer: offer }),
    });
    const closeOffer = await renderAndGetTimerEventHandler(
      "boot_auto_start_offer_closed",
    );
    await waitFor(() => {
      expect(useBackendObservationStore.getState().bootAutoStartOffer).toEqual(
        offer,
      );
    });

    act(() => {
      closeOffer({ payload: {} });
    });

    expect(useBackendObservationStore.getState().bootAutoStartOffer).toBeNull();
    expect(useBackendSyncStore.getState().connection.status).toBe("ready");
  });

  it("does not reopen an expired offer when an earlier full state arrives late", async () => {
    let resolveFullState: (payload: TimerSnapshot) => void = () => {
      throw new Error("Expected the full-state request to be pending");
    };
    const pendingFullState = new Promise<TimerSnapshot>((resolve) => {
      resolveFullState = resolve;
    });
    mockInvoke.mockReturnValue(pendingFullState.then(acceptedCommand));
    const closeOffer = await renderAndGetTimerEventHandler(
      "boot_auto_start_offer_closed",
    );
    await waitFor(() => {
      expect(getCommandInvokeCount("get_full_state")).toBe(1);
    });
    act(() => {
      closeOffer({ payload: {} });
    });

    await act(async () => {
      resolveFullState(
        createFullStatePayload({
          boot_auto_start_offer: createBootAutoStartOfferForTest(),
        }),
      );
      await pendingFullState;
    });

    expect(useBackendObservationStore.getState().bootAutoStartOffer).toBeNull();
    expect(useBackendSyncStore.getState().connection.status).toBe("ready");
  });

  it("receives session completion events while the initial full state is pending", async () => {
    let resolveFullState: (payload: TimerSnapshot) => void = () => {
      throw new Error("Expected the full-state request to be pending");
    };
    const pendingFullState = new Promise<TimerSnapshot>((resolve) => {
      resolveFullState = resolve;
    });
    mockInvoke.mockImplementation((command) =>
      command === "get_full_state"
        ? pendingFullState.then(acceptedCommand)
        : Promise.resolve(undefined),
    );

    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_full_state", undefined);
    });
    emitSessionCompleted(sessionHandler);

    expect(
      useBackendObservationStore.getState().requiresManualContinueDecision,
    ).toBe(true);

    await act(async () => {
      resolveFullState(createFullStatePayload());
      await pendingFullState;
    });
  });

  it("reports get_full_state command failures through code-derived connection presentation", async () => {
    vi.useFakeTimers();
    const backendError = {
      code: "window_control_error",
      message: "failed to read full state from C:\\Users\\nak\\secret",
    };
    mockInvoke.mockResolvedValue({ status: "rejected", error: backendError });

    renderTimerSync();

    await flushTimerSyncSetup();
    await advanceThroughTimerSyncConnectionRetries();

    const connection = useBackendSyncStore.getState().connection;
    expect(connection).toEqual({
      status: "failed",
      failure: { status: "rejected", error: backendError },
    });
    if (connection.status !== "failed")
      throw new Error("Expected failed connection");
    expect(describeTimerSyncFailure(connection.failure)).toMatchObject({
      canRetry: true,
      userMessage: "ウィンドウ操作に失敗しました",
    });
    expect(formatTimerSyncConnectionErrorDetail(connection.failure)).toBe(
      "window_control_error",
    );
  });

  it("retries raw get_full_state invoke failures as connection failures", async () => {
    vi.useFakeTimers();
    mockCommandFailure(
      "get_full_state",
      new Error("transport failed from C:\\Users\\nak\\secret"),
    );

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(getCommandInvokeCount("get_full_state")).toBe(1);
    expect(useBackendSyncStore.getState().connection.status).toBe("retry_wait");

    await advanceThroughTimerSyncConnectionRetries();

    expect(getCommandInvokeCount("get_full_state")).toBe(4);
    const connection = useBackendSyncStore.getState().connection;
    expect(connection).toMatchObject({
      status: "failed",
      failure: { status: "unavailable" },
    });
    if (connection.status !== "failed")
      throw new Error("Expected failed connection");
    expect(describeTimerSyncFailure(connection.failure).canRetry).toBe(true);
    expect(formatTimerSyncConnectionErrorDetail(connection.failure)).toBe(
      "Command response unavailable",
    );
  });

  it("reports non-retryable get_full_state command failures without automatic retries", async () => {
    vi.useFakeTimers();
    const backendError = {
      code: "invalid_state_transition",
      message: "invalid type near C:\\Users\\nak\\secret",
    };
    mockInvoke.mockResolvedValue({ status: "rejected", error: backendError });

    renderTimerSync();

    await flushTimerSyncSetup();

    const connection = useBackendSyncStore.getState().connection;
    expect(connection).toEqual({
      status: "failed",
      failure: { status: "rejected", error: backendError },
    });
    if (connection.status !== "failed")
      throw new Error("Expected failed connection");
    expect(describeTimerSyncFailure(connection.failure)).toMatchObject({
      canRetry: false,
      userMessage: "現在の状態では実行できません",
    });

    expect(getCommandInvokeCount("get_full_state")).toBe(1);

    await advanceThroughTimerSyncConnectionRetries();

    expect(getCommandInvokeCount("get_full_state")).toBe(1);
  });

  it("should clean up listeners from a failed setup attempt before retrying", async () => {
    const unlistenTick = vi.fn();

    mockListen.mockImplementation((event: string) => {
      if (event === "timer_tick") {
        return Promise.resolve(unlistenTick);
      }

      if (event === "session_completed") {
        return Promise.reject(new Error("session listener failed"));
      }

      return Promise.resolve(noopUnlisten);
    });

    renderTimerSync();

    await waitFor(() => {
      expect(unlistenTick).toHaveBeenCalledTimes(1);
    });
  });

  it("retries connection setup after 1, 2, and 4 seconds, then reports the failure", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(getTimerTickListenCount()).toBe(1);

    await vi.advanceTimersByTimeAsync(999);
    expect(getTimerTickListenCount()).toBe(1);
    await vi.advanceTimersByTimeAsync(1);
    expect(getTimerTickListenCount()).toBe(2);

    await vi.advanceTimersByTimeAsync(1999);
    expect(getTimerTickListenCount()).toBe(2);
    await vi.advanceTimersByTimeAsync(1);
    expect(getTimerTickListenCount()).toBe(3);

    await vi.advanceTimersByTimeAsync(3999);
    expect(getTimerTickListenCount()).toBe(3);
    expect(useBackendSyncStore.getState().connection.status).toBe("retry_wait");
    await vi.advanceTimersByTimeAsync(1);
    expect(getTimerTickListenCount()).toBe(4);

    const connection = useBackendSyncStore.getState().connection;
    expect(connection).toMatchObject({
      status: "failed",
      failure: { status: "setup" },
    });
    if (connection.status !== "failed")
      throw new Error("Expected failed connection");
    expect(describeTimerSyncFailure(connection.failure).canRetry).toBe(true);
    expect(formatTimerSyncConnectionErrorDetail(connection.failure)).toBe(
      "Error: listen failed",
    );

    await vi.advanceTimersByTimeAsync(60_000);
    expect(getTimerTickListenCount()).toBe(4);
  });

  it("manual retry clears the error and grants three new retries", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    expect(getTimerTickListenCount()).toBe(1);

    await advanceThroughTimerSyncConnectionRetries();

    expect(useBackendSyncStore.getState().connection.status).toBe("failed");

    mockListen.mockClear();
    act(() => {
      useBackendSyncStore.getState().reconnect();
    });
    await flushTimerSyncSetup();

    expect(useBackendSyncStore.getState().connection.status).toBe("retry_wait");
    expect(getTimerTickListenCount()).toBe(1);

    await advanceThroughTimerSyncConnectionRetries();
    expect(getTimerTickListenCount()).toBe(4);
    expect(useBackendSyncStore.getState().connection.status).toBe("failed");
  });

  it("should reconnect successfully when manual retry setup succeeds", async () => {
    vi.useFakeTimers();
    mockListen.mockRejectedValue(new Error("listen failed"));

    renderTimerSync();

    await flushTimerSyncSetup();
    await advanceThroughTimerSyncConnectionRetries();

    expect(useBackendSyncStore.getState().connection.status).toBe("failed");

    mockListen.mockClear();
    mockInvoke.mockClear();
    mockListen.mockResolvedValue(noopUnlisten);
    act(() => {
      useBackendSyncStore.getState().reconnect();
    });
    await flushTimerSyncSetup();

    expect(useBackendSyncStore.getState().connection.status).toBe("ready");
    expect(mockInvoke.mock.calls).toContainEqual(["get_full_state", undefined]);
    expect(mockListen.mock.calls.map(([event]) => event)).toContain(
      "timer_tick",
    );
  });

  it("restores the running timer and marks the connection available", async () => {
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createFullStatePayload({
        status: "working",
        remaining_seconds: 777,
        current_cycle: 2,
      }),
    });
    renderTimerSync();

    await waitFor(() => {
      expect(useBackendObservationStore.getState()).toMatchObject({
        status: "working",
        remainingSeconds: 777,
        currentCycle: 2,
      });
    });
  });

  it("should handle timer_tick events", async () => {
    const tickHandler = await renderAndGetTimerEventHandler("timer_tick");
    emitTimerTick(tickHandler);

    expect(useBackendObservationStore.getState().status).toBe("working");
    expect(useBackendObservationStore.getState().remainingSeconds).toBe(1499);
  });

  it("should handle session_completed events", async () => {
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler);

    expect(
      useBackendObservationStore.getState().requiresManualContinueDecision,
    ).toBe(true);
  });

  it("notifies the user with the long-break sound when work ends before a long break", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "work_to_break",
      completed_session_type: "work",
      next_status: "on_break",
      next_session_type: "long_break",
      requires_manual_continue_decision: false,
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "休憩時間です！",
        body: "お疲れ様でした。休憩を取りましょう。",
        sound: "/app/resources/sounds/long_break.wav",
      });
    });
  });

  it("notifies the user to resume work when a break awaits their decision", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      completed_session_type: "long_break",
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "作業に戻りましょう！",
        body: "休憩が終わりました。次のポモドーロを始めましょう。",
        sound: "/app/resources/sounds/work.wav",
      });
    });
  });

  it("notifies the user when the next work session starts automatically", async () => {
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "break_auto_started",
      next_status: "working",
      requires_manual_continue_decision: false,
    });

    await waitFor(() => {
      expect(sendNotification).toHaveBeenCalledExactlyOnceWith({
        title: "休憩終了",
        body: "休憩終了。次の作業を自動開始しました。",
        sound: "/app/resources/sounds/work.wav",
      });
    });
  });

  it("does not notify the user to resume work after a scheduled stop", async () => {
    vi.useFakeTimers();
    vi.mocked(isPermissionGranted).mockResolvedValue(true);
    vi.spyOn(navigator, "userAgent", "get").mockReturnValue("Linux");
    const sessionHandler =
      await renderAndGetTimerEventHandler("session_completed");
    emitSessionCompleted(sessionHandler, {
      transition_reason: "scheduled_stop_executed",
      requires_manual_continue_decision: false,
    });

    await vi.runOnlyPendingTimersAsync();

    expect(sendNotification).not.toHaveBeenCalled();
  });

  it("releases a listener registered after unmount without continuing setup", async () => {
    const unlisten = vi.fn();
    let resolveRegistration: (unlisten: () => void) => void = () => {
      throw new Error("Expected a pending listener registration");
    };
    const pendingRegistration = new Promise<() => void>((resolve) => {
      resolveRegistration = resolve;
    });
    mockListen.mockReturnValue(pendingRegistration);

    const { unmount } = renderTimerSync();
    unmount();

    await act(async () => {
      resolveRegistration(unlisten);
      await pendingRegistration;
    });

    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(useBackendSyncStore.getState().connection.status).toBe("idle");
  });

  it("cancels a pending connection retry on unmount", async () => {
    vi.useFakeTimers();
    mockCommandFailure("get_full_state", new Error("connection unavailable"));

    const { unmount } = renderTimerSync();
    await flushTimerSyncSetup();
    expect(getCommandInvokeCount("get_full_state")).toBe(1);

    unmount();
    mockListen.mockClear();
    mockInvoke.mockClear();
    await advanceThroughTimerSyncConnectionRetries();

    expect(mockListen).not.toHaveBeenCalled();
    expect(mockInvoke).not.toHaveBeenCalled();
    expect(useBackendSyncStore.getState().connection.status).toBe("idle");
  });

  it("releases every listener and ignores full state resolved after unmount", async () => {
    const acquiredUnlisteners: ReturnType<typeof vi.fn>[] = [];
    mockListen.mockImplementation(() => {
      const unlisten = vi.fn();
      acquiredUnlisteners.push(unlisten);
      return Promise.resolve(unlisten);
    });

    let resolvePendingFullState: (payload: TimerSnapshot) => void = () => {
      throw new Error("Expected pending full-state resolver");
    };
    const pendingFullState = new Promise<TimerSnapshot>((resolve) => {
      resolvePendingFullState = resolve;
    });
    mockInvoke.mockImplementation((command) => {
      if (command === "get_full_state") {
        return pendingFullState.then(acceptedCommand);
      }
      return Promise.resolve(undefined);
    });

    const remainingSecondsBeforeUnmount = 823;
    useBackendObservationStore.setState({
      remainingSeconds: remainingSecondsBeforeUnmount,
    });
    const { unmount } = renderTimerSync();

    await waitFor(() => {
      expect(getCommandInvokeCount("get_full_state")).toBe(1);
    });

    unmount();

    expect(acquiredUnlisteners).not.toHaveLength(0);
    for (const unlisten of acquiredUnlisteners) {
      expect(unlisten).toHaveBeenCalledTimes(1);
    }

    await act(async () => {
      resolvePendingFullState(
        createFullStatePayload({ remaining_seconds: 17 }),
      );
      await pendingFullState;
      await Promise.resolve();
    });

    expect(useBackendObservationStore.getState().remainingSeconds).toBe(
      remainingSecondsBeforeUnmount,
    );
    expect(useBackendSyncStore.getState().connection.status).toBe("idle");
    expect(useBackendObservationStore.getState().settings).toBeNull();
  });

  it("should show stop-pending info on auto_run_stop_pending events", async () => {
    const stopPendingListener = await renderAndGetTimerEventHandler(
      "auto_run_stop_pending",
    );
    stopPendingListener({ payload: {} });

    expect(useTimerNoticeStore.getState().infoMessage?.id).toBe(
      "auto_run_stop_pending",
    );
  });

  it("should show stopped info on auto_run_stopped events", async () => {
    const stoppedListener =
      await renderAndGetTimerEventHandler("auto_run_stopped");
    stoppedListener({ payload: {} });

    expect(useTimerNoticeStore.getState().infoMessage?.id).toBe(
      "auto_run_stopped",
    );
  });

  it("takes the settings announced by settings_changed events", async () => {
    const settingsListener =
      await renderAndGetTimerEventHandler("settings_changed");
    const savedSettings = createSettingsForTest({
      notificationSoundMuted: true,
    });

    settingsListener({
      payload: {
        settings: settingsPayloadForTest(savedSettings),
        settings_file_status: "ok",
      },
    });

    expect(useBackendObservationStore.getState().settings).toEqual(
      savedSettings,
    );
  });
});
