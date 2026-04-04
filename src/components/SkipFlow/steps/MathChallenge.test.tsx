/**
 * Component tests for MathChallenge
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MathChallenge from "./MathChallenge";

const mockRetryStep = vi.fn();

// Mock the store
vi.mock("@/store/skipFlowStore", () => ({
  useSkipFlowStore: vi.fn(
    (selector: (state: { retryStep: typeof mockRetryStep }) => unknown) =>
      selector({
        retryStep: mockRetryStep,
      }),
  ),
}));

// Mock math generator
vi.mock("@/utils/mathGenerator", () => ({
  generateEasyProblem: vi.fn(() => ({
    question: "10 + 20 = ?",
    answer: 30,
  })),
  generateHardProblem: vi.fn(() => ({
    question: "5 × 6 = ?",
    answer: 30,
  })),
  validateAnswer: vi.fn((input: string | number, answer: number) => {
    const parsed = typeof input === "string" ? parseInt(input, 10) : input;
    return parsed === answer;
  }),
}));

describe("MathChallenge", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockRetryStep.mockReset();
  });

  describe("Rendering", () => {
    it("should render with title and instructions", () => {
      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      expect(screen.getByText("Math Challenge")).toBeInTheDocument();
      expect(screen.getByText(/Solve the following/iu)).toBeInTheDocument();
    });

    it("should display math problem", () => {
      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      // Should display a question mark
      expect(screen.getByText(/[=] \?/u)).toBeInTheDocument();
    });

    it("should render input and submit button", () => {
      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      expect(screen.getByPlaceholderText("Your answer")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: /Submit/iu }),
      ).toBeInTheDocument();
    });

    it("should render cancel button", () => {
      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      expect(
        screen.getByRole("button", { name: /Cancel and Take Break/iu }),
      ).toBeInTheDocument();
    });
  });

  describe("Input handling", () => {
    it("should allow numeric input", async () => {
      const user = userEvent.setup();

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      await user.type(input, "42");

      expect(input).toHaveValue("42");
    });

    it("should allow negative numbers", async () => {
      const user = userEvent.setup();

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      await user.type(input, "-5");

      expect(input).toHaveValue("-5");
    });

    it("should not allow non-numeric input", async () => {
      const user = userEvent.setup();

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      await user.type(input, "abc123");

      // Should only accept numeric part
      expect(input).toHaveValue("123");
    });
  });

  describe("Submit functionality", () => {
    it("should call onComplete when correct answer is submitted", async () => {
      const user = userEvent.setup();

      // Mock to generate predictable problem
      const { generateEasyProblem } = await import("@/utils/mathGenerator");
      vi.mocked(generateEasyProblem).mockReturnValue({
        question: "10 + 5 = ?",
        answer: 15,
      });

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      const submitButton = screen.getByRole("button", { name: /Submit/iu });

      await user.type(input, "15");
      await user.click(submitButton);

      await waitFor(() => {
        expect(mockOnComplete).toHaveBeenCalledTimes(1);
      });
    });
  });

  describe("Cancel functionality", () => {
    it("should call onCancel when cancel button is clicked", async () => {
      const user = userEvent.setup();

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const cancelButton = screen.getByRole("button", {
        name: /Cancel and Take Break/iu,
      });
      await user.click(cancelButton);

      expect(mockOnCancel).toHaveBeenCalledTimes(1);
    });
  });

  describe("Accessibility", () => {
    it("should have proper input type for numeric input", () => {
      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      expect(input).toHaveAttribute("inputMode", "numeric");
    });
  });
});
