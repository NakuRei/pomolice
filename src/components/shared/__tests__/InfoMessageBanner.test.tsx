import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { InfoMessageBanner } from "../InfoMessageBanner";
import { SessionType, TimerStatus, useTimerStore } from "../../../store/timerStore";

describe("InfoMessageBanner", () => {
  beforeEach(() => {
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      isFullscreenBreak: false,
      showContinueDialog: false,
      isConnected: false,
      shouldPromptStartOnBoot: false,
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
    expect(screen.queryByTestId("info-message-banner")).not.toBeInTheDocument();
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

    expect(screen.getByTestId("info-message-banner")).toBeInTheDocument();
    expect(screen.getByText("現在セッション終了後に停止予定")).toBeInTheDocument();
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
    expect(screen.getByTestId("info-message-banner")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(screen.queryByTestId("info-message-banner")).not.toBeInTheDocument();
    expect(useTimerStore.getState().infoMessage).toBeNull();
  });
});
