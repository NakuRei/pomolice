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
import { useTimerStore } from "./store/timerStore";
import { resetTimerStoreForTest } from "./test/resetTimerStore";

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
    useSettingsStore.setState({
      durationSettingsApplicationStatus: "confirmed",
      automationSettingsApplicationStatus: "confirmed",
    });
    resetTimerStoreForTest();
    useTimerStore.setState({ isConnected: true });
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
    "opens $pageName settings page from settings top",
    async ({ openPage, headingName }) => {
      const user = userEvent.setup();

      render(<App />);
      await openPage(user);

      expect(
        screen.getByRole("heading", { name: headingName }),
      ).toBeInTheDocument();
    },
  );

  it("keeps unresolved settings visible across every settings route", async () => {
    const user = userEvent.setup();
    useTimerStore.setState({ isConnected: false });

    render(<App />);

    await waitFor(() => {
      expect(useTimerStore.getState().isConnected).toBe(true);
    });
    act(() => {
      useSettingsStore.setState({
        durationSettingsApplicationStatus: "pending",
        automationSettingsApplicationStatus: "confirmed",
      });
    });

    const expectUnresolvedSettingsNotice = (): void => {
      expect(screen.getByRole("status")).toHaveTextContent(
        /適用はまだ確認されていません/u,
      );
    };

    expectUnresolvedSettingsNotice();
    await openSettingsTop(user);
    expectUnresolvedSettingsNotice();

    await user.click(
      screen.getByRole("button", { name: settingsTopTimerButtonName }),
    );
    expectUnresolvedSettingsNotice();
    await user.click(screen.getByRole("button", { name: "設定一覧へ戻る" }));

    await user.click(
      screen.getByRole("button", { name: settingsTopAutoRunButtonName }),
    );
    expectUnresolvedSettingsNotice();
    await user.click(screen.getByRole("button", { name: "設定一覧へ戻る" }));

    await user.click(
      screen.getByRole("button", { name: settingsTopNotificationButtonName }),
    );
    expectUnresolvedSettingsNotice();
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
