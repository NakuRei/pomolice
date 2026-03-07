import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import App from "./App";
import {
  createWeeklyAutoRunConfigAllDaysDisabled,
  useSettingsStore,
} from "./store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "./store/timerStore";
import { playNotificationSoundPreview } from "./services/notifications";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    status: "idle",
    remaining_seconds: 1500,
    session_type: "work",
    current_cycle: 0,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    is_fullscreen_break: false,
    post_break_decision_pending: false,
    should_prompt_start_on_boot: false,
    active_window_summary: null,
    settings: {
      work_duration_secs: 1500,
      short_break_duration_secs: 300,
      long_break_duration_secs: 900,
      long_break_interval: 4,
      post_break_transition_policy: "require_manual_continue",
    },
  }),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("./services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(async () => ({ status: "played" })),
}));

describe("App", () => {
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

  it("renders without crashing", () => {
    render(<App />);
  });

  it("displays 'Pomolice' title", () => {
    render(<App />);
    const heading = screen.getByText(/^Pomolice$/i);
    expect(heading).toBeInTheDocument();
  });

  it("renders timer display", () => {
    render(<App />);
    const timerDisplay = screen.getByTestId("timer-display");
    expect(timerDisplay).toBeInTheDocument();
  });

  it("renders start button", () => {
    render(<App />);
    const startButton = screen.getByTestId("start-button");
    expect(startButton).toBeInTheDocument();
  });

  it("toggles notification mute state when mute button is clicked", async () => {
    const user = userEvent.setup();
    useSettingsStore.setState({ notificationSoundMuted: false });

    render(<App />);

    const muteButton = screen.getByTestId("notification-mute-button");
    await user.click(muteButton);

    expect(useSettingsStore.getState().notificationSoundMuted).toBe(true);
  });

  it("shows auto-run badge when scheduled auto-run is enabled", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);

    expect(screen.getByTestId("header-badge-auto-run")).toBeInTheDocument();
    expect(
      screen.queryByTestId("header-badge-auto-continue"),
    ).not.toBeInTheDocument();
  });

  it("shows auto-continue badge when post-break auto-continue is enabled", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: false,
      post_break_transition_policy: "start_next_work_automatically",
    });

    render(<App />);

    expect(
      screen.getByTestId("header-badge-auto-continue"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("header-badge-auto-run"),
    ).not.toBeInTheDocument();
  });

  it("shows both badges without omission and keeps fixed order", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "start_next_work_automatically",
    });

    render(<App />);

    const badgeContainer = screen.getByTestId("header-status-badges");
    const badges = badgeContainer.querySelectorAll(
      '[data-testid^="header-badge-"]',
    );
    const badgeIds = Array.from(badges).map((badgeElement) =>
      badgeElement.getAttribute("data-testid"),
    );

    expect(badgeIds).toEqual([
      "header-badge-auto-run",
      "header-badge-auto-continue",
    ]);
  });

  it("opens settings top screen when settings button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("timer-display")).not.toBeInTheDocument();
  });

  it("opens timer settings page from settings top", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    expect(screen.getByTestId("settings-timer-page")).toBeInTheDocument();
    expect(
      screen.getByTestId("settings-work-duration-input"),
    ).toBeInTheDocument();
  });

  it("opens auto-run settings page from settings top", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    expect(screen.getByTestId("settings-auto-run-page")).toBeInTheDocument();
    expect(
      screen.getByTestId("settings-auto-run-feature-toggle"),
    ).toBeInTheDocument();
  });

  it("opens notification settings page from settings top", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));

    expect(
      screen.getByTestId("settings-notification-page"),
    ).toBeInTheDocument();
    expect(
      screen.getByTestId("settings-notification-sound-volume"),
    ).toBeInTheDocument();
  });

  it("displays current settings summary text on settings top", async () => {
    const user = userEvent.setup();
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

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));

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

  it("switches post-break transition policy on settings top", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));

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

  it("returns to home screen after fullscreen break ends while settings top is open", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });
    await waitFor(() => {
      expect(screen.getByTestId("fullscreen-break")).toBeInTheDocument();
    });

    act(() => {
      useTimerStore.setState({ isFullscreenBreak: false });
    });
    await waitFor(() => {
      expect(screen.getByTestId("timer-display")).toBeInTheDocument();
    });
    expect(screen.queryByTestId("settings-top-page")).not.toBeInTheDocument();
  });

  it("saves timer settings and returns to settings top", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    const workDurationInput = screen.getByTestId(
      "settings-work-duration-input",
    ) as HTMLInputElement;
    await user.clear(workDurationInput);
    await user.type(workDurationInput, "31");
    await user.click(screen.getByTestId("settings-save"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().workDuration).toBe(31);
    expect(useSettingsStore.getState().post_break_transition_policy).toBe(
      "require_manual_continue",
    );
  });

  it("saves auto-run settings and returns to settings top", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "09:00" },
    });
    fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
      target: { value: "18:00" },
    });
    await user.click(screen.getByTestId("settings-save"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(true);
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.enabled).toBe(
      true,
    );
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.startTime).toBe(
      "09:00",
    );
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.endTime).toBe(
      "18:00",
    );
  });

  it("saves notification settings and only updates notification keys", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      workDuration: 28,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));

    await user.click(screen.getByTestId("settings-notification-mute-button"));
    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    ) as HTMLInputElement;
    await user.click(screen.getByTestId("settings-notification-mute-button"));
    fireEvent.change(volumeSlider, { target: { value: "35" } });
    await user.click(screen.getByTestId("settings-save"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(35);
    expect(useSettingsStore.getState().workDuration).toBe(28);
  });

  it("does not persist unsaved timer draft when saving notification settings", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      workDuration: 25,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    const workDurationInput = screen.getByTestId(
      "settings-work-duration-input",
    ) as HTMLInputElement;
    await user.clear(workDurationInput);
    await user.type(workDurationInput, "40");
    await user.click(screen.getByTestId("settings-cancel"));

    await user.click(screen.getByTestId("settings-top-row-notification"));
    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    ) as HTMLInputElement;
    fireEvent.change(volumeSlider, { target: { value: "30" } });
    await user.click(screen.getByTestId("settings-save"));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(30);
  });

  it("cancels timer settings without persisting edits", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({ workDuration: 25 });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    const workDurationInput = screen.getByTestId(
      "settings-work-duration-input",
    ) as HTMLInputElement;
    await user.clear(workDurationInput);
    await user.type(workDurationInput, "45");
    await user.click(screen.getByTestId("settings-cancel"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().workDuration).toBe(25);
  });

  it("cancels auto-run settings without persisting edits", async () => {
    const user = userEvent.setup();

    const weeklyAutoRunConfig = createWeeklyAutoRunConfigAllDaysDisabled();
    useSettingsStore.setState({
      autoRunFeatureEnabled: false,
      weeklyAutoRunConfig,
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "10:00" },
    });
    fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
      target: { value: "19:00" },
    });
    await user.click(screen.getByTestId("settings-cancel"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().autoRunFeatureEnabled).toBe(false);
    expect(useSettingsStore.getState().weeklyAutoRunConfig.mon.enabled).toBe(
      false,
    );
  });

  it("cancels notification settings without persisting edits", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));

    await user.click(screen.getByTestId("settings-notification-mute-button"));
    const volumeSlider = screen.getByTestId(
      "settings-notification-sound-volume",
    ) as HTMLInputElement;
    await user.click(screen.getByTestId("settings-notification-mute-button"));
    fireEvent.change(volumeSlider, { target: { value: "80" } });
    await user.click(screen.getByTestId("settings-cancel"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(50);
  });

  it("disables timer duration inputs while timer is running", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    act(() => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
    });

    expect(screen.getByTestId("settings-work-duration-input")).toBeDisabled();
    expect(screen.getByTestId("settings-short-break-input")).toBeDisabled();
    expect(screen.getByTestId("settings-long-break-input")).toBeDisabled();
    expect(screen.getByTestId("settings-interval-input")).toBeDisabled();
    expect(
      screen.getByTestId("settings-duration-edit-disabled-message"),
    ).toBeInTheDocument();
  });

  it("returns to settings top when Escape is pressed on timer settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("settings-timer-page")).not.toBeInTheDocument();
  });

  it("shows day validation error and disables save in auto-run settings", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    await user.click(screen.getByTestId("settings-auto-run-feature-toggle"));
    await user.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
    fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
      target: { value: "invalid-time" },
    });

    expect(
      screen.getByTestId("settings-auto-run-day-error-mon"),
    ).toBeInTheDocument();
    expect(screen.getByTestId("settings-save")).toBeDisabled();
  });

  it("returns to settings top when Escape is pressed on auto-run settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-auto-run-page"),
    ).not.toBeInTheDocument();
  });

  it("disables notification controls when mute is enabled", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));
    await user.click(screen.getByTestId("settings-notification-mute-button"));

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

  it("shows unavailable message when preview sound file is not available", async () => {
    const user = userEvent.setup();
    vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({
      status: "unavailable",
    });

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));
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

  it("returns to settings top when Escape is pressed on notification settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-notification-page"),
    ).not.toBeInTheDocument();
  });
});
