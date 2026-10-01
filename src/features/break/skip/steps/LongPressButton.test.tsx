import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import LongPressButton from "./LongPressButton";

const interactionCases = [
  {
    label: "mouse",
    start: (button: HTMLElement): void => {
      fireEvent.mouseDown(button);
    },
    end: (button: HTMLElement): void => {
      fireEvent.mouseUp(button);
    },
  },
  {
    label: "touch",
    start: (button: HTMLElement): void => {
      fireEvent.touchStart(button);
    },
    end: (button: HTMLElement): void => {
      fireEvent.touchEnd(button);
    },
  },
] as const;

describe("LongPressButton", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    if (vi.isFakeTimers()) {
      act(() => {
        vi.runOnlyPendingTimers();
      });
    }
    vi.useRealTimers();
  });

  it("renders the challenge text and both actions", () => {
    render(
      <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
    );

    expect(screen.getByText("Long Press Challenge")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Press and Hold/iu }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /Cancel and Take Break/iu }),
    ).toBeInTheDocument();
  });

  it.each(interactionCases)(
    "shows the early-release notice when a $label press ends too soon",
    ({ start, end }) => {
      // Fake timers are opt-in per test because userEvent clicks deadlock here under Vitest, even with advanceTimers.
      vi.useFakeTimers();

      render(
        <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
      );

      const button = screen.getByRole("button", { name: /Press and Hold/iu });

      act(() => {
        start(button);
        vi.advanceTimersByTime(5000);
        end(button);
      });

      expect(screen.getByText(/You released too early/iu)).toBeInTheDocument();
      expect(screen.getByText("Progress: 0%")).toBeInTheDocument();
    },
  );

  it("resets when the pointer leaves during an active press", () => {
    vi.useFakeTimers();

    render(
      <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
    );

    const button = screen.getByRole("button", { name: /Press and Hold/iu });

    act(() => {
      fireEvent.mouseDown(button);
    });

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    act(() => {
      fireEvent.mouseLeave(button);
    });

    expect(screen.getByText(/You released too early/iu)).toBeInTheDocument();
    expect(screen.getByText("Progress: 0%")).toBeInTheDocument();
  });

  it("calls onComplete after the required hold duration", () => {
    vi.useFakeTimers();

    render(
      <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
    );

    const button = screen.getByRole("button", { name: /Press and Hold/iu });

    act(() => {
      fireEvent.mouseDown(button);
      vi.advanceTimersByTime(15100);
    });

    expect(mockOnComplete).toHaveBeenCalledTimes(1);
  });

  it("clears the early-release notice after the timeout", () => {
    vi.useFakeTimers();

    render(
      <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
    );

    const button = screen.getByRole("button", { name: /Press and Hold/iu });

    act(() => {
      fireEvent.mouseDown(button);
      vi.advanceTimersByTime(1000);
      fireEvent.mouseUp(button);
    });

    expect(screen.getByText(/You released too early/iu)).toBeInTheDocument();

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(
      screen.queryByText(/You released too early/iu),
    ).not.toBeInTheDocument();
  });

  it("calls onCancel when the cancel button is clicked", async () => {
    const user = userEvent.setup();

    render(
      <LongPressButton onComplete={mockOnComplete} onCancel={mockOnCancel} />,
    );

    await user.click(
      screen.getByRole("button", { name: /Cancel and Take Break/iu }),
    );

    expect(mockOnCancel).toHaveBeenCalledTimes(1);
  });
});
