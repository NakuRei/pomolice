import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { TimerEffectsNotice } from "./TimerEffectsNotice";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { useTimerStore } from "@/store/timerStore";
import { TimerStatus } from "@/types/timer";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { useTimerEffectsStore } from "@/store/timerEffectsStore";
import {
  createTimerEffectsForTest,
  createTimerSnapshotForTest,
} from "@/test/timerEffectsFixture";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("timer effect failures", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    resetTimerStoreForTest();
    useTimerEffectsStore.setState(useTimerEffectsStore.getInitialState(), true);
  });

  it("shows save and unlock failures together and clears them only after recovery", async () => {
    useTimerEffectsStore.getState().observe(
      createTimerEffectsForTest({
        revision: 1,
        break_lock: {
          target: "unlocked",
          last_attempt: { action: "release", status: "failed" },
        },
        last_timer_save: "failed",
      }),
    );
    vi.mocked(invoke).mockResolvedValue(
      createTimerSnapshotForTest({
        effects: createTimerEffectsForTest({
          revision: 2,
          break_lock: {
            target: "unlocked",
            last_attempt: { action: "release", status: "succeeded" },
          },
          last_timer_save: "succeeded",
        }),
      }),
    );
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
      useTimerEffectsStore.getState().observe(
        createTimerEffectsForTest({
          revision: 1,
          last_timer_save: "failed",
        }),
      );
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

  it("keeps a save failure visible after an accepted emergency stop closes its dialog", async () => {
    useTimerStore.setState({ status: TimerStatus.WORKING });
    vi.mocked(invoke).mockImplementationOnce(() => {
      useTimerStore.setState({ status: TimerStatus.IDLE });
      return Promise.resolve(
        createTimerSnapshotForTest({
          effects: createTimerEffectsForTest({
            revision: 1,
            last_timer_save: "failed",
          }),
        }),
      );
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
