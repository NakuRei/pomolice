import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { CycleProgress } from "./CycleProgress";
import { SessionType, useTimerStore } from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

function segmentStates(): Record<string, string | null> {
  const bar = screen.getByTestId("cycle-progress");
  const states: Record<string, string | null> = {};

  for (const segment of bar.querySelectorAll(
    "[data-testid^='cycle-segment-']",
  )) {
    const testId = segment.getAttribute("data-testid");
    if (testId !== null) {
      states[testId.replace("cycle-segment-", "")] =
        segment.getAttribute("data-state");
    }
  }

  return states;
}

describe("CycleProgress", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useTimerStore.setState({ settings: createSettingsForTest() });
  });

  it("shows nothing until the settings have arrived", () => {
    useTimerStore.setState({ settings: null });

    render(<CycleProgress />);

    expect(screen.queryByTestId("cycle-progress")).not.toBeInTheDocument();
  });

  it("shows the whole cycle before the timer has ever started", () => {
    render(<CycleProgress />);

    expect(
      screen.getByRole("img", { name: "ポモドーロ 開始前 全 4 サイクル" }),
    ).toBeInTheDocument();
    expect(
      Object.values(segmentStates()).every((state) => state === "upcoming"),
    ).toBe(true);
  });

  it("draws one work span and one break span per configured cycle", () => {
    useTimerStore.setState({
      settings: createSettingsForTest({ longBreakInterval: 6 }),
    });

    render(<CycleProgress />);

    expect(Object.keys(segmentStates())).toEqual([
      "work-1",
      "break-1",
      "work-2",
      "break-2",
      "work-3",
      "break-3",
      "work-4",
      "break-4",
      "work-5",
      "break-5",
      "work-6",
      "break-6",
    ]);
  });

  it("marks the running work session and everything before it", () => {
    act(() => {
      useTimerStore.setState({
        currentCycle: 2,
        sessionType: SessionType.WORK,
        remainingSeconds: 750,
      });
    });

    render(<CycleProgress />);

    expect(segmentStates()).toMatchObject({
      "work-1": "completed",
      "break-1": "completed",
      "work-2": "active",
      "break-2": "upcoming",
    });
    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 4" }),
    ).toBeInTheDocument();
  });

  it("moves the mark onto the break once the work session ends", () => {
    render(<CycleProgress />);

    act(() => {
      useTimerStore.setState({
        currentCycle: 1,
        sessionType: SessionType.SHORT_BREAK,
        remainingSeconds: 150,
      });
    });

    expect(segmentStates()).toMatchObject({
      "work-1": "completed",
      "break-1": "active",
      "work-2": "upcoming",
    });
  });

  it("follows a changed cycle count while running", () => {
    act(() => {
      useTimerStore.setState({ currentCycle: 1 });
    });

    render(<CycleProgress />);
    expect(Object.keys(segmentStates())).toHaveLength(8);

    act(() => {
      useTimerStore.setState({
        settings: createSettingsForTest({ longBreakInterval: 5 }),
      });
    });

    expect(Object.keys(segmentStates())).toHaveLength(10);
    expect(
      screen.getByRole("img", { name: "ポモドーロ 1 / 5" }),
    ).toBeInTheDocument();
  });

  it("fills the running session against the length it started with", () => {
    act(() => {
      useTimerStore.setState({
        settings: createSettingsForTest({ workDurationSecs: 50 * 60 }),
        currentCycle: 1,
        sessionType: SessionType.WORK,
        remainingSeconds: 750,
        sessionDurationSeconds: 1500,
      });
    });

    render(<CycleProgress />);

    const fill = screen.getByTestId("cycle-segment-work-1").firstElementChild;
    expect(fill).toHaveStyle({ width: "50%" });
  });

  it("shows a cycle past a lowered interval as the last cycle", () => {
    act(() => {
      useTimerStore.setState({
        settings: createSettingsForTest({ longBreakInterval: 2 }),
        currentCycle: 3,
        sessionType: SessionType.WORK,
        remainingSeconds: 750,
        sessionDurationSeconds: 1500,
      });
    });

    render(<CycleProgress />);

    expect(segmentStates()).toEqual({
      "work-1": "completed",
      "break-1": "completed",
      "work-2": "active",
      "break-2": "upcoming",
    });
    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 2" }),
    ).toBeInTheDocument();
  });
});
