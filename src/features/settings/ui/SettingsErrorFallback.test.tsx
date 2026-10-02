import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsErrorFallback } from "./SettingsErrorFallback";

describe("SettingsErrorFallback", () => {
  it("renders the settings fallback UI", () => {
    render(
      <SettingsErrorFallback
        error={new Error("boom")}
        onBackToHome={() => undefined}
        resetErrorBoundary={() => undefined}
      />,
    );

    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(
      screen.getByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();
  });

  it("resets and returns home when the home button is clicked", async () => {
    const user = userEvent.setup();
    const onBackToHome = vi.fn();
    const resetErrorBoundary = vi.fn();

    render(
      <SettingsErrorFallback
        error={new Error("boom")}
        onBackToHome={onBackToHome}
        resetErrorBoundary={resetErrorBoundary}
      />,
    );

    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(resetErrorBoundary).toHaveBeenCalledTimes(1);
    expect(onBackToHome).toHaveBeenCalledTimes(1);
  });

  it("calls resetErrorBoundary when retry is clicked", async () => {
    const user = userEvent.setup();
    const onBackToHome = vi.fn();
    const resetErrorBoundary = vi.fn();

    render(
      <SettingsErrorFallback
        error={new Error("boom")}
        onBackToHome={onBackToHome}
        resetErrorBoundary={resetErrorBoundary}
      />,
    );

    await user.click(screen.getByRole("button", { name: "再表示を試みる" }));

    expect(resetErrorBoundary).toHaveBeenCalledTimes(1);
    expect(onBackToHome).not.toHaveBeenCalled();
  });
});
