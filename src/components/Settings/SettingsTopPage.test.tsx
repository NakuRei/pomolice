import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
} from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";
import { resetSettingsStoreForTest } from "@/test/resetSettingsStore";
import { SettingsTopPage } from "./SettingsTopPage";

describe("SettingsTopPage", () => {
  beforeEach(() => {
    resetSettingsStoreForTest();
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

  it("renders timer and auto-run summaries on settings top", () => {
    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    weeklyAutoRunConfig.mon.enabled = true;
    weeklyAutoRunConfig.mon.startTime = "09:00";
    weeklyAutoRunConfig.mon.endTime = "18:00";
    weeklyAutoRunConfig.tue.enabled = true;
    weeklyAutoRunConfig.tue.startTime = "09:00";
    weeklyAutoRunConfig.tue.endTime = "18:00";

    useSettingsStore.setState({
      workDuration: 30,
      shortBreakDuration: 6,
      longBreakDuration: 20,
      longBreakInterval: 5,
      autoRunFeatureEnabled: true,
      weeklyAutoRunConfig,
      postBreakTransitionPolicy: "start_next_work_automatically",
      notificationSoundMuted: true,
      notificationSoundVolumePercent: 35,
    });

    render(
      <SettingsTopPage
        openHomeScreen={() => undefined}
        openTimerSettings={() => undefined}
        openAutoRunSettings={() => undefined}
        openNotificationSettings={() => undefined}
      />,
    );

    expect(
      screen.getByRole("button", { name: /^タイマー設定/u }),
    ).toHaveTextContent("作業30分 / 短休憩6分 / 長休憩20分 / 間隔5回");
    expect(
      screen.getByRole("button", { name: /^曜日別自動実行/u }),
    ).toHaveTextContent("月火 09:00-18:00");
    expect(
      screen.getByText("休憩後に自動で次の作業を開始する"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^通知/u })).toHaveTextContent(
      "ミュートON / 音量35%",
    );
  });

  it("focuses the settings top heading on mount", async () => {
    render(
      <SettingsTopPage
        openHomeScreen={() => undefined}
        openTimerSettings={() => undefined}
        openAutoRunSettings={() => undefined}
        openNotificationSettings={() => undefined}
      />,
    );

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "設定" })).toHaveFocus();
    });
  });

  it("renders the shared back button on settings top and calls home navigation", async () => {
    const user = userEvent.setup();
    const openHomeScreen = vi.fn();

    render(
      <SettingsTopPage
        openHomeScreen={openHomeScreen}
        openTimerSettings={() => undefined}
        openAutoRunSettings={() => undefined}
        openNotificationSettings={() => undefined}
      />,
    );

    const backButton = screen.getByRole("button", { name: "ホームへ戻る" });

    await user.click(backButton);

    expect(openHomeScreen).toHaveBeenCalledTimes(1);
  });

  it("shows compact auto-run summary when enabled weekdays have multiple windows", () => {
    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    weeklyAutoRunConfig.mon.enabled = true;
    weeklyAutoRunConfig.mon.startTime = "09:00";
    weeklyAutoRunConfig.mon.endTime = "18:00";
    weeklyAutoRunConfig.tue.enabled = true;
    weeklyAutoRunConfig.tue.startTime = "10:00";
    weeklyAutoRunConfig.tue.endTime = "19:00";

    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      weeklyAutoRunConfig,
    });

    render(
      <SettingsTopPage
        openHomeScreen={() => undefined}
        openTimerSettings={() => undefined}
        openAutoRunSettings={() => undefined}
        openNotificationSettings={() => undefined}
      />,
    );

    expect(
      screen.getByRole("button", { name: /^曜日別自動実行/u }),
    ).toHaveTextContent("有効2日（詳細参照）");
  });

  it("switches post-break transition policy on settings top", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      postBreakTransitionPolicy: "require_manual_continue",
    });

    render(
      <SettingsTopPage
        openHomeScreen={() => undefined}
        openTimerSettings={() => undefined}
        openAutoRunSettings={() => undefined}
        openNotificationSettings={() => undefined}
      />,
    );

    expect(screen.getByText("毎回確認する")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "自動継続をONにする" }),
    );

    expect(useSettingsStore.getState().postBreakTransitionPolicy).toBe(
      "start_next_work_automatically",
    );
    expect(
      screen.getByRole("button", { name: "毎回確認に戻す" }),
    ).toHaveAttribute("aria-pressed", "true");
    expect(
      screen.getByText("休憩後に自動で次の作業を開始する"),
    ).toBeInTheDocument();
  });
});
