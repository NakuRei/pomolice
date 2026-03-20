import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ToggleIconButton } from "../ToggleIconButton";

describe("ToggleIconButton", () => {
  it("renders a stable accessible name and exposes pressed state", () => {
    render(
      <ToggleIconButton
        label="通知音"
        pressed={false}
        onPressedChange={() => undefined}
      >
        <span data-testid="toggle-icon-off">off</span>
      </ToggleIconButton>,
    );

    const button = screen.getByRole("button", { name: "通知音" });

    expect(button).toHaveAccessibleName("通知音");
    expect(button).toHaveAttribute("aria-pressed", "false");
    expect(screen.getByTestId("toggle-icon-off")).toBeInTheDocument();
  });

  it("renders the pressed icon and reports the next pressed state on click", async () => {
    const user = userEvent.setup();
    const handlePressedChange = vi.fn();

    render(
      <ToggleIconButton
        label="通知音"
        pressed
        onPressedChange={handlePressedChange}
        pressedAppearance="danger"
        pressedChildren={<span data-testid="toggle-icon-on">on</span>}
      >
        <span data-testid="toggle-icon-off">off</span>
      </ToggleIconButton>,
    );

    const button = screen.getByRole("button", { name: "通知音" });

    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button).toHaveClass("text-danger-500");
    expect(screen.getByTestId("toggle-icon-on")).toBeInTheDocument();

    await user.click(button);

    expect(handlePressedChange).toHaveBeenCalledWith(false);
  });
});
