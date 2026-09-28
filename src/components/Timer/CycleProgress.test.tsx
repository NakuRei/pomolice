import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { CycleProgress } from "./CycleProgress";
import { useTimerStore } from "@/store/timerStore";
import { SessionType } from "@/types/timer";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

describe("CycleProgress", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useTimerStore.setState({ settings: createSettingsForTest() });
  });

  it("shows nothing until the settings have arrived", () => {
    useTimerStore.setState({ settings: null });

    render(<CycleProgress />);

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("shows the whole cycle before the timer has ever started", () => {
    render(<CycleProgress />);

    expect(
      screen.getByRole("img", { name: "ポモドーロ 開始前 全 4 サイクル" }),
    ).toBeInTheDocument();
  });

  it("updates the cycle summary when the timer or settings change", () => {
    useTimerStore.setState({ currentCycle: 1 });

    render(<CycleProgress />);
    expect(
      screen.getByRole("img", { name: "ポモドーロ 1 / 4" }),
    ).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ currentCycle: 2 });
    });
    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 4" }),
    ).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({
        settings: createSettingsForTest({ longBreakInterval: 5 }),
      });
    });

    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 5" }),
    ).toBeInTheDocument();
  });

  it.each([
    {
      sessionType: SessionType.WORK,
      currentCycle: 2,
      remainingSeconds: 750,
      sessionDurationSeconds: 1500,
      segmentKey: "work-2",
    },
    {
      sessionType: SessionType.SHORT_BREAK,
      currentCycle: 2,
      remainingSeconds: 150,
      sessionDurationSeconds: 300,
      segmentKey: "break-2",
    },
    {
      sessionType: SessionType.LONG_BREAK,
      currentCycle: 4,
      remainingSeconds: 450,
      sessionDurationSeconds: 900,
      segmentKey: "break-4",
    },
  ])(
    "fills the running $sessionType against the length it started with",
    ({ segmentKey, ...timerState }) => {
      useTimerStore.setState({
        settings: createSettingsForTest({
          workDurationSecs: 3000,
          shortBreakDurationSecs: 600,
          longBreakDurationSecs: 1800,
        }),
        ...timerState,
      });

      render(<CycleProgress />);

      const fill = screen.getByTestId(
        `cycle-segment-${segmentKey}`,
      ).firstElementChild;
      expect(fill).toHaveStyle({ width: "50%" });
    },
  );

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

    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 2" }),
    ).toBeInTheDocument();
  });
});
