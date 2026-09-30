import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { InfoMessageBanner } from "./InfoMessageBanner";
import { useTimerStore } from "@/store/timerStore";

describe("InfoMessageBanner", () => {
  beforeEach(() => {
    useTimerStore.setState({
      status: "idle",
      remainingSeconds: 25 * 60,
      sessionType: "work",
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
      isConnected: false,
      bootAutoStartOffer: null,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not render when there is no info message", () => {
    render(<InfoMessageBanner />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders current info message", () => {
    useTimerStore.setState({
      infoMessage: {
        id: "auto_run_stop_pending",
        message: "現在セッション終了後に停止予定",
        ttlMs: 5000,
      },
    });

    render(<InfoMessageBanner />);

    const notice = screen.getByRole("status");

    expect(notice).toHaveAttribute("aria-live", "polite");
    expect(
      screen.getByText("現在セッション終了後に停止予定"),
    ).toBeInTheDocument();
  });

  it("clears the message after ttl", () => {
    vi.useFakeTimers();
    useTimerStore.setState({
      infoMessage: {
        id: "auto_run_stopped",
        message: "自動停止しました",
        ttlMs: 1000,
      },
    });

    render(<InfoMessageBanner />);
    expect(screen.getByRole("status")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(useTimerStore.getState().infoMessage).toBeNull();
  });
});
