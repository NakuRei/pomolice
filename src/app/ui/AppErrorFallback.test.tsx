import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { AppErrorFallback } from "./AppErrorFallback";

describe("AppErrorFallback", () => {
  it("renders the app-level fallback UI", () => {
    const resetErrorBoundary = vi.fn();

    render(
      <AppErrorFallback
        error={new Error("internal failure")}
        resetErrorBoundary={resetErrorBoundary}
      />,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByRole("heading", {
        name: "アプリケーションエラーが発生しました",
      }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "画面の再表示を試みてください。復帰しない場合はアプリを再起動してください。",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "再表示を試みる" }),
    ).toBeInTheDocument();
    expect(resetErrorBoundary).not.toHaveBeenCalled();
  });

  it("does not expose raw error details", () => {
    render(
      <AppErrorFallback
        error={new Error("internal failure")}
        resetErrorBoundary={vi.fn()}
      />,
    );

    expect(screen.queryByText("internal failure")).not.toBeInTheDocument();
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
