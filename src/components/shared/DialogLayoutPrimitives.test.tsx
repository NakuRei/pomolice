import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DialogPanel } from "./DialogPanel";

describe("Dialog layout primitives", () => {
  it("renders an accessible dialog panel with optional linked ids", () => {
    render(
      <DialogPanel
        labelledBy="dialog-title"
        describedBy="dialog-description"
        testId="dialog-panel"
      >
        <h2 id="dialog-title">Dialog title</h2>
        <p id="dialog-description">Dialog description</p>
      </DialogPanel>,
    );

    const panel = screen.getByRole("dialog");

    expect(panel).toBe(screen.getByTestId("dialog-panel"));
    expect(panel).toHaveAttribute("aria-modal", "true");
    expect(panel).toHaveAttribute("aria-labelledby", "dialog-title");
    expect(panel).toHaveAttribute("aria-describedby", "dialog-description");
  });
});
