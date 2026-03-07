import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { NotificationMuteToggleButton } from "../NotificationMuteToggleButton";

describe("NotificationMuteToggleButton", () => {
  it("renders the unmuted state and calls onToggle when clicked", async () => {
    const user = userEvent.setup();
    const handleToggle = vi.fn();

    render(
      <NotificationMuteToggleButton
        muted={false}
        onToggle={handleToggle}
        testId="notification-mute-toggle-button"
        iconClassName="h-6 w-6"
      />,
    );

    const button = screen.getByTestId("notification-mute-toggle-button");

    expect(button).toHaveAccessibleName("通知音をミュートする");

    await user.click(button);

    expect(handleToggle).toHaveBeenCalledTimes(1);
  });

  it("renders the muted state semantics", () => {
    render(
      <NotificationMuteToggleButton
        muted
        onToggle={() => undefined}
        testId="notification-mute-toggle-button"
      />,
    );

    const button = screen.getByTestId("notification-mute-toggle-button");

    expect(button).toHaveAccessibleName("通知音をオンにする");
  });
});
