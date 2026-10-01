import { useBackendObservationStore } from "@/backend/observationStore";
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { SessionIndicator } from "./SessionIndicator";
import { resetAppStateForTest } from "@/test/resetAppState";
import { createSettingsForTest } from "@/test/settingsFixture";

function setLongBreakInterval(longBreakInterval: number): void {
  useBackendObservationStore.setState({
    settings: createSettingsForTest({ longBreakInterval }),
  });
}

function getSessionIndicator(): HTMLElement {
  return screen.getByText(/ポモドーロ \d+\/\d+/u);
}

describe("SessionIndicator", () => {
  beforeEach(() => {
    resetAppStateForTest();
    setLongBreakInterval(4);
  });

  describe("Visibility", () => {
    it("should not render when cycle is 0", () => {
      useBackendObservationStore.setState({ currentCycle: 0 });
      render(<SessionIndicator />);
      const indicator = screen.queryByText(/ポモドーロ \d+\/\d+/u);
      expect(indicator).not.toBeInTheDocument();
    });

    it("should not render before the settings have arrived", () => {
      useBackendObservationStore.setState({ currentCycle: 2, settings: null });
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
        useBackendObservationStore.setState({ currentCycle });
        setLongBreakInterval(longBreakInterval);
        render(<SessionIndicator />);

        expect(getSessionIndicator()).toHaveTextContent(expectedText);
      },
    );

    it("should update when cycle changes", () => {
      useBackendObservationStore.setState({ currentCycle: 1 });
      render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/4");

      act(() => {
        useBackendObservationStore.setState({ currentCycle: 3 });
      });
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 3/4");
    });
  });

  describe("Settings Integration", () => {
    it("should reflect custom long break interval", () => {
      useBackendObservationStore.setState({ currentCycle: 2 });
      setLongBreakInterval(6);
      render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 2/6");
    });

    it("should update when settings change", () => {
      useBackendObservationStore.setState({ currentCycle: 1 });
      render(<SessionIndicator />);
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/4");

      act(() => {
        setLongBreakInterval(3);
      });
      expect(getSessionIndicator()).toHaveTextContent("ポモドーロ 1/3");
    });
  });

  describe("Lowered Interval", () => {
    it("shows a cycle past the interval as the last cycle", () => {
      useBackendObservationStore.setState({ currentCycle: 4 });
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
      useBackendObservationStore.setState({ currentCycle: 2 });
      render(<SessionIndicator />);
      const indicator = getSessionIndicator();
      expect(indicator).toHaveAttribute("aria-label", "ポモドーロ 2 / 4");
    });
  });
});
