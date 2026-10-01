import { useTimerObservationStore } from "@/store/timerObservationStore";
import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import App from "./App";
import { resetTimerStoreForTest } from "./test/resetTimerStore";
import { createSettingsForTest } from "./test/settingsFixture";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

const IDLE_FULL_STATE = vi.hoisted(() => ({
  timer: {
    status: "idle",
    remaining_seconds: 1500,
    session_duration_secs: 1500,
    session_type: "work",
    current_cycle: 0,
    completed_pomodoros: 0,
    break_interaction_count: 0,
    away_during_break: false,
    post_break_decision_pending: false,
    boot_auto_start_offer: null,
    active_window_summary: null,
    settings: {
      work_duration_secs: 1500,
      short_break_duration_secs: 300,
      long_break_duration_secs: 900,
      long_break_interval: 4,
      post_break_transition_policy: "require_manual_continue",
      auto_run_feature_enabled: false,
      weekly_auto_run_config: {
        mon: { enabled: false, start_time: "09:00", end_time: "18:00" },
        tue: { enabled: false, start_time: "09:00", end_time: "18:00" },
        wed: { enabled: false, start_time: "09:00", end_time: "18:00" },
        thu: { enabled: false, start_time: "09:00", end_time: "18:00" },
        fri: { enabled: false, start_time: "09:00", end_time: "18:00" },
        sat: { enabled: false, start_time: "09:00", end_time: "18:00" },
        sun: { enabled: false, start_time: "09:00", end_time: "18:00" },
      },
      notification_sound_volume_percent: 50,
      notification_sound_muted: false,
    },
    settings_file_status: "ok",
  },
  effects: {
    revision: "0",
    break_lock: { target: "unlocked", last_attempt: null },
    last_timer_save: null,
  },
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi
    .fn()
    .mockResolvedValue({ status: "accepted", value: IDLE_FULL_STATE }),
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

const MONDAY_DAYTIME_WINDOW = {
  weekday_key: "mon",
  start_time: "09:00",
  end_time: "18:00",
  crosses_midnight: false,
} as const;

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
    resetTimerStoreForTest();
    useTimerObservationStore.setState({
      settings: createSettingsForTest(),
    });
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: IDLE_FULL_STATE,
    });
    vi.mocked(listen).mockResolvedValue(vi.fn());
  });

  it("shows SCHEDULED when the backend reports an active auto-run window at connection", async () => {
    vi.mocked(invoke).mockImplementation((command) =>
      Promise.resolve({
        status: "accepted",
        value:
          command === "get_full_state"
            ? {
                ...IDLE_FULL_STATE,
                timer: {
                  ...IDLE_FULL_STATE.timer,
                  active_window_summary: MONDAY_DAYTIME_WINDOW,
                },
              }
            : undefined,
      }),
    );

    render(<App />);

    await waitFor(() => {
      expect(screen.getByTestId("header-auto-run")).toHaveTextContent(
        "SCHEDULED",
      );
    });
  });

  it("switches between SCHEDULED and MANUAL as the backend reports window changes", async () => {
    const backendEventHandlers = new Map<string, (event: unknown) => void>();
    vi.mocked(listen).mockImplementation((event, handler) => {
      backendEventHandlers.set(event, handler as (event: unknown) => void);
      return Promise.resolve(vi.fn());
    });

    render(<App />);
    expect(screen.queryByTestId("header-auto-run")).not.toBeInTheDocument();
    expect(await screen.findByTestId("header-auto-run")).toHaveTextContent(
      "MANUAL",
    );
    await waitFor(() => {
      expect(backendEventHandlers.has("active_window_changed")).toBe(true);
    });
    const emitActiveWindowChanged = (
      activeWindowSummary: typeof MONDAY_DAYTIME_WINDOW | null,
    ): void => {
      act(() => {
        backendEventHandlers.get("active_window_changed")?.({
          payload: { active_window_summary: activeWindowSummary },
        });
      });
    };

    emitActiveWindowChanged(MONDAY_DAYTIME_WINDOW);
    expect(screen.getByTestId("header-auto-run")).toHaveTextContent(
      "SCHEDULED",
    );

    emitActiveWindowChanged(null);
    expect(screen.getByTestId("header-auto-run")).toHaveTextContent("MANUAL");
  });

  it("opens settings top screen when settings button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();
    expect(screen.queryByTestId("timer-display")).not.toBeInTheDocument();
  });

  it("returns to home screen when the settings top header back button is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);
    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(screen.getByTestId("timer-display")).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "設定" }),
    ).not.toBeInTheDocument();
  });

  it("returns to home screen when Escape is pressed on settings top page", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    fireEvent.keyDown(window, { key: "Escape" });

    expect(screen.getByTestId("timer-display")).toBeInTheDocument();
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

  it("keeps the unreadable-settings notice visible across every settings route", async () => {
    const user = userEvent.setup();
    vi.mocked(invoke).mockResolvedValue({
      status: "accepted",
      value: {
        ...IDLE_FULL_STATE,
        timer: { ...IDLE_FULL_STATE.timer, settings_file_status: "unusable" },
      },
    });

    render(<App />);

    const expectSettingsFileNotice = async (): Promise<void> => {
      expect(await screen.findByRole("status")).toHaveTextContent(
        /保存済みの設定を読み込めなかった/u,
      );
    };

    await expectSettingsFileNotice();
    await openSettingsTop(user);
    await expectSettingsFileNotice();

    await user.click(
      screen.getByRole("button", { name: settingsTopTimerButtonName }),
    );
    await expectSettingsFileNotice();
    await user.click(screen.getByRole("button", { name: "設定一覧へ戻る" }));

    await user.click(
      screen.getByRole("button", { name: settingsTopAutoRunButtonName }),
    );
    await expectSettingsFileNotice();
    await user.click(screen.getByRole("button", { name: "設定一覧へ戻る" }));

    await user.click(
      screen.getByRole("button", { name: settingsTopNotificationButtonName }),
    );
    await expectSettingsFileNotice();
  });

  it("saves the flipped mute setting when the home mute toggle is clicked", async () => {
    const user = userEvent.setup();

    render(<App />);
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith("get_full_state");
    });

    await user.click(screen.getByRole("button", { name: "通知音" }));

    expect(vi.mocked(invoke)).toHaveBeenCalledWith("update_settings", {
      changes: { notification_sound_muted: true },
    });
  });

  it("returns to home screen after fullscreen break ends while settings top is open", async () => {
    const user = userEvent.setup();

    render(<App />);
    await openSettingsTop(user);

    expect(screen.getByRole("heading", { name: "設定" })).toBeInTheDocument();

    act(() => {
      useTimerObservationStore.getState().observeEffects({
        revision: "1",
        last_timer_save: null,
        break_lock: {
          target: "locked",
          last_attempt: { action: "enter", status: "succeeded" },
        },
      });
    });
    await waitFor(() => {
      expect(screen.getByText("BREAK")).toBeInTheDocument();
    });

    act(() => {
      useTimerObservationStore.getState().observeEffects({
        revision: "2",
        last_timer_save: null,
        break_lock: {
          target: "unlocked",
          last_attempt: { action: "release", status: "succeeded" },
        },
      });
    });
    await waitFor(() => {
      expect(screen.getByTestId("timer-display")).toBeInTheDocument();
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
