import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useErrorStore } from "@/store/errorStore";
import { ConnectionBanner } from "./ConnectionBanner";

describe("ConnectionBanner", () => {
  beforeEach(() => {
    useErrorStore.getState().clearAllErrors();
  });

  it("renders nothing without a connection error", () => {
    render(<ConnectionBanner />);

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders connection error banner with alert semantics", () => {
    useErrorStore.getState().reportError({
      id: "connection",
      message: "入力内容を確認してください",
      technicalDetail: "setup failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: false,
      retryFn: null,
    });

    render(<ConnectionBanner />);

    const banner = screen.getByRole("alert");

    expect(banner).toHaveTextContent("入力内容を確認してください");
  });

  it("shows retry button only when canRetry is true", () => {
    const retryFn = vi.fn(() => Promise.resolve());

    useErrorStore.getState().reportError({
      id: "connection",
      message: "バックエンドに接続できません",
      technicalDetail: "setup failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: false,
      retryFn: null,
    });

    render(<ConnectionBanner />);

    expect(
      screen.queryByRole("button", { name: "再試行" }),
    ).not.toBeInTheDocument();

    act(() => {
      useErrorStore.getState().reportError({
        id: "connection",
        message: "バックエンドに接続できません",
        technicalDetail: "setup failed",
        severity: "critical",
        origin: "backend_connection",
        canRetry: true,
        retryFn,
      });
    });

    expect(screen.getByRole("button", { name: "再試行" })).toBeInTheDocument();
  });

  it("shows retrying state while retryFn is pending", async () => {
    const user = userEvent.setup();
    let resolveRetry: () => void = () => {
      throw new Error("Retry promise resolver was not initialized");
    };
    const retryPromise = new Promise<void>((resolve) => {
      resolveRetry = resolve;
    });
    const retryFn = vi.fn(() => retryPromise);

    useErrorStore.getState().reportError({
      id: "connection",
      message: "バックエンドに接続できません",
      technicalDetail: "setup failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
      retryFn,
    });

    render(<ConnectionBanner />);

    const retryButton = screen.getByRole("button", { name: "再試行" });

    await user.click(retryButton);

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(retryButton).toBeDisabled();
    expect(screen.getByText("接続を試みています...")).toBeInTheDocument();

    resolveRetry();

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "再試行" })).not.toBeDisabled();
    });
  });

  it("returns to retryable state when retryFn rejects", async () => {
    const user = userEvent.setup();
    let rejectRetry: (error: Error) => void = () => {
      throw new Error("Retry promise rejector was not initialized");
    };
    const retryPromise = new Promise<void>((_resolve, reject) => {
      rejectRetry = reject;
    });
    const retryFn = vi.fn(() => retryPromise);

    useErrorStore.getState().reportError({
      id: "connection",
      message: "バックエンドに接続できません",
      technicalDetail: "setup failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
      retryFn,
    });

    render(<ConnectionBanner />);

    const retryButton = screen.getByRole("button", { name: "再試行" });

    await user.click(retryButton);

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(retryButton).toBeDisabled();

    rejectRetry(new Error("retry failed"));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "再試行" })).not.toBeDisabled();
    });
  });
});
