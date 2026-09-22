import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useSettingsStore } from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";
import { SettingsTimerPage } from "./SettingsTimerPage";

describe("SettingsTimerPage", () => {
  beforeEach(() => {
    useSettingsStore.getState().reset();
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      isFullscreenBreak: false,
      requiresManualContinueDecision: false,
      isConnected: true,
      bootAutoStartPromptWindow: null,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
  });

  it("saves timer settings and keeps unrelated settings unchanged", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      workDuration: 25,
      autoRunFeatureEnabled: true,
      notificationSoundVolumePercent: 50,
      postBreakTransitionPolicy: "require_manual_continue",
    });

    render(<SettingsTimerPage openSettingsTopScreen={openSettingsTopScreen} />);

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "32");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().workDuration).toBe(32);
    expect(useSettingsStore.getState().postBreakTransitionPolicy).toBe(
      "require_manual_continue",
    );
    expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(true);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(50);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("discards timer draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({ workDuration: 25 });

    render(<SettingsTimerPage openSettingsTopScreen={openSettingsTopScreen} />);

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "44");
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("blocks timer save when long break interval draft is not an integer", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    render(<SettingsTimerPage openSettingsTopScreen={openSettingsTopScreen} />);

    const intervalInput = screen.getByRole("spinbutton", {
      name: "長い休憩の間隔",
    });
    await user.clear(intervalInput);
    await user.type(intervalInput, "2.5");

    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();

    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().longBreakInterval).toBe(4);
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("keeps out-of-range timer draft visible while blocking save", async () => {
    const user = userEvent.setup();

    render(<SettingsTimerPage openSettingsTopScreen={() => undefined} />);

    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "999");

    expect(workInput).toHaveValue(999);
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("disables timer duration inputs and keeps existing warning when timer is running", () => {
    useTimerStore.setState({ status: TimerStatus.WORKING });

    render(<SettingsTimerPage openSettingsTopScreen={() => undefined} />);

    expect(screen.getByRole("spinbutton", { name: "作業時間" })).toBeDisabled();
    expect(screen.getByRole("spinbutton", { name: "短い休憩" })).toBeDisabled();
    expect(screen.getByRole("spinbutton", { name: "長い休憩" })).toBeDisabled();
    expect(
      screen.getByRole("spinbutton", { name: "長い休憩の間隔" }),
    ).toBeDisabled();
    expect(
      screen.getByText(
        "タイマー実行中は時間長設定を変更できません。曜日別スケジュールのみ編集できます。",
      ),
    ).toBeInTheDocument();
  });

  it("focuses the timer heading on mount", async () => {
    render(<SettingsTimerPage openSettingsTopScreen={() => undefined} />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "タイマー設定" }),
      ).toHaveFocus();
    });
  });
});
