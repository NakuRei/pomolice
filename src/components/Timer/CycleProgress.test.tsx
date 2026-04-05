import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { CycleProgress } from "./CycleProgress";
import { useTimerStore } from "@/store/timerStore";
import { useSettingsStore } from "@/store/settingsStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";

describe("CycleProgress", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useSettingsStore.getState().resetToDefaults();
  });

  it("does not render before the first cycle starts", () => {
    render(<CycleProgress />);

    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("renders a progress summary and one dot per configured cycle", () => {
    useTimerStore.setState({ currentCycle: 2 });
    useSettingsStore.getState().setLongBreakInterval(6);

    render(<CycleProgress />);

    expect(
      screen.getByRole("img", { name: "長い休憩まで 2 / 6 サイクル完了" }),
    ).toBeInTheDocument();
    expect(screen.getAllByLabelText(/^サイクル /u)).toHaveLength(6);
    expect(screen.getByLabelText("サイクル 1 完了")).toBeInTheDocument();
    expect(screen.getByLabelText("サイクル 2 完了")).toBeInTheDocument();
    expect(screen.getByLabelText("サイクル 6")).toBeInTheDocument();
  });

  it("updates when the cycle count and interval change", () => {
    useTimerStore.setState({ currentCycle: 1 });

    render(<CycleProgress />);

    expect(
      screen.getByRole("img", { name: "長い休憩まで 1 / 4 サイクル完了" }),
    ).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ currentCycle: 3 });
      useSettingsStore.getState().setLongBreakInterval(5);
    });

    expect(
      screen.getByRole("img", { name: "長い休憩まで 3 / 5 サイクル完了" }),
    ).toBeInTheDocument();
    expect(screen.getAllByLabelText(/^サイクル /u)).toHaveLength(5);
    expect(screen.getByLabelText("サイクル 3 完了")).toBeInTheDocument();
    expect(screen.getByLabelText("サイクル 5")).toBeInTheDocument();
  });
});
