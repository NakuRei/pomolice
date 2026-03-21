import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useErrorStore } from "@/store/errorStore";
import { ConnectionBanner } from "../ConnectionBanner";

describe("ConnectionBanner", () => {
  beforeEach(() => {
    useErrorStore.getState().clearAllErrors();
  });

  it("renders nothing without a connection error", () => {
    render(<ConnectionBanner />);

    expect(screen.queryByTestId("connection-banner")).not.toBeInTheDocument();
  });

  it("renders connection error banner with alert semantics", () => {
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

    const banner = screen.getByRole("alert");

    expect(banner).toBe(screen.getByTestId("connection-banner"));
    expect(banner).toHaveTextContent(
      "バックエンドに接続できません。アプリが正常に動作しない可能性があります。",
    );
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

    const { rerender } = render(<ConnectionBanner />);

    expect(
      screen.queryByTestId("connection-banner-retry"),
    ).not.toBeInTheDocument();

    useErrorStore.getState().reportError({
      id: "connection",
      message: "バックエンドに接続できません",
      technicalDetail: "setup failed",
      severity: "critical",
      origin: "backend_connection",
      canRetry: true,
      retryFn,
    });

    rerender(<ConnectionBanner />);

    expect(screen.getByTestId("connection-banner-retry")).toBeInTheDocument();
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

    await user.click(screen.getByTestId("connection-banner-retry"));

    expect(retryFn).toHaveBeenCalledTimes(1);
    expect(screen.getByTestId("connection-banner-retry")).toBeDisabled();
    expect(screen.getByText("接続を試みています...")).toBeInTheDocument();

    resolveRetry();

    await waitFor(() => {
      expect(screen.getByTestId("connection-banner-retry")).not.toBeDisabled();
    });
  });
});
