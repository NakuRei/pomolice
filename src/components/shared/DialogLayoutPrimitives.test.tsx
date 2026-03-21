import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { DialogActionRow } from "./DialogActionRow";
import { DialogOverlay } from "./DialogOverlay";
import { DialogPanel } from "./DialogPanel";

describe("Dialog layout primitives", () => {
  it("renders the shared overlay shell and merges a custom class name", () => {
    render(
      <DialogOverlay testId="dialog-overlay" className="backdrop-blur-sm">
        <div>Dialog content</div>
      </DialogOverlay>,
    );

    const overlay = screen.getByTestId("dialog-overlay");

    expect(overlay).toHaveClass("backdrop-blur-sm");
    expect(overlay).toHaveTextContent("Dialog content");
  });

  it("renders an accessible dialog panel with optional linked ids", () => {
    render(
      <DialogPanel
        labelledBy="dialog-title"
        describedBy="dialog-description"
        testId="dialog-panel"
        className="max-w-lg"
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
    expect(panel).toHaveClass("max-w-lg");
  });

  it("renders the shared action row and merges a custom class name", () => {
    render(
      <DialogActionRow testId="dialog-action-row" className="justify-end gap-3">
        <button type="button">Cancel</button>
        <button type="button">Confirm</button>
      </DialogActionRow>,
    );

    const actionRow = screen.getByTestId("dialog-action-row");

    expect(actionRow).toHaveClass("justify-end", "gap-3");
    expect(screen.getByRole("button", { name: "Cancel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Confirm" })).toBeInTheDocument();
  });
});
