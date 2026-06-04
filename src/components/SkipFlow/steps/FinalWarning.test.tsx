import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import FinalWarning from "./FinalWarning";

describe("FinalWarning", () => {
  const mockOnConfirm = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows the final warning and focuses the take-break action", async () => {
    render(<FinalWarning onConfirm={mockOnConfirm} onCancel={mockOnCancel} />);

    expect(screen.getByText("Final Warning")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Are you absolutely certain you want to skip this break?",
      ),
    ).toBeInTheDocument();

    await waitFor(() => {
      expect(
        screen.getByRole("button", { name: "No, Take the Break" }),
      ).toHaveFocus();
    });
  });

  it("notifies when the user confirms the skip", async () => {
    const user = userEvent.setup();
    render(<FinalWarning onConfirm={mockOnConfirm} onCancel={mockOnCancel} />);

    await user.click(screen.getByRole("button", { name: "Yes, Skip Anyway" }));

    expect(mockOnConfirm).toHaveBeenCalledTimes(1);
  });

  it("notifies when the user chooses to take the break", async () => {
    const user = userEvent.setup();
    render(<FinalWarning onConfirm={mockOnConfirm} onCancel={mockOnCancel} />);

    await user.click(
      screen.getByRole("button", { name: "No, Take the Break" }),
    );

    expect(mockOnCancel).toHaveBeenCalledTimes(1);
  });
});
