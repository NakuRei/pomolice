import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useErrorStore, type AppError } from "@/store/errorStore";
import { InlineError } from "./InlineError";

function getActiveError(errorId: string): AppError {
  const activeError = useErrorStore.getState().errors[errorId];
  expect(activeError).toBeDefined();
  if (activeError === undefined) {
    throw new Error(`Expected active error for ${errorId}`);
  }

  return activeError;
}

describe("InlineError", () => {
  beforeEach(() => {
    useErrorStore.getState().clearAllErrors();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("renders nothing when the error does not exist", () => {
    render(<InlineError errorId="start_work" />);

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
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

    const retryButton = screen.getByRole("button", { name: "再試行" });

    expect(retryButton).toHaveTextContent("再試行");

    await user.click(retryButton);

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(getActiveError("start_work").retryCount).toBe(1);
  });

  it("absorbs stored retry rejections after recording the retry attempt", async () => {
    const user = userEvent.setup();
    const retryFn = vi.fn(() => Promise.reject(new Error("retry failed")));

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

    await user.click(screen.getByRole("button", { name: "再試行" }));
    await Promise.resolve();

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(getActiveError("start_work").retryCount).toBe(1);
  });

  it("uses the supplied retry action when the error has no stored retry function", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn(() => Promise.resolve());

    useErrorStore.getState().reportError({
      id: "emergency_stop",
      message: "停止に失敗しました。",
      technicalDetail: "rate_limited",
      severity: "critical",
      origin: "backend_command",
      canRetry: true,
      retryFn: null,
    });

    render(<InlineError errorId="emergency_stop" onRetry={onRetry} />);

    await user.click(screen.getByRole("button", { name: "再試行" }));

    expect(onRetry).toHaveBeenCalledTimes(1);
    expect(getActiveError("emergency_stop").retryCount).toBe(1);
  });

  it("absorbs supplied retry action rejections", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn(() => Promise.reject(new Error("retry failed")));

    useErrorStore.getState().reportError({
      id: "emergency_stop",
      message: "停止に失敗しました。",
      technicalDetail: "rate_limited",
      severity: "critical",
      origin: "backend_command",
      canRetry: true,
      retryFn: null,
    });

    render(<InlineError errorId="emergency_stop" onRetry={onRetry} />);

    await user.click(screen.getByRole("button", { name: "再試行" }));
    await Promise.resolve();

    expect(onRetry).toHaveBeenCalledTimes(1);
    expect(getActiveError("emergency_stop").retryCount).toBe(1);
  });

  it("does not render retry action when the error is not retryable", () => {
    useErrorStore.getState().reportError({
      id: "emergency_stop",
      message: "停止に失敗しました。",
      technicalDetail: "invalid_state_transition",
      severity: "critical",
      origin: "backend_command",
      canRetry: false,
      retryFn: null,
    });

    render(<InlineError errorId="emergency_stop" onRetry={() => undefined} />);

    expect(screen.queryByRole("button", { name: "再試行" })).toBeNull();
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

    expect(screen.getByRole("alert")).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(useErrorStore.getState().errors.start_work).toBeUndefined();
  });
});
