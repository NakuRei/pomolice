import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { StatusNotice } from "./StatusNotice";

describe("StatusNotice", () => {
  it("renders an explicit status role and polite live region when requested", () => {
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
    expect(notice).toHaveTextContent("設定を保存しました。");
  });

  it("renders a leading icon and trailing actions slot", () => {
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

    expect(notice).toHaveTextContent("接続に失敗しました。");
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
});
