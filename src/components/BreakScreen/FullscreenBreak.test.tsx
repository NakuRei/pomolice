import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { FullscreenBreak } from "./FullscreenBreak";
import { useTimerStore, SessionType } from "@/store/timerStore";
import * as motivationalMessagesModule from "@/data/motivationalMessages";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

describe("FullscreenBreak", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    resetTimerStoreForTest();
    useTimerStore.setState({
      sessionType: SessionType.SHORT_BREAK,
      remainingSeconds: 300,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Rendering", () => {
    it("should render fullscreen break screen", () => {
      render(<FullscreenBreak />);

      const breakScreen = screen.getByTestId("fullscreen-break");
      expect(breakScreen).toBeInTheDocument();
    });

    it("should display timer", () => {
      render(<FullscreenBreak />);

      const timer = screen.getByTestId("break-timer-display");
      expect(timer).toBeInTheDocument();
    });

    it("should display session type", () => {
      render(<FullscreenBreak />);

      expect(screen.getByText("短い休憩")).toBeInTheDocument();
    });

    it("should include motivational message component output", () => {
      const getRandomMessageSpy = vi
        .spyOn(motivationalMessagesModule, "getRandomMessage")
        .mockReturnValue("休憩メッセージ");

      render(<FullscreenBreak />);

      expect(getRandomMessageSpy).toHaveBeenCalledTimes(1);
      expect(screen.getByText("休憩メッセージ")).toBeInTheDocument();
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
    it("should show skip flow modal when clicked", () => {
      render(<FullscreenBreak />);

      const skipButton = screen.getByText("スキップ");
      fireEvent.click(skipButton);

      // Phase 5: Shows 10-step skip flow instead of simple dialog
      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
      expect(
        screen.getByText(/Are you sure you want to skip/iu),
      ).toBeInTheDocument();
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
      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
      expect(screen.getByText(/of 10/iu)).toBeInTheDocument();
    });

    it("should show first confirmation step", () => {
      expect(
        screen.getByText(/Are you sure you want to skip/iu),
      ).toBeInTheDocument();
    });

    it("should have take break button", () => {
      expect(screen.getByText(/No, Take Break/iu)).toBeInTheDocument();
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
