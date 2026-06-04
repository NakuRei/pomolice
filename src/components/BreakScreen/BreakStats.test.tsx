import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { BreakStats } from "./BreakStats";
import { useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

function expectBreakStat(
  scope: HTMLElement,
  labelText: string,
  valueText: string,
): void {
  expect(scope).toHaveTextContent(`${labelText} ${valueText}`);
}

describe("BreakStats", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
  });

  it("renders both statistics labels", () => {
    render(<BreakStats />);

    expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
    expect(screen.getByText("現在のサイクル:")).toBeInTheDocument();
  });

  it("shows the current timer-store values", () => {
    useTimerStore.setState({
      completedPomodoros: 3,
      currentCycle: 2,
    });

    const { container } = render(<BreakStats />);

    expectBreakStat(container, "今日のポモドーロ:", "3");
    expectBreakStat(container, "現在のサイクル:", "2");
  });

  it("reacts to timer-store updates", () => {
    const { container } = render(<BreakStats />);

    act(() => {
      useTimerStore.setState({
        completedPomodoros: 5,
        currentCycle: 4,
      });
    });

    expectBreakStat(container, "今日のポモドーロ:", "5");
    expectBreakStat(container, "現在のサイクル:", "4");
  });
});
