import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SettingsGearIcon } from "../../icons";
import { ActionIconButton } from "../ActionIconButton";

describe("ActionIconButton", () => {
  it("renders an icon-only action button with the provided accessible name", async () => {
    const user = userEvent.setup();
    const handleClick = vi.fn();

    render(
      <ActionIconButton
        label="設定を開く"
        testId="action-icon-button"
        onClick={handleClick}
      >
        <SettingsGearIcon className="h-6 w-6" />
      </ActionIconButton>,
    );

    const button = screen.getByTestId("action-icon-button");

    expect(button).toHaveAccessibleName("設定を開く");

    await user.click(button);

    expect(handleClick).toHaveBeenCalledTimes(1);
  });
});
