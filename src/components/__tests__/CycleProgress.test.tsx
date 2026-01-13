import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { CycleProgress } from "../Timer/CycleProgress";
import { useTimerStore } from "../../store/timerStore";
import { useSettingsStore } from "../../store/settingsStore";

describe("CycleProgress", () => {
  beforeEach(() => {
    useTimerStore.getState().reset();
    useSettingsStore.getState().resetToDefaults();
  });

  describe("Visibility", () => {
    it("should not render when cycle is 0", () => {
      render(<CycleProgress />);
      const progress = screen.queryByTestId("cycle-progress");
      expect(progress).not.toBeInTheDocument();
    });

    it("should render when cycle is 1 or greater", () => {
      useTimerStore.setState({ currentCycle: 1 });
      render(<CycleProgress />);
      const progress = screen.getByTestId("cycle-progress");
      expect(progress).toBeInTheDocument();
    });
  });

  describe("Dot Count", () => {
    it("should render 4 dots for default interval (4)", () => {
      useTimerStore.setState({ currentCycle: 1 });
      render(<CycleProgress />);

      expect(screen.getByTestId("cycle-dot-1")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-2")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-3")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-4")).toBeInTheDocument();
    });

    it("should render correct number of dots for custom interval", () => {
      useTimerStore.setState({ currentCycle: 1 });
      useSettingsStore.getState().setLongBreakInterval(6);
      render(<CycleProgress />);

      expect(screen.getByTestId("cycle-dot-1")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-2")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-3")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-4")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-5")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-6")).toBeInTheDocument();
    });
  });

  describe("Dot States - Cycle 1", () => {
    it("should show 1 filled dot and 3 empty dots (●○○○)", () => {
      useTimerStore.setState({ currentCycle: 1 });
      render(<CycleProgress />);

      // Filled dot (blue)
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");

      // Empty dots (gray)
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-gray-300");
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-gray-300");
      expect(screen.getByTestId("cycle-dot-4")).toHaveClass("bg-gray-300");
    });
  });

  describe("Dot States - Cycle 2", () => {
    it("should show 2 filled dots and 2 empty dots (●●○○)", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<CycleProgress />);

      // Filled dots
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-blue-600");

      // Empty dots
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-gray-300");
      expect(screen.getByTestId("cycle-dot-4")).toHaveClass("bg-gray-300");
    });
  });

  describe("Dot States - Cycle 3", () => {
    it("should show 3 filled dots and 1 empty dot (●●●○)", () => {
      useTimerStore.setState({ currentCycle: 3 });
      render(<CycleProgress />);

      // Filled dots
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-blue-600");

      // Empty dot
      expect(screen.getByTestId("cycle-dot-4")).toHaveClass("bg-gray-300");
    });
  });

  describe("Dot States - Cycle 4", () => {
    it("should show 4 filled dots (●●●●)", () => {
      useTimerStore.setState({ currentCycle: 4 });
      render(<CycleProgress />);

      // All filled dots
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-4")).toHaveClass("bg-blue-600");
    });
  });

  describe("Dynamic Updates", () => {
    it("should update when cycle progresses", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<CycleProgress />);

      // Initially: ●○○○
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-gray-300");

      // Progress to cycle 2: ●●○○
      useTimerStore.setState({ currentCycle: 2 });
      rerender(<CycleProgress />);

      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-gray-300");
    });

    it("should update when settings change", () => {
      useTimerStore.setState({ currentCycle: 2 });
      const { rerender } = render(<CycleProgress />);

      // Initially: 4 dots, 2 filled
      expect(screen.getAllByRole("img")[0].children).toHaveLength(4);

      // Change to 6 dots
      useSettingsStore.getState().setLongBreakInterval(6);
      rerender(<CycleProgress />);

      expect(screen.getAllByRole("img")[0].children).toHaveLength(6);
      expect(screen.getByTestId("cycle-dot-1")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-2")).toHaveClass("bg-blue-600");
      expect(screen.getByTestId("cycle-dot-3")).toHaveClass("bg-gray-300");
    });
  });

  describe("Accessibility", () => {
    it("should have role='img' on container", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<CycleProgress />);

      const container = screen.getByTestId("cycle-progress");
      expect(container).toHaveAttribute("role", "img");
    });

    it("should have descriptive aria-label on container", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<CycleProgress />);

      const container = screen.getByTestId("cycle-progress");
      expect(container).toHaveAttribute(
        "aria-label",
        "長い休憩まで 2 / 4 サイクル完了"
      );
    });

    it("should have aria-label on each dot", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<CycleProgress />);

      expect(screen.getByTestId("cycle-dot-1")).toHaveAttribute(
        "aria-label",
        "サイクル 1 完了"
      );
      expect(screen.getByTestId("cycle-dot-2")).toHaveAttribute(
        "aria-label",
        "サイクル 2 完了"
      );
      expect(screen.getByTestId("cycle-dot-3")).toHaveAttribute(
        "aria-label",
        "サイクル 3"
      );
      expect(screen.getByTestId("cycle-dot-4")).toHaveAttribute(
        "aria-label",
        "サイクル 4"
      );
    });
  });

  describe("Edge Cases", () => {
    it("should handle cycle 0 (not render)", () => {
      useTimerStore.setState({ currentCycle: 0 });
      render(<CycleProgress />);

      expect(screen.queryByTestId("cycle-progress")).not.toBeInTheDocument();
    });

    it("should handle minimum interval (2)", () => {
      useTimerStore.setState({ currentCycle: 1 });
      useSettingsStore.getState().setLongBreakInterval(2);
      render(<CycleProgress />);

      expect(screen.getByTestId("cycle-dot-1")).toBeInTheDocument();
      expect(screen.getByTestId("cycle-dot-2")).toBeInTheDocument();
      expect(screen.queryByTestId("cycle-dot-3")).not.toBeInTheDocument();
    });

    it("should handle maximum interval (10)", () => {
      useTimerStore.setState({ currentCycle: 5 });
      useSettingsStore.getState().setLongBreakInterval(10);
      render(<CycleProgress />);

      // Should have 10 dots
      for (let i = 1; i <= 10; i++) {
        expect(screen.getByTestId(`cycle-dot-${i}`)).toBeInTheDocument();
      }

      // First 5 should be filled
      for (let i = 1; i <= 5; i++) {
        expect(screen.getByTestId(`cycle-dot-${i}`)).toHaveClass("bg-blue-600");
      }

      // Last 5 should be empty
      for (let i = 6; i <= 10; i++) {
        expect(screen.getByTestId(`cycle-dot-${i}`)).toHaveClass("bg-gray-300");
      }
    });
  });
});
