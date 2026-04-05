import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FullscreenBreakErrorFallback } from "./FullscreenBreakErrorFallback";

describe("FullscreenBreakErrorFallback", () => {
  it("renders the fullscreen fallback UI and exposes an alert region", () => {
    render(
      <FullscreenBreakErrorFallback
        error={new Error("boom")}
        resetErrorBoundary={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByText("表示エラーが発生しました")).toBeInTheDocument();
    expect(
      screen.getByText(
        "休憩タイマーはバックグラウンドで動作しています。休憩が終わると自動的に復帰します。",
      ),
    ).toBeInTheDocument();
  });

  it("calls resetErrorBoundary when retry is clicked", async () => {
    const user = userEvent.setup();
    const resetErrorBoundary = vi.fn();

    render(
      <FullscreenBreakErrorFallback
        error={new Error("boom")}
        resetErrorBoundary={resetErrorBoundary}
      />,
    );

    await user.click(screen.getByRole("button", { name: "再表示を試みる" }));

    expect(resetErrorBoundary).toHaveBeenCalledTimes(1);
  });
});
