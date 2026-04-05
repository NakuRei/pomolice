import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppErrorFallback } from "./AppErrorFallback";

describe("AppErrorFallback", () => {
  it("renders the app-level fallback UI", () => {
    render(
      <AppErrorFallback
        error={new Error("boom")}
        resetErrorBoundary={vi.fn()}
      />,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText("アプリケーションエラーが発生しました"),
    ).toBeInTheDocument();
  });

  it("uses inline styles instead of Tailwind classes", () => {
    render(
      <AppErrorFallback
        error={new Error("boom")}
        resetErrorBoundary={vi.fn()}
      />,
    );

    const alert = screen.getByRole("alert");
    const container = alert.parentElement;

    expect(container).not.toBeNull();
    expect(container).toHaveAttribute("style");
    expect(container).not.toHaveAttribute("class");
  });

  it("calls resetErrorBoundary when retry is clicked", async () => {
    const user = userEvent.setup();
    const resetErrorBoundary = vi.fn();

    render(
      <AppErrorFallback
        error={new Error("boom")}
        resetErrorBoundary={resetErrorBoundary}
      />,
    );

    await user.click(screen.getByRole("button", { name: "再表示を試みる" }));

    expect(resetErrorBoundary).toHaveBeenCalledTimes(1);
  });
});
