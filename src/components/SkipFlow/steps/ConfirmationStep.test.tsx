import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ConfirmationStep from "./ConfirmationStep";

describe("ConfirmationStep", () => {
  const mockOnConfirm = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the selected warning and focuses the take-break action", async () => {
    render(
      <ConfirmationStep
        step={1}
        onConfirm={mockOnConfirm}
        onCancel={mockOnCancel}
      />,
    );

    expect(
      screen.getByText("Are you sure you want to skip this break?"),
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "No, Take Break" }),
      ).toHaveFocus();
    });
  });

  it("updates the visible warning when the step changes", () => {
    const { rerender } = render(
      <ConfirmationStep
        step={1}
        onConfirm={mockOnConfirm}
        onCancel={mockOnCancel}
      />,
    );

    rerender(
      <ConfirmationStep
        step={2}
        onConfirm={mockOnConfirm}
        onCancel={mockOnCancel}
      />,
    );

    expect(
      screen.getByText(
        "Breaks are important for your health. Do you still want to skip?",
      ),
    ).toBeInTheDocument();
  });

  it("notifies when the user chooses to continue", async () => {
    const user = userEvent.setup();
    render(
      <ConfirmationStep
        step={1}
        onConfirm={mockOnConfirm}
        onCancel={mockOnCancel}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Yes, Continue" }));

    expect(mockOnConfirm).toHaveBeenCalledTimes(1);
  });

  it("notifies when the user chooses to take the break", async () => {
    const user = userEvent.setup();
    render(
      <ConfirmationStep
        step={1}
        onConfirm={mockOnConfirm}
        onCancel={mockOnCancel}
      />,
    );

    await user.click(screen.getByRole("button", { name: "No, Take Break" }));

    expect(mockOnCancel).toHaveBeenCalledTimes(1);
  });
});
