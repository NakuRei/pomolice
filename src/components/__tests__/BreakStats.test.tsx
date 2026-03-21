import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { BreakStats } from "../BreakScreen/BreakStats";
import { useTimerStore } from "@/store/timerStore";

describe("BreakStats", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
  });

  describe("Rendering", () => {
    it("should render all statistics sections", () => {
      render(<BreakStats />);

      expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
      expect(screen.getByText("現在のサイクル:")).toBeInTheDocument();
    });

    it("should display correct initial values", () => {
      render(<BreakStats />);

      // Initial completed pomodoros and cycle should both be 0
      const allZeros = screen.getAllByText("0");
      expect(allZeros.length).toBeGreaterThanOrEqual(2);
    });
  });

  describe("Completed Pomodoros", () => {
    it("should display current completed pomodoros count", () => {
      // Set completed pomodoros to 3
      useTimerStore.setState({ completedPomodoros: 3 });

      render(<BreakStats />);

      expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument();
    });

    it("should update when completed pomodoros changes", () => {
      const { rerender } = render(<BreakStats />);

      expect(screen.getAllByText("0").length).toBeGreaterThanOrEqual(1);

      // Update completed pomodoros
      act(() => {
        useTimerStore.setState({ completedPomodoros: 5 });
      });
      rerender(<BreakStats />);

      expect(screen.getByText("5")).toBeInTheDocument();
    });

    it("should handle large numbers", () => {
      useTimerStore.setState({ completedPomodoros: 99 });

      render(<BreakStats />);

      expect(screen.getByText("99")).toBeInTheDocument();
    });
  });

  describe("Current Cycle", () => {
    it("should display current cycle number", () => {
      useTimerStore.setState({ currentCycle: 2 });

      render(<BreakStats />);

      expect(screen.getByText("現在のサイクル:")).toBeInTheDocument();
      expect(screen.getByText("2")).toBeInTheDocument();
    });

    it("should update when cycle changes", () => {
      const { rerender } = render(<BreakStats />);

      act(() => {
        useTimerStore.setState({ currentCycle: 3 });
      });
      rerender(<BreakStats />);

      expect(screen.getByText("3")).toBeInTheDocument();
    });
  });

  describe("Integration with Timer Store", () => {
    it("should react to timer store changes", () => {
      const { rerender } = render(<BreakStats />);

      // Simulate completing a work session
      act(() => {
        useTimerStore.setState({
          completedPomodoros: 1,
          currentCycle: 1,
        });
      });

      rerender(<BreakStats />);

      // Both pomodoros and cycle will show 1
      const allOnes = screen.getAllByText("1");
      expect(allOnes.length).toBe(2);
    });

    it("should handle multiple work cycles", () => {
      useTimerStore.setState({
        completedPomodoros: 4,
        currentCycle: 4,
      });

      render(<BreakStats />);

      // Both pomodoros and cycle will show 4
      const allFours = screen.getAllByText("4");
      expect(allFours.length).toBe(2);
    });
  });
});
