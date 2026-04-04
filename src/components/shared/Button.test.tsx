import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it("renders a standard button with the default button type", () => {
    render(<Button>Save</Button>);

    const button = screen.getByRole("button", { name: "Save" });

    expect(button).toHaveAttribute("type", "button");
  });

  it("forwards the ref to the underlying button element", () => {
    const buttonRef = createRef<HTMLButtonElement>();

    render(<Button ref={buttonRef}>Save</Button>);

    expect(buttonRef.current).toBeInstanceOf(HTMLButtonElement);
    expect(buttonRef.current).toHaveTextContent("Save");
  });

  it("passes through standard button attributes", async () => {
    const user = userEvent.setup();
    const handleClick = vi.fn();

    render(
      <Button onClick={handleClick} disabled aria-label="Save settings">
        Save
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Save settings" });

    expect(button).toBeDisabled();
    expect(button).toHaveAccessibleName("Save settings");

    await user.click(button);

    expect(handleClick).not.toHaveBeenCalled();
  });

  it("preserves an explicit button type instead of forcing the default", () => {
    render(<Button type="submit">Save</Button>);

    expect(screen.getByRole("button", { name: "Save" })).toHaveAttribute(
      "type",
      "submit",
    );
  });
});
