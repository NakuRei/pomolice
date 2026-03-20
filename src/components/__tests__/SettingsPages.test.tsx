import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
} from "../../store/settingsStore";
import {
  SessionType,
  TimerStatus,
  useTimerStore,
} from "../../store/timerStore";
import { SettingsTopPage } from "../Settings/SettingsTopPage";
import { SettingsTimerPage } from "../Settings/SettingsTimerPage";
import { SettingsAutoRunPage } from "../Settings/SettingsAutoRunPage";
import { SettingsNotificationPage } from "../Settings/SettingsNotificationPage";
import { playNotificationSoundPreview } from "../../services/notifications";

vi.mock("../../services/notifications", () => ({
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
      post_break_transition_policy: "start_next_work_automatically",
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

    expect(screen.getByTestId("settings-top-row-timer")).toHaveTextContent(
      "作業30分 / 短休憩6分 / 長休憩20分 / 間隔5回",
    );
    expect(screen.getByTestId("settings-top-row-auto-run")).toHaveTextContent(
      "月火 09:00-18:00",
    );
    expect(
      screen.getByTestId("settings-top-row-post-break-policy"),
    ).toHaveTextContent("休憩後に自動で次の作業を開始する");
    expect(
      screen.getByTestId("settings-top-row-notification"),
    ).toHaveTextContent("ミュートON / 音量35%");
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
      expect(screen.getByTestId("settings-top-heading")).toHaveFocus();
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

    const backButton = screen.getByTestId("settings-top-back-home");
    expect(backButton).toHaveAccessibleName("ホームへ戻る");

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

    expect(screen.getByTestId("settings-top-row-auto-run")).toHaveTextContent(
      "有効2日（詳細参照）",
    );
  });

  it("switches post-break transition policy on settings top", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      post_break_transition_policy: "require_manual_continue",
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
      screen.getByTestId("settings-top-row-post-break-policy"),
    ).toHaveTextContent("毎回確認する");

    await user.click(
      screen.getByTestId("settings-top-post-break-policy-toggle"),
    );

    expect(useSettingsStore.getState().post_break_transition_policy).toBe(
      "start_next_work_automatically",
    );
    expect(
      screen.getByTestId("settings-top-row-post-break-policy"),
    ).toHaveTextContent("休憩後に自動で次の作業を開始する");
  });

  it("saves timer settings and keeps unrelated settings unchanged", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      workDuration: 25,
      autoRunFeatureEnabled: true,
      notificationSoundVolumePercent: 50,
      post_break_transition_policy: "require_manual_continue",
    });

    render(<SettingsTimerPage openSettingsTopScreen={openSettingsTopScreen} />);

    const workInput = screen.getByTestId("settings-work-duration-input");
    await user.clear(workInput);
    await user.type(workInput, "32");
    await user.click(screen.getByTestId("settings-save"));

    expect(useSettingsStore.getState().workDuration).toBe(32);
    expect(useSettingsStore.getState().post_break_transition_policy).toBe(
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

    const workInput = screen.getByTestId("settings-work-duration-input");
    await user.clear(workInput);
    await user.type(workInput, "44");
    await user.click(screen.getByTestId("settings-cancel"));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("blocks timer save when long break interval draft is not an integer", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    render(<SettingsTimerPage openSettingsTopScreen={openSettingsTopScreen} />);

    const intervalInput = screen.getByTestId("settings-interval-input");
    await user.clear(intervalInput);
    await user.type(intervalInput, "2.5");

    expect(screen.getByTestId("settings-save")).toBeDisabled();

    await user.click(screen.getByTestId("settings-save"));

    expect(useSettingsStore.getState().longBreakInterval).toBe(4);
    expect(openSettingsTopScreen).not.toHaveBeenCalled();
  });

  it("disables timer duration inputs and keeps existing warning when timer is running", () => {
    useTimerStore.setState({ status: TimerStatus.WORKING });

    render(<SettingsTimerPage openSettingsTopScreen={() => undefined} />);

    expect(screen.getByTestId("settings-timer-footer")).toBeInTheDocument();
    expect(screen.getByTestId("settings-work-duration-input")).toBeDisabled();
    expect(screen.getByTestId("settings-short-break-input")).toBeDisabled();
    expect(screen.getByTestId("settings-long-break-input")).toBeDisabled();
    expect(screen.getByTestId("settings-interval-input")).toBeDisabled();
    expect(
      screen.getByTestId("settings-duration-edit-disabled-message"),
    ).toBeInTheDocument();
  });

  it("focuses the timer heading on mount", async () => {
    render(<SettingsTimerPage openSettingsTopScreen={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByTestId("settings-timer-heading")).toHaveFocus();
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

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "09:00" },
    });
    fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
      target: { value: "18:00" },
    });
    await user.click(screen.getByTestId("settings-save"));

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

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
      target: { value: "19:00" },
    });
    await user.click(screen.getByTestId("settings-cancel"));

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

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "invalid-time" },
    });

    expect(
      screen.getByTestId("settings-auto-run-day-error-mon"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("settings-auto-run-footer")).toBeInTheDocument();
    expect(screen.getByTestId("settings-save")).toBeDisabled();
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

    const weeklyErrorNotice = screen.getByTestId(
      "settings-auto-run-weekly-errors",
    );

    expect(weeklyErrorNotice).toHaveAttribute("role", "alert");
    expect(weeklyErrorNotice).toHaveTextContent(
      "Schedule windows overlap between mon and tue.",
    );
  });

  it("focuses the auto-run heading on mount", async () => {
    render(<SettingsAutoRunPage openSettingsTopScreen={() => undefined} />);

    await waitFor(() => {
      expect(screen.getByTestId("settings-auto-run-heading")).toHaveFocus();
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

    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    );
    fireEvent.change(volumeSlider, { target: { value: "35" } });
    await user.click(screen.getByTestId("settings-save"));

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

    await user.click(screen.getByTestId("settings-notification-mute-button"));
    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    );
    await user.click(screen.getByTestId("settings-notification-mute-button"));
    fireEvent.change(volumeSlider, { target: { value: "80" } });
    await user.click(screen.getByTestId("settings-cancel"));

    expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(50);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("disables volume and preview button when notification is muted", async () => {
    const user = userEvent.setup();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await user.click(screen.getByTestId("settings-notification-mute-button"));

    expect(
      screen.getByTestId("settings-notification-footer"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("settings-notification-sound-volume"),
    ).toBeDisabled();
    expect(
      screen.getByTestId("settings-notification-sound-preview-button"),
    ).toBeDisabled();
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveTextContent("通知音がミュート中のため再生しませんでした。");
  });

  it("initializes notification preview ui state on each mount", async () => {
    const user = userEvent.setup();

    const firstRender = render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    await user.selectOptions(
      screen.getByTestId("settings-notification-sound-preview-type"),
      "long_break",
    );
    await user.click(screen.getByTestId("settings-notification-mute-button"));
    firstRender.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    const previewTypeSelect = screen.getByTestId(
      "settings-notification-sound-preview-type",
    );
    expect(previewTypeSelect).toHaveValue("work");
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
      screen.getByTestId("settings-notification-sound-preview-button"),
    );

    expect(playNotificationSoundPreview).toHaveBeenCalledTimes(1);
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveTextContent(
      "通知音ファイルを読み込めなかったため再生できませんでした。",
    );
  });

  it("focuses the notification heading on mount", async () => {
    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await waitFor(() => {
      expect(screen.getByTestId("settings-notification-heading")).toHaveFocus();
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
    const workInput = screen.getByTestId("settings-work-duration-input");
    await user.clear(workInput);
    await user.type(workInput, "40");
    await user.click(screen.getByTestId("settings-cancel"));
    timerPageRenderResult.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    );
    fireEvent.change(volumeSlider, { target: { value: "30" } });
    await user.click(screen.getByTestId("settings-save"));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(30);
  });
});
