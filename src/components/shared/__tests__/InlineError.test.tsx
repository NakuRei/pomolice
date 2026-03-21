import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useErrorStore } from "@/store/errorStore";
import { InlineError } from "../InlineError";

describe("InlineError", () => {
  beforeEach(() => {
    useErrorStore.getState().clearAllErrors();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing when the error does not exist", () => {
    render(<InlineError errorId="start_work" />);

    expect(
      screen.queryByTestId("inline-error-start_work"),
    ).not.toBeInTheDocument();
  });

  it("renders alert notice with the current error message", () => {
    useErrorStore.getState().reportError({
      id: "start_work",
      message: "開始に失敗しました。",
      technicalDetail: "backend timeout",
      severity: "critical",
      origin: "backend_command",
      canRetry: false,
      retryFn: null,
    });

    render(<InlineError errorId="start_work" />);

    const notice = screen.getByRole("alert");

    expect(notice).toBe(screen.getByTestId("inline-error-start_work"));
    expect(notice).toHaveTextContent("開始に失敗しました。");
  });

  it("renders retry action when retry is available", async () => {
    const user = userEvent.setup();
    const retryFn = vi.fn(() => Promise.resolve());

    useErrorStore.getState().reportError({
      id: "start_work",
      message: "開始に失敗しました。",
      technicalDetail: "backend timeout",
      severity: "critical",
      origin: "backend_command",
      canRetry: true,
      retryFn,
    });

    render(<InlineError errorId="start_work" />);

    const retryButton = screen.getByTestId("inline-error-retry-start_work");

    expect(retryButton).toHaveTextContent("再試行");

    await user.click(retryButton);

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(useErrorStore.getState().errors.start_work.retryCount).toBe(1);
  });

  it("auto-clears recoverable errors after timeout", () => {
    vi.useFakeTimers();

    useErrorStore.getState().reportError({
      id: "start_work",
      message: "開始に失敗しました。",
      technicalDetail: "backend timeout",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: false,
      retryFn: null,
    });

    render(<InlineError errorId="start_work" />);

    expect(screen.getByTestId("inline-error-start_work")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(
      screen.queryByTestId("inline-error-start_work"),
    ).not.toBeInTheDocument();
    expect(useErrorStore.getState().errors.start_work).toBeUndefined();
  });
});
