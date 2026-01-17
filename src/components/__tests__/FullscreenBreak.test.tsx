import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { FullscreenBreak } from "../BreakScreen/FullscreenBreak";
import { useTimerStore, SessionType } from "../../store/timerStore";

describe("FullscreenBreak", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
    useTimerStore.setState({
      sessionType: SessionType.SHORT_BREAK,
      remainingSeconds: 300,
    });
  });

  describe("Rendering", () => {
    it("should render fullscreen break screen", () => {
      render(<FullscreenBreak />);

      const breakScreen = screen.getByTestId("fullscreen-break");
      expect(breakScreen).toBeInTheDocument();
    });

    it("should display timer in large format", () => {
      render(<FullscreenBreak />);

      const timer = screen.getByTestId("break-timer-display");
      expect(timer).toBeInTheDocument();
      expect(timer).toHaveClass("text-[120px]");
    });

    it("should display session type", () => {
      render(<FullscreenBreak />);

      expect(screen.getByText("短い休憩")).toBeInTheDocument();
    });

    it("should render motivational message component", () => {
      render(<FullscreenBreak />);

      // MotivationalMessage renders with specific styling (text-2xl)
      // Find by the unique combination of classes that only the message has
      const { container } = render(<FullscreenBreak />);
      const messageElement = container.querySelector('.text-2xl.font-medium.text-gray-100');
      expect(messageElement).toBeInTheDocument();
    });

    it("should render break statistics", () => {
      render(<FullscreenBreak />);

      expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
    });

    it("should render skip button", () => {
      render(<FullscreenBreak />);

      const skipButton = screen.getByText("スキップ");
      expect(skipButton).toBeInTheDocument();
    });
  });

  describe("Session Type Display", () => {
    it("should display 'short break' for short break", () => {
      useTimerStore.setState({ sessionType: SessionType.SHORT_BREAK });

      render(<FullscreenBreak />);

      expect(screen.getByText("短い休憩")).toBeInTheDocument();
    });

    it("should display 'long break' for long break", () => {
      useTimerStore.setState({ sessionType: SessionType.LONG_BREAK });

      render(<FullscreenBreak />);

      expect(screen.getByText("長い休憩")).toBeInTheDocument();
    });
  });

  describe("Timer Display", () => {
    it("should format remaining time correctly (MM:SS)", () => {
      useTimerStore.setState({ remainingSeconds: 300 }); // 5:00

      render(<FullscreenBreak />);

      const timer = screen.getByTestId("break-timer-display");
      expect(timer).toHaveTextContent("05:00");
    });

    it("should update when remaining seconds changes", () => {
      const { rerender } = render(<FullscreenBreak />);

      expect(screen.getByText("05:00")).toBeInTheDocument();

      act(() => {
        useTimerStore.setState({ remainingSeconds: 60 }); // 1:00
      });
      rerender(<FullscreenBreak />);

      expect(screen.getByText("01:00")).toBeInTheDocument();
    });

    it("should handle single digit seconds", () => {
      useTimerStore.setState({ remainingSeconds: 5 });

      render(<FullscreenBreak />);

      expect(screen.getByText("00:05")).toBeInTheDocument();
    });
  });

  describe("Skip Button", () => {
    it("should be positioned in bottom-right corner", () => {
      const { container } = render(<FullscreenBreak />);

      const skipButtonContainer = container.querySelector(".absolute.bottom-3.right-3");
      expect(skipButtonContainer).toBeInTheDocument();
    });

    it("should show skip flow modal when clicked", () => {
      render(<FullscreenBreak />);

      const skipButton = screen.getByText("スキップ");
      fireEvent.click(skipButton);

      // Phase 5: Shows 10-step skip flow instead of simple dialog
      expect(screen.getByText(/Step/i)).toBeInTheDocument();
      expect(screen.getByText(/Are you sure you want to skip/i)).toBeInTheDocument();
    });
  });

  describe("Skip Confirmation Dialog", () => {
    beforeEach(() => {
      render(<FullscreenBreak />);
      const skipButton = screen.getByText("スキップ");
      fireEvent.click(skipButton);
    });

    // Phase 5: Skip flow modal with 10 steps
    it("should show skip flow progress indicator", () => {
      expect(screen.getByText(/Step/i)).toBeInTheDocument();
      expect(screen.getByText(/of 10/i)).toBeInTheDocument();
    });

    it("should show first confirmation step", () => {
      expect(screen.getByText(/Are you sure you want to skip/i)).toBeInTheDocument();
    });

    it("should have take break button", () => {
      expect(screen.getByText(/No, Take Break/i)).toBeInTheDocument();
    });
  });

  describe("Styling", () => {
    it("should have dark background", () => {
      const { container } = render(<FullscreenBreak />);

      const breakScreen = container.querySelector(".bg-gray-900");
      expect(breakScreen).toBeInTheDocument();
    });

    it("should cover entire screen (fixed inset-0)", () => {
      const { container } = render(<FullscreenBreak />);

      const breakScreen = container.querySelector(".fixed.inset-0");
      expect(breakScreen).toBeInTheDocument();
    });

    it("should have high z-index for layering", () => {
      const { container } = render(<FullscreenBreak />);

      const breakScreen = container.querySelector(".z-50");
      expect(breakScreen).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("should have accessible timer (aria-live)", () => {
      render(<FullscreenBreak />);

      const timer = screen.getByTestId("break-timer-display");
      expect(timer).toHaveAttribute("aria-live", "polite");
      expect(timer).toHaveAttribute("aria-atomic", "true");
    });

    it("should have test-id for automated testing", () => {
      render(<FullscreenBreak />);

      expect(screen.getByTestId("fullscreen-break")).toBeInTheDocument();
      expect(screen.getByTestId("break-timer-display")).toBeInTheDocument();
    });
  });

  describe("Integration", () => {
    it("should integrate with BreakStats component", () => {
      useTimerStore.setState({ completedPomodoros: 3, currentCycle: 3 });

      render(<FullscreenBreak />);

      // Both completed pomodoros and current cycle will show 3
      const allThrees = screen.getAllByText("3");
      expect(allThrees.length).toBe(2);
    });

    it("should handle timer countdown during break", () => {
      useTimerStore.setState({ remainingSeconds: 10 });

      const { rerender } = render(<FullscreenBreak />);

      expect(screen.getByText("00:10")).toBeInTheDocument();

      act(() => {
        useTimerStore.setState({ remainingSeconds: 5 });
      });
      rerender(<FullscreenBreak />);

      expect(screen.getByText("00:05")).toBeInTheDocument();
    });
  });
});
