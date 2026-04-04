import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { BreakStats } from "./BreakStats";
import { useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

function expectBreakStat(labelText: string, valueText: string): void {
  const label = screen.getByText(labelText);
  expect(label.parentElement).toHaveTextContent(`${labelText} ${valueText}`);
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

    render(<BreakStats />);

    expectBreakStat("今日のポモドーロ:", "3");
    expectBreakStat("現在のサイクル:", "2");
  });

  it("reacts to timer-store updates", () => {
    render(<BreakStats />);

    act(() => {
      useTimerStore.setState({
        completedPomodoros: 5,
        currentCycle: 4,
      });
    });

    expectBreakStat("今日のポモドーロ:", "5");
    expectBreakStat("現在のサイクル:", "4");
  });
});
