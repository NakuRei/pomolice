import { useBackendObservationStore } from "@/backend/observationStore";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { TimerEffectsNotice } from "./TimerEffectsNotice";
import { EmergencyStopButton } from "@/features/timer/ui/EmergencyStopButton";

import { resetAppStateForTest } from "@/test/resetAppState";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer effect failures", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    resetAppStateForTest();
    useBackendObservationStore.setState(
      useBackendObservationStore.getInitialState(),
      true,
    );
  });

  it("shows save and unlock failures together and clears them only after recovery", async () => {
    useBackendObservationStore.setState({
      effects: createTimerEffectsForTest({
        break_lock: {
          target: "unlocked",
          last_attempt: { action: "release", status: "failed" },
        },
        last_timer_save: "failed",
      }),
    });
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          break_lock: {
            target: "unlocked",
            last_attempt: { action: "release", status: "succeeded" },
          },
          last_timer_save: "succeeded",
        }),
      }),
    });
    render(<TimerEffectsNotice />);
    expect(screen.getByRole("alert")).toHaveTextContent(
      "画面のロック解除に失敗しました",
    );
    expect(screen.getByRole("alert")).toHaveTextContent(
      "タイマーの状態の保存に失敗しました",
    );
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "再試行" }));
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("keeps a failure visible until a successful effect is observed", () => {
    vi.useFakeTimers();
    try {
      useBackendObservationStore.setState({
        effects: createTimerEffectsForTest({
          last_timer_save: "failed",
        }),
      });
      render(<TimerEffectsNotice />);
      act(() => {
        vi.advanceTimersByTime(60_000);
      });
      expect(screen.getByRole("alert")).toHaveTextContent(
        "タイマーの状態の保存に失敗しました",
      );
    } finally {
      vi.useRealTimers();
    }
  });

  it("disables duplicate recovery while pending and keeps failures after acceptance", async () => {
    const effects = createTimerEffectsForTest({
      last_timer_save: "failed",
    });
    useBackendObservationStore.setState({ effects: effects });
    let finishRetry: (response: unknown) => void = () => {
      throw new Error("Expected a pending recovery request");
    };
    vi.mocked(invoke).mockImplementation(
      () =>
        new Promise((resolve) => {
          finishRetry = resolve;
        }),
    );
    render(<TimerEffectsNotice />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "再試行" }));
    expect(screen.getByRole("button", { name: "再試行中…" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "再試行中…" }));
    expect(invoke).toHaveBeenCalledExactlyOnceWith(
      "retry_failed_timer_effects",
    );
    await act(async () => {
      finishRetry({
        status: "accepted",
        value: createTimerSnapshotForTest({ effects }),
      });
      await Promise.resolve();
    });
    expect(screen.getByRole("alert")).toHaveTextContent(
      "タイマーの状態の保存に失敗しました",
    );
    expect(screen.getByRole("button", { name: "再試行" })).toBeEnabled();
  });

  it("keeps a save failure visible after an accepted emergency stop closes its dialog", async () => {
    useBackendObservationStore.setState({ status: "working" });
    vi.mocked(invoke).mockImplementationOnce(() => {
      useBackendObservationStore.setState({ status: "idle" });
      return Promise.resolve({
        status: "accepted",
        value: createTimerSnapshotForTest({
          effects: createTimerEffectsForTest({
            last_timer_save: "failed",
          }),
        }),
      });
    });
    render(
      <>
        <TimerEffectsNotice />
        <EmergencyStopButton />
      </>,
    );
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "緊急停止" }));
    await user.click(screen.getByRole("button", { name: "停止する" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      "操作は反映されています",
    );
    expect(invoke).toHaveBeenCalledExactlyOnceWith("emergency_stop");
  });
});
