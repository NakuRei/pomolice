import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { IconButton } from "../IconButton";

describe("IconButton", () => {
  it("renders an icon-only button with a stable accessible name", async () => {
    const user = userEvent.setup();
    const handleClick = vi.fn();

    render(
      <IconButton label="設定" onClick={handleClick}>
        <span data-testid="icon-button-glyph">gear</span>
      </IconButton>,
    );

    const button = screen.getByRole("button", { name: "設定" });

    expect(button).toHaveAccessibleName("設定");
    expect(button).toHaveAttribute("type", "button");

    await user.click(button);

    expect(handleClick).toHaveBeenCalledTimes(1);
  });

  it("passes through disabled and appearance styling", () => {
    render(
      <IconButton label="削除" appearance="danger" disabled>
        <span>trash</span>
      </IconButton>,
    );

    const button = screen.getByRole("button", { name: "削除" });

    expect(button).toBeDisabled();
    expect(button).toHaveClass("text-red-600");
  });
});
