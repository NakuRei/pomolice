import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
} from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";
import { SettingsTopPage } from "./SettingsTopPage";
import { SettingsTimerPage } from "./SettingsTimerPage";
import { SettingsAutoRunPage } from "./SettingsAutoRunPage";
import { SettingsNotificationPage } from "./SettingsNotificationPage";
import { playNotificationSoundPreview } from "@/services/notifications";

vi.mock("@/services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(() =>
    Promise.resolve({ status: "played" }),
  ),
}));

describe("Settings Pages", () => {
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
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
    vi.mocked(playNotificationSoundPreview).mockResolvedValue({
      status: "played",
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

    await user.click(
      screen.getByRole("checkbox", { name: "曜日別自動実行の全体切り替え" }),
    );
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

    await user.click(
      screen.getByRole("checkbox", { name: "曜日別自動実行の全体切り替え" }),
    );
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

    await user.click(
      screen.getByRole("checkbox", { name: "曜日別自動実行の全体切り替え" }),
    );
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

  it("saves notification settings and keeps timer settings unchanged", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      workDuration: 27,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(
      <SettingsNotificationPage
        openSettingsTopScreen={openSettingsTopScreen}
      />,
    );

    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    fireEvent.change(volumeSlider, { target: { value: "35" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(35);
    expect(useSettingsStore.getState().workDuration).toBe(27);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("discards notification draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(
      <SettingsNotificationPage
        openSettingsTopScreen={openSettingsTopScreen}
      />,
    );

    await user.click(screen.getByRole("button", { name: "通知音" }));
    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    await user.click(screen.getByRole("button", { name: "通知音" }));
    fireEvent.change(volumeSlider, { target: { value: "80" } });
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(50);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("disables volume and preview button when notification is muted", async () => {
    const user = userEvent.setup();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await user.click(screen.getByRole("button", { name: "通知音" }));

    expect(screen.getByRole("slider", { name: "通知音量" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    ).toBeDisabled();
    expect(
      screen.getByText("通知音がミュート中のため再生しませんでした。"),
    ).toBeInTheDocument();
  });

  it("initializes notification preview ui state on each mount", async () => {
    const user = userEvent.setup();

    const firstRender = render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "テストする通知音の種類" }),
      "long_break",
    );
    await user.click(screen.getByRole("button", { name: "通知音" }));
    firstRender.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    const previewTypeSelect = screen.getByRole("combobox", {
      name: "テストする通知音の種類",
    });
    expect(previewTypeSelect).toHaveValue("work");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveAttribute("aria-live", "polite");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveTextContent("");
  });

  it("shows unavailable message when preview sound file is not available", async () => {
    const user = userEvent.setup();
    vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({
      status: "unavailable",
    });

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await user.click(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    );

    expect(playNotificationSoundPreview).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText(
        "通知音ファイルを読み込めなかったため再生できませんでした。",
      ),
    ).toBeInTheDocument();
  });

  it("focuses the notification heading on mount", async () => {
    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "通知設定" })).toHaveFocus();
    });
  });

  it("does not persist unsaved timer draft when notification settings are saved", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      workDuration: 25,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    const timerPageRenderResult = render(
      <SettingsTimerPage openSettingsTopScreen={() => undefined} />,
    );
    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "40");
    await user.click(screen.getByRole("button", { name: "キャンセル" }));
    timerPageRenderResult.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    fireEvent.change(volumeSlider, { target: { value: "30" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(30);
  });
});
