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
import { useSettingsStore } from "./store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "./store/timerStore";

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

  it("shows manual-continue badge when post-break policy requires confirmation", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: false,
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);

    expect(
      screen.getByTestId("header-badge-require-manual-continue"),
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId("header-badge-auto-continue"),
    ).not.toBeInTheDocument();
    expect(screen.queryByTestId("header-badge-auto-run")).not.toBeInTheDocument();
  });

  it("shows auto-run badge together with manual-continue badge when scheduled auto-run is enabled", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);

    expect(screen.getByTestId("header-badge-auto-run")).toBeInTheDocument();
    expect(
      screen.getByTestId("header-badge-require-manual-continue"),
    ).toBeInTheDocument();
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
    expect(
      screen.queryByTestId("header-badge-require-manual-continue"),
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

  it("shows auto-run badge before manual-continue badge", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "require_manual_continue",
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
      "header-badge-require-manual-continue",
    ]);
  });

  it("opens settings top screen when settings button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("timer-display")).not.toBeInTheDocument();
  });

  it("returns to home screen when the settings top header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-back-home"));

    expect(screen.getByTestId("timer-display")).toBeInTheDocument();
    expect(screen.queryByTestId("settings-top-page")).not.toBeInTheDocument();
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

  it("returns to settings top after saving timer settings page", async () => {
    const user = userEvent.setup();

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
    expect(screen.queryByTestId("settings-timer-page")).not.toBeInTheDocument();
  });

  it("returns to settings top when the timer header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));
    await user.click(screen.getByTestId("settings-timer-back"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("settings-timer-page")).not.toBeInTheDocument();
  });

  it("returns to settings top after canceling auto-run settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));

    await user.click(screen.getByTestId("settings-cancel"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("settings-auto-run-page")).not.toBeInTheDocument();
  });

  it("returns to settings top when the auto-run header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-auto-run"));
    await user.click(screen.getByTestId("settings-auto-run-back"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-auto-run-page"),
    ).not.toBeInTheDocument();
  });

  it("returns to settings top after saving notification settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));

    fireEvent.change(screen.getByTestId("settings-notification-sound-volume"), {
      target: { value: "40" },
    });
    await user.click(screen.getByTestId("settings-save"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-notification-page"),
    ).not.toBeInTheDocument();
  });

  it("returns to settings top when the notification header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-notification"));
    await user.click(screen.getByTestId("settings-notification-back"));

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(
      screen.queryByTestId("settings-notification-page"),
    ).not.toBeInTheDocument();
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

  it("returns to settings top when Escape is pressed on timer settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await user.click(screen.getByTestId("settings-button"));
    await user.click(screen.getByTestId("settings-top-row-timer"));

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("settings-top-page")).toBeInTheDocument();
    expect(screen.queryByTestId("settings-timer-page")).not.toBeInTheDocument();
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
