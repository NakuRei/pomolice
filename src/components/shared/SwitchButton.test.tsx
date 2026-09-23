import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { SwitchButton } from "./SwitchButton";

describe("SwitchButton", () => {
  it("is exposed as a named switch with its checked state", () => {
    render(
      <SwitchButton
        aria-label="曜日別自動実行"
        checked={false}
        onCheckedChange={() => undefined}
      />,
    );

    expect(
      screen.getByRole("switch", { name: "曜日別自動実行" }),
    ).not.toBeChecked();
  });

  it("reports the next checked state on click", async () => {
    const user = userEvent.setup();
    const handleCheckedChange = vi.fn();

    render(
      <SwitchButton
        aria-label="曜日別自動実行"
        checked
        onCheckedChange={handleCheckedChange}
      />,
    );

    const switchButton = screen.getByRole("switch");
    expect(switchButton).toBeChecked();

    await user.click(switchButton);

    expect(handleCheckedChange).toHaveBeenCalledWith(false);
  });
});
