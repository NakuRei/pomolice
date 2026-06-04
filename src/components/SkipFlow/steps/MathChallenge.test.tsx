/**
 * Component tests for MathChallenge
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import MathChallenge from "./MathChallenge";
import { useSkipFlowStore } from "@/store/skipFlowStore";

/**
 * Solves the math problem currently displayed on screen by parsing the
 * rendered question. Driving input from the visible problem keeps the test
 * independent of how problems are generated.
 */
function solveDisplayedProblem(): number {
  const text = screen.getByText(/[=]\s*\?/u).textContent;
  const groups = /(?<left>-?\d+)\s*(?<operator>[+\-×])\s*(?<right>-?\d+)/u.exec(
    text,
  )?.groups;

  if (groups?.operator === undefined) {
    throw new Error(`could not parse displayed problem: "${text}"`);
  }

  const left = Number(groups.left);
  const right = Number(groups.right);

  switch (groups.operator) {
    case "+":
      return left + right;
    case "×":
      return left * right;
    case "-":
      return left - right;
    default:
      throw new Error(`unexpected operator in problem: "${text}"`);
  }
}

function formElementOf(input: HTMLElement): HTMLFormElement {
  const form = input.closest("form");

  if (form === null) {
    throw new Error("answer input is not inside a form");
  }

  return form;
}

describe("MathChallenge", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useSkipFlowStore.getState().resetSkipFlow();
  });

  afterEach(() => {
    if (vi.isFakeTimers()) {
      act(() => {
        vi.runOnlyPendingTimers();
      });
    }
    vi.useRealTimers();
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

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      const submitButton = screen.getByRole("button", { name: /Submit/iu });

      await user.type(input, String(solveDisplayedProblem()));
      await user.click(submitButton);

      await waitFor(() => {
        expect(mockOnComplete).toHaveBeenCalledTimes(1);
      });
    });

    it("shows an error and recovers with a fresh problem after a wrong answer", () => {
      // Fake timers are opt-in here because the recovery is delayed by a timeout,
      // and userEvent deadlocks with fake timers under Vitest.
      vi.useFakeTimers();

      render(
        <MathChallenge
          difficulty="easy"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />,
      );

      const input = screen.getByPlaceholderText("Your answer");
      const wrongAnswer = solveDisplayedProblem() + 1;
      fireEvent.change(input, { target: { value: String(wrongAnswer) } });
      fireEvent.submit(formElementOf(input));

      expect(screen.getByText(/Incorrect/iu)).toBeInTheDocument();
      expect(input).toBeDisabled();
      expect(mockOnComplete).not.toHaveBeenCalled();

      act(() => {
        vi.advanceTimersByTime(1500);
      });

      expect(screen.queryByText(/Incorrect/iu)).not.toBeInTheDocument();
      expect(input).toHaveValue("");
      expect(input).not.toBeDisabled();
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
