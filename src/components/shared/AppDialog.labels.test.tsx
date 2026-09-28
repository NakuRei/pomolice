import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { AppDialog } from "./AppDialog";

describe("AppDialog accessible labels", () => {
  it("exposes an accessible name when labelledBy is provided", () => {
    render(
      <AppDialog labelledBy="dialog-title">
        <h2 id="dialog-title">Dialog title</h2>
      </AppDialog>,
    );

    const panel = screen.getByRole("dialog", { name: "Dialog title" });

    expect(panel).toHaveAccessibleName("Dialog title");
  });

  it("exposes an accessible description when describedBy is provided", () => {
    render(
      <AppDialog labelledBy="dialog-title" describedBy="dialog-description">
        <h2 id="dialog-title">Dialog title</h2>
        <p id="dialog-description">Dialog description</p>
      </AppDialog>,
    );

    const panel = screen.getByRole("dialog", {
      name: "Dialog title",
      description: "Dialog description",
    });

    expect(panel).toHaveAccessibleDescription("Dialog description");
  });
});
