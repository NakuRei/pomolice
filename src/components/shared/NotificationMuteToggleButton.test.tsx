import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { NotificationMuteToggleButton } from "./NotificationMuteToggleButton";

describe("NotificationMuteToggleButton", () => {
  it.each([
    {
      muted: false,
      pressed: "false",
      title: "通知音オン",
    },
    {
      muted: true,
      pressed: "true",
      title: "通知音ミュート中",
    },
  ])(
    "maps muted=$muted to stable button semantics",
    ({ muted, pressed, title }) => {
      render(
        <NotificationMuteToggleButton
          muted={muted}
          onToggle={() => undefined}
          testId="notification-mute-toggle-button"
        />,
      );

      const button = screen.getByRole("button", { name: "通知音" });

      expect(button).toHaveAccessibleName("通知音");
      expect(button).toHaveAttribute("aria-pressed", pressed);
      expect(button).toHaveAttribute("title", title);
    },
  );
});
