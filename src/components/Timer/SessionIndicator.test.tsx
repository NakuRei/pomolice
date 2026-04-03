import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { SessionIndicator } from "./SessionIndicator";
import { useTimerStore } from "@/store/timerStore";
import { useSettingsStore } from "@/store/settingsStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

function getSessionIndicator(): HTMLElement {
  return screen.getByText(/ポモドーロ \d+\/\d+/u);
}

describe("SessionIndicator", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useSettingsStore.getState().resetToDefaults();
  });

  describe("Visibility", () => {
    it("should not render when cycle is 0", () => {
      render(<SessionIndicator />);
      const indicator = screen.queryByText(/ポモドーロ \d+\/\d+/u);
      expect(indicator).not.toBeInTheDocument();
    });
  });

  describe("Display Content", () => {
    it.each([
      { currentCycle: 1, longBreakInterval: 4, expectedText: "ポモドーロ 1/4" },
      { currentCycle: 2, longBreakInterval: 4, expectedText: "ポモドーロ 2/4" },
      { currentCycle: 4, longBreakInterval: 4, expectedText: "ポモドーロ 4/4" },
      {
        currentCycle: 10,
        longBreakInterval: 10,
        expectedText: "ポモドーロ 10/10",
      },
    ])(
      "should display the current cycle summary: $expectedText",
      ({ currentCycle, longBreakInterval, expectedText }) => {
        useTimerStore.setState({ currentCycle });
        useSettingsStore.getState().setLongBreakInterval(longBreakInterval);
        render(<SessionIndicator />);

        expect(getSessionIndicator()).toHaveTextContent(expectedText);
      },
    );

    it("should update when cycle changes", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/4");

      // Update cycle
      act(() => {
        useTimerStore.setState({ currentCycle: 3 });
      });
      rerender(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 3/4");
    });
  });

  describe("Settings Integration", () => {
    it("should reflect custom long break interval", () => {
      useTimerStore.setState({ currentCycle: 2 });
      useSettingsStore.getState().setLongBreakInterval(6);
      render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 2/6");
    });

    it("should update when settings change", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/4");

      // Change interval setting
      act(() => {
        useSettingsStore.getState().setLongBreakInterval(3);
      });
      rerender(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/3");
    });
  });

  describe("Accessibility", () => {
    it("should have aria-label", () => {
      useTimerStore.setState({ currentCycle: 2 });
      render(<SessionIndicator />);
      const indicator = getSessionIndicator();
      expect(indicator).toHaveAttribute("aria-label", "ポモドーロ 2 / 4");
    });
  });
});
