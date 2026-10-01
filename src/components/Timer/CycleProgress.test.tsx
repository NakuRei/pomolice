import { useTimerObservationStore } from "@/store/timerObservationStore";
import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { CycleProgress } from "./CycleProgress";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";

describe("CycleProgress", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useTimerObservationStore
      .getState()
      .observeFullState(createTimerSnapshotForTest());
  });

  it("shows nothing until the settings have arrived", () => {
    useTimerObservationStore.setState({ settings: null });

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
    useTimerObservationStore.setState({ currentCycle: 1 });

    render(<CycleProgress />);
    expect(
      screen.getByRole("img", { name: "ポモドーロ 1 / 4" }),
    ).toBeInTheDocument();

    act(() => {
      useTimerObservationStore.setState({ currentCycle: 2 });
    });
    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 4" }),
    ).toBeInTheDocument();

    act(() => {
      useTimerObservationStore.setState({
        settings: createSettingsForTest({ longBreakInterval: 5 }),
      });
    });

    expect(
      screen.getByRole("img", { name: "ポモドーロ 2 / 5" }),
    ).toBeInTheDocument();
  });

  it.each([
    {
      sessionType: "work",
      currentCycle: 2,
      remainingSeconds: 750,
      sessionDurationSeconds: 1500,
      segmentKey: "work-2",
    },
    {
      sessionType: "short_break",
      currentCycle: 2,
      remainingSeconds: 150,
      sessionDurationSeconds: 300,
      segmentKey: "break-2",
    },
    {
      sessionType: "long_break",
      currentCycle: 4,
      remainingSeconds: 450,
      sessionDurationSeconds: 900,
      segmentKey: "break-4",
    },
  ] as const)(
    "fills the running $sessionType against the length it started with",
    ({ segmentKey, ...timerState }) => {
      useTimerObservationStore.setState({
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
      useTimerObservationStore.setState({
        settings: createSettingsForTest({ longBreakInterval: 2 }),
        currentCycle: 3,
        sessionType: "work",
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
