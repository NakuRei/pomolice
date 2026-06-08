import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import {
  afterAll,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { AppRoot } from "../../AppRoot";
import type * as FullscreenBreakModule from "../../components/BreakScreen/FullscreenBreak";
import type * as HomeScreenModule from "../../components/Home/HomeScreen";
import type * as SettingsTimerPageModule from "../../components/Settings/SettingsTimerPage";
import { FRONTEND_LOG_EVENT_IDS } from "../../logging/logEvents";
import { useSettingsStore } from "../../store/settingsStore";
import {
  SessionType,
  TimerStatus,
  useTimerStore,
} from "../../store/timerStore";

const { crash, errorLoggerMock, infoLoggerMock } = vi.hoisted(() => ({
  crash: {
    fullscreen: false,
    home: false,
    settingsTimer: false,
  },
  errorLoggerMock: vi.fn(),
  infoLoggerMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue({
    active_window_summary: null,
    break_interaction_count: 0,
    completed_pomodoros: 0,
    current_cycle: 0,
    is_fullscreen_break: false,
    post_break_decision_pending: false,
    remaining_seconds: 1500,
    session_type: "work",
    settings: {
      long_break_duration_secs: 900,
      long_break_interval: 4,
      post_break_transition_policy: "require_manual_continue",
      short_break_duration_secs: 300,
      work_duration_secs: 1500,
    },
    should_prompt_start_on_boot: false,
    status: "idle",
  }),
}));

vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"),
  sendNotification: vi.fn(),
}));

vi.mock("../../services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(() =>
    Promise.resolve({ status: "played" }),
  ),
}));

vi.mock("../../hooks/useTimerSync", () => ({
  useTimerSync: vi.fn(),
}));

vi.mock("../../logging/logger", () => ({
  createLogger: vi.fn(() => ({
    debug: vi.fn(),
    error: errorLoggerMock,
    info: infoLoggerMock,
    trace: vi.fn(),
    warn: vi.fn(),
  })),
}));

vi.mock("../../components/Home/HomeScreen", async (orig) => {
  const actual = await orig<typeof HomeScreenModule>();
  const { throwOnRender } = await import("../../test/throwOnRender");
  const Throwing = throwOnRender("home crash");
  const RealHomeScreen = actual.HomeScreen;

  function HomeScreen(
    props: React.ComponentProps<typeof RealHomeScreen>,
  ): React.ReactElement {
    return crash.home ? <Throwing /> : <RealHomeScreen {...props} />;
  }

  return { ...actual, HomeScreen };
});

vi.mock("../../components/BreakScreen/FullscreenBreak", async (orig) => {
  const actual = await orig<typeof FullscreenBreakModule>();
  const { throwOnRender } = await import("../../test/throwOnRender");
  const Throwing = throwOnRender("fullscreen crash");
  const RealFullscreenBreak = actual.FullscreenBreak;

  function FullscreenBreak(): React.ReactElement {
    return crash.fullscreen ? <Throwing /> : <RealFullscreenBreak />;
  }

  return { ...actual, FullscreenBreak };
});

vi.mock("../../components/Settings/SettingsTimerPage", async (orig) => {
  const actual = await orig<typeof SettingsTimerPageModule>();
  const { throwOnRender } = await import("../../test/throwOnRender");
  const Throwing = throwOnRender("settings timer crash");
  const RealSettingsTimerPage = actual.SettingsTimerPage;

  function SettingsTimerPage(
    props: React.ComponentProps<typeof RealSettingsTimerPage>,
  ): React.ReactElement {
    return crash.settingsTimer ? (
      <Throwing />
    ) : (
      <RealSettingsTimerPage {...props} />
    );
  }

  return { ...actual, SettingsTimerPage };
});

function resetCrashFlags(): void {
  crash.fullscreen = false;
  crash.home = false;
  crash.settingsTimer = false;
}

function caughtErrorPayloads(): Record<string, unknown>[] {
  return errorLoggerMock.mock.calls
    .filter(
      (callArguments) =>
        callArguments[0] ===
        FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
    )
    .map((callArguments) => callArguments[2] as Record<string, unknown>);
}

async function openSettingsTop(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "設定を開く" }));
}

async function openTimerSettings(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: /^タイマー設定/u }));
}

describe("ErrorBoundary integration", () => {
  beforeAll(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    errorLoggerMock.mockClear();
    infoLoggerMock.mockClear();
    resetCrashFlags();
    useSettingsStore.getState().reset();
    useTimerStore.setState({
      activeWindowSummary: null,
      breakInteractionCount: 0,
      completedPomodoros: 0,
      currentCycle: 0,
      infoMessage: null,
      infoMessageShownAtById: {},
      isConnected: true,
      isFullscreenBreak: false,
      remainingSeconds: 25 * 60,
      requiresManualContinueDecision: false,
      sessionType: SessionType.WORK,
      shouldPromptStartOnBoot: false,
      status: TimerStatus.IDLE,
    });
  });

  it("shows the fullscreen fallback and logs the fullscreen scope without a current_screen", async () => {
    crash.fullscreen = true;
    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });

    render(<AppRoot />);

    expect(
      await screen.findByText("表示エラーが発生しました"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("アプリケーションエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({ boundary_scope: "fullscreen_break" }),
    );
    const caughtPayloads = caughtErrorPayloads();
    expect(caughtPayloads.length).toBeGreaterThan(0);
    for (const caughtPayload of caughtPayloads) {
      expect(caughtPayload).not.toHaveProperty("current_screen");
    }
  });

  it("contains settings crashes within the settings boundary and logs the current settings screen", async () => {
    const user = userEvent.setup();

    render(<AppRoot />);
    await openSettingsTop(user);
    crash.settingsTimer = true;
    await openTimerSettings(user);

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("アプリケーションエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({
        boundary_scope: "settings",
        current_screen: "settingsTimer",
      }),
    );
  });

  it("shows the app fallback for home-screen crashes and logs the app scope without a current_screen", async () => {
    crash.home = true;

    render(<AppRoot />);

    expect(
      await screen.findByText("アプリケーションエラーが発生しました"),
    ).toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({ boundary_scope: "app" }),
    );
    const caughtPayloads = caughtErrorPayloads();
    expect(caughtPayloads.length).toBeGreaterThan(0);
    for (const caughtPayload of caughtPayloads) {
      expect(caughtPayload).not.toHaveProperty("current_screen");
    }
  });

  it("recovers the settings boundary and logs a reset once the crash is resolved", async () => {
    const user = userEvent.setup();

    render(<AppRoot />);
    await openSettingsTop(user);
    crash.settingsTimer = true;
    await openTimerSettings(user);

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();

    crash.settingsTimer = false;
    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(await screen.findByText("Pomolice")).toBeInTheDocument();
    expect(
      screen.queryByText("設定画面の表示中にエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });

  it("keeps the fallback when retrying while the crash still occurs", async () => {
    const user = userEvent.setup();
    crash.fullscreen = true;
    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });

    render(<AppRoot />);

    expect(
      await screen.findByText("表示エラーが発生しました"),
    ).toBeInTheDocument();

    await user.click(within(screen.getByRole("alert")).getByRole("button"));

    await waitFor(() => {
      expect(screen.getByText("表示エラーが発生しました")).toBeInTheDocument();
    });
    expect(
      screen.queryByText("アプリケーションエラーが発生しました"),
    ).not.toBeInTheDocument();
  });

  it("restores the fullscreen break and logs a reset when retry succeeds after the crash is resolved", async () => {
    const user = userEvent.setup();
    crash.fullscreen = true;
    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });

    render(<AppRoot />);

    expect(
      await screen.findByText("表示エラーが発生しました"),
    ).toBeInTheDocument();

    crash.fullscreen = false;
    await user.click(within(screen.getByRole("alert")).getByRole("button"));

    expect(await screen.findByText("休憩")).toBeInTheDocument();
    expect(
      screen.queryByText("表示エラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "fullscreen_break" },
    );
  });

  it("restores the app boundary and logs a reset when retry succeeds after the crash is resolved", async () => {
    const user = userEvent.setup();
    crash.home = true;

    render(<AppRoot />);

    expect(
      await screen.findByText("アプリケーションエラーが発生しました"),
    ).toBeInTheDocument();

    crash.home = false;
    await user.click(within(screen.getByRole("alert")).getByRole("button"));

    expect(await screen.findByText("Pomolice")).toBeInTheDocument();
    expect(
      screen.queryByText("アプリケーションエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "app" },
    );
  });

  it("resets the settings boundary and logs a reset when navigating to another settings screen", async () => {
    const user = userEvent.setup();

    render(<AppRoot />);
    await openSettingsTop(user);
    crash.settingsTimer = true;
    await openTimerSettings(user);

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();

    crash.settingsTimer = false;
    await user.keyboard("{Escape}");

    expect(await screen.findByText("設定")).toBeInTheDocument();
    expect(
      screen.queryByText("設定画面の表示中にエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });
});
