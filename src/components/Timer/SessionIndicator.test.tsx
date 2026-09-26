import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { SessionIndicator } from "./SessionIndicator";
import { useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

function setLongBreakInterval(longBreakInterval: number): void {
  useTimerStore.setState({
    settings: createSettingsForTest({ longBreakInterval }),
  });
}

function getSessionIndicator(): HTMLElement {
  return screen.getByText(/ポモドーロ \d+\/\d+/u);
}

describe("SessionIndicator", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    setLongBreakInterval(4);
  });

  describe("Visibility", () => {
    it("should not render when cycle is 0", () => {
      render(<SessionIndicator />);
      const indicator = screen.queryByText(/ポモドーロ \d+\/\d+/u);
      expect(indicator).not.toBeInTheDocument();
    });

    it("should not render before the settings have arrived", () => {
      useTimerStore.setState({ currentCycle: 2, settings: null });
      render(<SessionIndicator />);

      expect(
        screen.queryByText(/ポモドーロ \d+\/\d+/u),
      ).not.toBeInTheDocument();
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
        setLongBreakInterval(longBreakInterval);
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
      setLongBreakInterval(6);
      render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 2/6");
    });

    it("should update when settings change", () => {
      useTimerStore.setState({ currentCycle: 1 });
      const { rerender } = render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/4");

      // Change interval setting
      act(() => {
        setLongBreakInterval(3);
      });
      rerender(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/3");
    });
  });

  describe("Lowered Interval", () => {
    it("shows a cycle past the interval as the last cycle", () => {
      useTimerStore.setState({ currentCycle: 4 });
      setLongBreakInterval(3);
      render(<SessionIndicator />);

      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 3/3");
      expect(getSessionIndicator()).toHaveAttribute(
        "aria-label",
        "ポモドーロ 3 / 3",
      );
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
