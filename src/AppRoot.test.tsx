import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AppRoot } from "./AppRoot";
import { useTimerObservationStore } from "./store/timerObservationStore";
import { useTimerSyncStore } from "./store/timerSyncStore";
import { useTimerOperationStore } from "./operations/operationStore";
import { useSkipFlowStore } from "./store/skipFlowStore";
import { resetTimerStoreForTest } from "./test/resetTimerStore";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "./test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn() }));
vi.mock("./components/shared/TrayUnavailableNotice", () => ({
  TrayUnavailableNotice: (): null => null,
}));
vi.mock("./components/shared/InputMonitorFailureNotice", () => ({
  InputMonitorFailureNotice: (): null => null,
}));

const lockedEffects = createTimerEffectsForTest({
  revision: "1",
  break_lock: {
    target: "locked",
    last_attempt: { action: "enter", status: "succeeded" },
  },
});

describe("application recovery controls", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useSkipFlowStore.getState().resetSkipFlow();
    vi.mocked(listen).mockReset().mockResolvedValue(vi.fn());
    vi.mocked(invoke)
      .mockReset()
      .mockImplementation((command) =>
        Promise.resolve({
          status: "accepted",
          value:
            command === "get_full_state"
              ? createTimerSnapshotForTest()
              : { kind: "input_monitor" },
        }),
      );
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("keeps one connection notice across home, settings and a locked break", async () => {
    await act(async () => {
      render(<AppRoot />);
      await Promise.resolve();
    });
    act(() => {
      useTimerSyncStore.getState().setConnection({
        status: "failed",
        failure: { status: "unavailable", cause: new Error("disconnected") },
      });
    });
    expect(screen.getAllByTestId("connection-banner")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "設定を開く" }));
    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(screen.getAllByTestId("connection-banner")).toHaveLength(1);
    act(() => {
      useTimerObservationStore.getState().observeEffects(lockedEffects);
    });
    expect(
      screen.getByRole("button", { name: "休憩をスキップ" }),
    ).toBeInTheDocument();
    expect(screen.getAllByTestId("connection-banner")).toHaveLength(1);
  });

  it("keeps recovery feedback separate and does not revive it for a later failure", async () => {
    vi.useFakeTimers();
    await act(async () => {
      render(<AppRoot />);
      await Promise.resolve();
    });
    act(() => {
      useTimerObservationStore.getState().observeEffects(
        createTimerEffectsForTest({
          revision: "1",
          last_timer_save: "failed",
        }),
      );
    });
    vi.mocked(invoke).mockRejectedValue(new Error("response lost"));
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "再試行" }));
      await Promise.resolve();
    });
    expect(screen.getAllByRole("alert")).toHaveLength(2);
    const operationNotice = screen.getByTestId(
      "inline-error-retry_failed_timer_effects",
    );
    expect(operationNotice).toHaveTextContent("直前の復旧操作");
    expect(operationNotice).toHaveTextContent(
      "操作の結果を確認できませんでした",
    );
    expect(
      within(operationNotice).queryByRole("button"),
    ).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "再試行" })).toHaveLength(1);
    act(() => {
      useTimerObservationStore.getState().observeEffects(
        createTimerEffectsForTest({
          revision: "2",
          last_timer_save: "succeeded",
        }),
      );
    });
    expect(screen.getByRole("alert")).toHaveTextContent("直前の復旧操作");
    expect(
      screen.queryByRole("button", { name: "再試行" }),
    ).not.toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(5000);
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    act(() => {
      useTimerObservationStore.getState().observeEffects(
        createTimerEffectsForTest({
          revision: "3",
          break_lock: {
            target: "locked",
            last_attempt: { action: "enter", status: "failed" },
          },
          last_timer_save: "succeeded",
        }),
      );
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "画面のロックに失敗しました",
    );
    expect(screen.getByRole("alert")).not.toHaveTextContent(
      "操作の結果を確認できませんでした",
    );
    expect(
      useTimerOperationStore.getState().operations.retry_failed_timer_effects
        ?.outcome.status,
    ).toBe("unavailable");
  });

  it("can reconnect after lock success arrives before an unavailable initial snapshot", async () => {
    vi.useFakeTimers();
    let rejectSnapshot: (reason: Error) => void = () => {
      throw new Error("Expected a pending snapshot");
    };
    const snapshot = new Promise<never>((_, reject) => {
      rejectSnapshot = reject;
    });
    vi.mocked(invoke).mockImplementation((command) =>
      command === "get_full_state"
        ? snapshot
        : Promise.resolve({
            status: "accepted",
            value: { kind: "input_monitor" },
          }),
    );
    const callbacks = new Map<string, (event: unknown) => void>();
    vi.mocked(listen).mockImplementation((event, callback) => {
      callbacks.set(event, callback as (event: unknown) => void);
      return Promise.resolve(vi.fn());
    });
    await act(async () => {
      render(<AppRoot />);
      await Promise.resolve();
    });
    expect(invoke).toHaveBeenCalledWith("get_full_state");
    const notifyEffects = callbacks.get("timer_effects_changed");
    expect(notifyEffects).toBeDefined();
    act(() => {
      notifyEffects?.({ payload: lockedEffects });
    });
    expect(screen.getByText(/緊急解除/u)).toHaveTextContent(
      "Ctrl + Alt + Shift + E",
    );
    expect(screen.getByTestId("timer-display")).toHaveTextContent("--:--");
    await act(async () => {
      rejectSnapshot(new Error("response lost"));
      await Promise.resolve();
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(7000);
    });
    expect(screen.getByTestId("connection-banner-retry")).toBeEnabled();
    const recovered = createTimerSnapshotForTest({ effects: lockedEffects });
    recovered.timer.session_type = "short_break";
    recovered.timer.status = "on_break";
    recovered.timer.remaining_seconds = 300;
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: recovered,
    });
    await act(async () => {
      fireEvent.click(screen.getByTestId("connection-banner-retry"));
      await Promise.resolve();
    });
    expect(screen.queryByTestId("connection-banner")).not.toBeInTheDocument();
    expect(screen.getByTestId("timer-display")).toHaveTextContent("05:00");
    expect(
      screen.getByRole("button", { name: "休憩をスキップ" }),
    ).toBeInTheDocument();
  });
});
