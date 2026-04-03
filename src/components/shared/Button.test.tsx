import { createRef } from "react";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./Button";

describe("Button", () => {
  it.each([
    {
      appearance: "primary",
      expectedClassNames: ["bg-primary-700", "text-on-primary"],
    },
    {
      appearance: "secondary",
      expectedClassNames: [
        "border-border",
        "bg-surface-raised",
        "text-on-surface",
        "hover:bg-surface-hover",
      ],
    },
    {
      appearance: "danger",
      expectedClassNames: ["bg-danger-600", "text-on-danger"],
    },
    {
      appearance: "subtle",
      expectedClassNames: ["bg-transparent", "text-text-muted"],
    },
  ] as const)(
    "applies the %s appearance token classes",
    ({ appearance, expectedClassNames }) => {
      render(<Button appearance={appearance}>Action</Button>);

      const button = screen.getByRole("button", { name: "Action" });

      expectedClassNames.forEach((expectedClassName) => {
        expect(button).toHaveClass(expectedClassName);
      });
    },
  );

  it("renders a standard button with the default type and default styling tokens", () => {
    render(<Button>Save</Button>);

    const button = screen.getByRole("button", { name: "Save" });

    expect(button).toHaveAttribute("type", "button");
    expect(button).toHaveClass("border");
    expect(button).toHaveClass("border-border");
    expect(button).toHaveClass("bg-surface-raised");
    expect(button).toHaveClass("text-on-surface");
    expect(button).toHaveClass("px-6");
    expect(button).toHaveClass("py-3");
    expect(button).toHaveClass("text-base");
  });

  it("applies the requested appearance, size, width, and custom layout classes", () => {
    render(
      <Button appearance="danger" size="lg" width="full" className="flex-1">
        Delete
      </Button>,
    );

    const button = screen.getByRole("button", { name: "Delete" });

    expect(button).toHaveClass("bg-danger-600");
    expect(button).toHaveClass("px-8");
    expect(button).toHaveClass("py-4");
    expect(button).toHaveClass("text-lg");
    expect(button).toHaveClass("w-full");
    expect(button).toHaveClass("flex-1");
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
