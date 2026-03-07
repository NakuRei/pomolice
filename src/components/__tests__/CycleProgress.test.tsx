import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
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

  describe("Dynamic Updates", () => {
    it("should update when cycle progresses", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<CycleProgress />);

      expect(screen.getByTestId("cycle-progress")).toHaveAttribute(
        "aria-label",
        "長い休憩まで 1 / 4 サイクル完了"
      );

      act(() => {
        useTimerStore.setState({ currentCycle: 2 });
      });
      rerender(<CycleProgress />);

      expect(screen.getByTestId("cycle-progress")).toHaveAttribute(
        "aria-label",
        "長い休憩まで 2 / 4 サイクル完了"
      );
    });

    it("should update when settings change", () => {
      useTimerStore.setState({ currentCycle: 2 });
      const { rerender } = render(<CycleProgress />);

      // Initially: 4 dots, 2 filled
      expect(screen.getAllByRole("img")[0].children).toHaveLength(4);

      // Change to 6 dots
      act(() => {
        useSettingsStore.getState().setLongBreakInterval(6);
      });
      rerender(<CycleProgress />);

      expect(screen.getAllByRole("img")[0].children).toHaveLength(6);
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
    });
  });
});
