import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
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

  it("shows auto-run badge when scheduled auto-run is enabled", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "require_manual_continue",
    });

    render(<App />);

    expect(screen.getByTestId("header-badge-auto-run")).toBeInTheDocument();
    expect(screen.queryByTestId("header-badge-auto-continue")).not.toBeInTheDocument();
  });

  it("shows auto-continue badge when post-break auto-continue is enabled", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: false,
      post_break_transition_policy: "start_next_work_automatically",
    });

    render(<App />);

    expect(screen.getByTestId("header-badge-auto-continue")).toBeInTheDocument();
    expect(screen.queryByTestId("header-badge-auto-run")).not.toBeInTheDocument();
  });

  it("shows both badges without omission and keeps fixed order", () => {
    useSettingsStore.setState({
      autoRunFeatureEnabled: true,
      post_break_transition_policy: "start_next_work_automatically",
    });

    render(<App />);

    const badgeContainer = screen.getByTestId("header-status-badges");
    const badges = badgeContainer.querySelectorAll('[data-testid^="header-badge-"]');
    const badgeIds = Array.from(badges).map((badgeElement) =>
      badgeElement.getAttribute("data-testid")
    );

    expect(badgeIds).toEqual([
      "header-badge-auto-run",
      "header-badge-auto-continue",
    ]);
  });
});

