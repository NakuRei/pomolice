import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
} from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";
import { SettingsAutoRunPage } from "./SettingsAutoRunPage";

describe("SettingsAutoRunPage", () => {
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

  it("saves auto-run settings and keeps timer settings unchanged", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      workDuration: 29,
      autoRunFeatureEnabled: false,
      weeklyAutoRunConfig: createWeeklyAutoRunConfigAllDaysDisabled(),
    });

    render(
      <SettingsAutoRunPage openSettingsTopScreen={openSettingsTopScreen} />,
    );

    await user.click(screen.getByRole("switch", { name: "曜日別自動実行" }));
    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    fireEvent.change(screen.getByLabelText("月曜日の開始時刻"), {
      target: { value: "09:00" },
    });
    fireEvent.change(screen.getByLabelText("月曜日の終了時刻"), {
      target: { value: "18:00" },
    });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(true);
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.enabled).toBe(
      true,
    );
    expect(useSettingsStore.getState().workDuration).toBe(29);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("discards auto-run draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    useSettingsStore.setState({
      autoRunFeatureEnabled: false,
      weeklyAutoRunConfig,
    });

    render(
      <SettingsAutoRunPage openSettingsTopScreen={openSettingsTopScreen} />,
    );

    await user.click(screen.getByRole("switch", { name: "曜日別自動実行" }));
    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    fireEvent.change(screen.getByLabelText("月曜日の開始時刻"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByLabelText("月曜日の終了時刻"), {
      target: { value: "19:00" },
    });
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(false);
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.enabled).toBe(
      false,
    );
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("blocks auto-run save when day time text is invalid", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    render(
      <SettingsAutoRunPage openSettingsTopScreen={openSettingsTopScreen} />,
    );

    await user.click(screen.getByRole("switch", { name: "曜日別自動実行" }));
    await user.click(
      screen.getByRole("checkbox", { name: "月曜日の自動実行を有効にする" }),
    );
    fireEvent.change(screen.getByLabelText("月曜日の開始時刻"), {
      target: { value: "invalid-time" },
    });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Start time for mon must use HH:MM (24-hour).",
    );
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("renders weekly auto-run overlap errors inside the shared weekly error notice", () => {
    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    weeklyAutoRunConfig.mon = {
      enabled: true,
      startTime: "23:00",
      endTime: "02:00",
    };
    weeklyAutoRunConfig.tue = {
      enabled: true,
      startTime: "01:00",
      endTime: "03:00",
    };

    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      weeklyAutoRunConfig,
    });

    render(<SettingsAutoRunPage openSettingsTopScreen={() => undefined} />);

    const weeklyErrorNotice = screen.getByRole("alert");

    expect(weeklyErrorNotice).toHaveAttribute("role", "alert");
    expect(weeklyErrorNotice).toHaveTextContent(
      "Schedule windows overlap between mon and tue.",
    );
  });

  it("focuses the auto-run heading on mount", async () => {
    render(<SettingsAutoRunPage openSettingsTopScreen={() => undefined} />);

    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "曜日別自動実行設定" }),
      ).toHaveFocus();
    });
  });
});
