import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import App from "./App";
import { useSettingsStore } from "./store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "./store/timerStore";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
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
  playNotificationSoundPreview: vi.fn(() =>
    Promise.resolve({ status: "played" }),
  ),
}));

const settingsTopTimerButtonName = /^タイマー設定/u;
const settingsTopAutoRunButtonName = /^曜日別自動実行/u;
const settingsTopNotificationButtonName = /^通知.*音量/u;

async function openSettingsTop(user: UserEvent): Promise<void> {
  await user.click(screen.getByRole("button", { name: "設定を開く" }));
}

async function openTimerSettings(user: UserEvent): Promise<void> {
  await openSettingsTop(user);
  await user.click(
    screen.getByRole("button", { name: settingsTopTimerButtonName }),
  );
}

async function openAutoRunSettings(user: UserEvent): Promise<void> {
  await openSettingsTop(user);
  await user.click(
    screen.getByRole("button", { name: settingsTopAutoRunButtonName }),
  );
}

async function openNotificationSettings(user: UserEvent): Promise<void> {
  await openSettingsTop(user);
  await user.click(
    screen.getByRole("button", { name: settingsTopNotificationButtonName }),
  );
}

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

  it("opens settings top screen when settings button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Pomolice" }),
    ).not.toBeInTheDocument();
  });

  it("returns to home screen when the settings top header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);
    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(
      screen.getByRole("heading", { name: "Pomolice" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "設定" }),
    ).not.toBeInTheDocument();
  });

  it("returns to home screen when Escape is pressed on settings top page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    fireEvent.keyDown(window, { key: "Escape" });

    expect(
      screen.getByRole("heading", { name: "Pomolice" }),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "設定" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    {
      pageName: "timer",
      openPage: openTimerSettings,
      headingName: "タイマー設定",
      controlRole: "spinbutton" as const,
      controlName: "作業時間",
    },
    {
      pageName: "auto-run",
      openPage: openAutoRunSettings,
      headingName: "曜日別自動実行設定",
      controlRole: "checkbox" as const,
      controlName: "曜日別自動実行の全体切り替え",
    },
    {
      pageName: "notification",
      openPage: openNotificationSettings,
      headingName: "通知設定",
      controlRole: "slider" as const,
      controlName: "通知音量",
    },
  ])(
    "opens $pageName settings page from settings top",
    async ({ openPage, headingName, controlRole, controlName }) => {
      const user = userEvent.setup();

      render(<App />);
      await openPage(user);

      expect(
        screen.getByRole("heading", { name: headingName }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole(controlRole, { name: controlName }),
      ).toBeInTheDocument();
    },
  );

  it("returns to settings top after saving timer settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openTimerSettings(user);

    const workDurationInput = screen.getByRole("spinbutton", {
      name: "作業時間",
    });
    await user.clear(workDurationInput);
    await user.type(workDurationInput, "31");
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "タイマー設定" }),
    ).not.toBeInTheDocument();
  });

  it("returns to settings top after canceling auto-run settings page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openAutoRunSettings(user);

    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "曜日別自動実行設定" }),
    ).not.toBeInTheDocument();
  });

  it("returns to settings top when the notification header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openNotificationSettings(user);
    await user.click(screen.getByRole("button", { name: "設定一覧へ戻る" }));

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "通知設定" }),
    ).not.toBeInTheDocument();
  });

  it("returns to home screen after fullscreen break ends while settings top is open", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });
    await waitFor(() => {
      expect(screen.getByText("休憩")).toBeInTheDocument();
    });

    act(() => {
      useTimerStore.setState({ isFullscreenBreak: false });
    });
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: "Pomolice" }),
      ).toBeInTheDocument();
    });
    expect(
      screen.queryByRole("heading", { name: "設定" }),
    ).not.toBeInTheDocument();
  });

  it.each([
    {
      pageName: "timer",
      openPage: openTimerSettings,
      headingName: "タイマー設定",
    },
    {
      pageName: "auto-run",
      openPage: openAutoRunSettings,
      headingName: "曜日別自動実行設定",
    },
    {
      pageName: "notification",
      openPage: openNotificationSettings,
      headingName: "通知設定",
    },
  ])(
    "returns to settings top when Escape is pressed on $pageName settings page",
    async ({ openPage, headingName }) => {
      const user = userEvent.setup();

      render(<App />);
      await openPage(user);

      fireEvent.keyDown(window, { key: "Escape" });

      expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
      expect(
        screen.queryByRole("heading", { name: headingName }),
      ).not.toBeInTheDocument();
    },
  );
});
