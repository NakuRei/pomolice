import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusNotice } from "./StatusNotice";

describe("StatusNotice", () => {
  it("renders info tone classes with an explicit status role and polite live region", () => {
    render(
      <StatusNotice
        tone="info"
        role="status"
        ariaLive="polite"
        testId="status-notice"
      >
        設定を保存しました。
      </StatusNotice>,
    );

    const notice = screen.getByRole("status");

    expect(notice).toBe(screen.getByTestId("status-notice"));
    expect(notice).toHaveAttribute("aria-live", "polite");
    expect(notice).toHaveClass(
      "rounded-md",
      "border",
      "border-info-800",
      "bg-info-950/30",
      "text-info-100",
      "px-4",
      "py-3",
      "text-sm",
    );
    expect(notice).toHaveTextContent("設定を保存しました。");
  });

  it("renders danger tone classes with a leading icon and trailing actions slot", () => {
    render(
      <StatusNotice
        tone="danger"
        icon={<span data-testid="status-notice-icon">!</span>}
        actions={<button type="button">再試行</button>}
        testId="status-notice"
      >
        接続に失敗しました。
      </StatusNotice>,
    );

    const notice = screen.getByTestId("status-notice");

    expect(notice).toHaveClass(
      "border-danger-800",
      "bg-danger-950/30",
      "text-danger-200",
    );
    expect(screen.getByTestId("status-notice-icon")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "再試行" })).toBeInTheDocument();
  });

  it("leaves role and aria-live unset when not provided", () => {
    render(
      <StatusNotice tone="warning" testId="status-notice">
        まだ保存していません。
      </StatusNotice>,
    );

    const notice = screen.getByTestId("status-notice");

    expect(notice).not.toHaveAttribute("role");
    expect(notice).not.toHaveAttribute("aria-live");
  });

  it("merges caller wrapper className without losing base tone classes", () => {
    render(
      <StatusNotice tone="success" className="mb-4" testId="status-notice">
        同期が完了しました。
      </StatusNotice>,
    );

    const notice = screen.getByTestId("status-notice");

    expect(notice).toHaveClass(
      "mb-4",
      "px-4",
      "py-3",
      "border-success-800",
      "bg-success-950/30",
      "text-success-200",
    );
  });
});
