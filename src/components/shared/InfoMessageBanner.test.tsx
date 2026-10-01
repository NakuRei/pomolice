import { useTimerNoticeStore } from "@/store/timerNoticeStore";
import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { InfoMessageBanner } from "./InfoMessageBanner";

describe("InfoMessageBanner", () => {
  beforeEach(() => {
    useTimerNoticeStore.setState(useTimerNoticeStore.getInitialState(), true);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("does not render when there is no info message", () => {
    render(<InfoMessageBanner />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("renders current info message", () => {
    useTimerNoticeStore.setState({
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
    useTimerNoticeStore.setState({
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
    expect(useTimerNoticeStore.getState().infoMessage).toBeNull();
  });
});
