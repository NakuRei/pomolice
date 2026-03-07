import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { SessionIndicator } from "../Timer/SessionIndicator";
import { useTimerStore } from "../../store/timerStore";
import { useSettingsStore } from "../../store/settingsStore";

describe("SessionIndicator", () => {
  beforeEach(() => {
    useTimerStore.getState().reset();
    useSettingsStore.getState().resetToDefaults();
  });

  describe("Visibility", () => {
    it("should not render when cycle is 0", () => {
      render(<SessionIndicator />);
      const indicator = screen.queryByTestId("session-indicator");
      expect(indicator).not.toBeInTheDocument();
    });

    it("should render when cycle is 1 or greater", () => {
      useTimerStore.setState({ currentCycle: 1 });
      render(<SessionIndicator />);
      const indicator = screen.getByTestId("session-indicator");
      expect(indicator).toBeInTheDocument();
    });
  });

  describe("Display Content", () => {
    it("should display current cycle and interval (1/4)", () => {
      useTimerStore.setState({ currentCycle: 1 });
      render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 1/4"
      );
    });

    it("should display current cycle and interval (2/4)", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 2/4"
      );
    });

    it("should display current cycle and interval (4/4)", () => {
      useTimerStore.setState({ currentCycle: 4 });
      render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 4/4"
      );
    });

    it("should update when cycle changes", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 1/4"
      );

      // Update cycle
      act(() => {
        useTimerStore.setState({ currentCycle: 3 });
      });
      rerender(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 3/4"
      );
    });
  });

  describe("Settings Integration", () => {
    it("should reflect custom long break interval", () => {
      useTimerStore.setState({ currentCycle: 2 });
      useSettingsStore.getState().setLongBreakInterval(6);
      render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 2/6"
      );
    });

    it("should update when settings change", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 1/4"
      );

      // Change interval setting
      act(() => {
        useSettingsStore.getState().setLongBreakInterval(3);
      });
      rerender(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 1/3"
      );
    });
  });

  describe("Accessibility", () => {
    it("should have aria-label", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<SessionIndicator />);
      const indicator = screen.getByTestId("session-indicator");
      expect(indicator).toHaveAttribute("aria-label", "ポモドーロ 2 / 4");
    });
  });

  describe("Edge Cases", () => {
    it("should handle large cycle numbers", () => {
      useTimerStore.setState({ currentCycle: 10 });
      useSettingsStore.getState().setLongBreakInterval(10);
      render(<SessionIndicator />);
      expect(screen.getByTestId("session-indicator")).toHaveTextContent(
        "ポモドーロ 10/10"
      );
    });
  });
});
