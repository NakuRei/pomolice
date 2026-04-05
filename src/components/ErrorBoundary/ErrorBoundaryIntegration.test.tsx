import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
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
import { FRONTEND_LOG_EVENT_IDS } from "../../logging/logEvents";
import { useSettingsStore } from "../../store/settingsStore";
import {
  SessionType,
  TimerStatus,
  useTimerStore,
} from "../../store/timerStore";

const {
  debugLoggerMock,
  errorLoggerMock,
  infoLoggerMock,
  traceLoggerMock,
  uiState,
  warnLoggerMock,
} = vi.hoisted(() => ({
  debugLoggerMock: vi.fn(),
  errorLoggerMock: vi.fn(),
  infoLoggerMock: vi.fn(),
  traceLoggerMock: vi.fn(),
  uiState: {
    fullscreenShouldThrow: false,
    homeShouldThrow: false,
    settingsAutoRunShouldThrow: false,
    settingsNotificationShouldThrow: false,
    settingsTimerShouldThrow: false,
    settingsTopShouldThrow: false,
  },
  warnLoggerMock: vi.fn(),
}));

vi.mock("../../hooks/useTimerSync", () => ({
  useTimerSync: (): void => undefined,
}));

vi.mock("../../logging/logger", () => ({
  createLogger: vi.fn(() => ({
    debug: debugLoggerMock,
    error: errorLoggerMock,
    info: infoLoggerMock,
    trace: traceLoggerMock,
    warn: warnLoggerMock,
  })),
}));

vi.mock("../../components/Home/HomeScreen", () => ({
  HomeScreen: ({
    onOpenSettingsTopScreen,
  }: {
    onOpenSettingsTopScreen: () => void;
  }): React.ReactElement => {
    if (uiState.homeShouldThrow) {
      throw new Error("home crash");
    }

    return (
      <div>
        <h1>Pomolice</h1>
        <button type="button" onClick={onOpenSettingsTopScreen}>
          設定を開く
        </button>
      </div>
    );
  },
}));

vi.mock("../../components/BreakScreen/FullscreenBreak", () => ({
  FullscreenBreak: (): React.ReactElement => {
    if (uiState.fullscreenShouldThrow) {
      throw new Error("fullscreen crash");
    }

    return <div>休憩</div>;
  },
}));

vi.mock("../../components/Settings/SettingsScreenLayout", () => ({
  SettingsScreenLayout: ({
    children,
    mainTestId,
  }: {
    children: React.ReactNode;
    mainTestId: string;
  }): React.ReactElement => <main data-testid={mainTestId}>{children}</main>,
}));

vi.mock("../../components/Settings/SettingsTopPage", () => ({
  SettingsTopPage: ({
    openAutoRunSettings,
    openHomeScreen,
    openNotificationSettings,
    openTimerSettings,
  }: {
    openAutoRunSettings: () => void;
    openHomeScreen: () => void;
    openNotificationSettings: () => void;
    openTimerSettings: () => void;
  }): React.ReactElement => {
    if (uiState.settingsTopShouldThrow) {
      throw new Error("settings top crash");
    }

    return (
      <div>
        <h1>設定</h1>
        <button type="button" onClick={openHomeScreen}>
          ホームへ戻る
        </button>
        <button type="button" onClick={openTimerSettings}>
          タイマー設定
        </button>
        <button type="button" onClick={openAutoRunSettings}>
          曜日別自動実行
        </button>
        <button type="button" onClick={openNotificationSettings}>
          通知と音量
        </button>
      </div>
    );
  },
}));

vi.mock("../../components/Settings/SettingsTimerPage", () => ({
  SettingsTimerPage: ({
    openSettingsTopScreen,
  }: {
    openSettingsTopScreen: () => void;
  }): React.ReactElement => {
    if (uiState.settingsTimerShouldThrow) {
      throw new Error("settings timer crash");
    }

    return (
      <div>
        <h1>タイマー設定</h1>
        <button type="button" onClick={openSettingsTopScreen}>
          設定へ戻る
        </button>
      </div>
    );
  },
}));

vi.mock("../../components/Settings/SettingsAutoRunPage", () => ({
  SettingsAutoRunPage: ({
    openSettingsTopScreen,
  }: {
    openSettingsTopScreen: () => void;
  }): React.ReactElement => {
    if (uiState.settingsAutoRunShouldThrow) {
      throw new Error("settings auto-run crash");
    }

    return (
      <div>
        <h1>曜日別自動実行設定</h1>
        <button type="button" onClick={openSettingsTopScreen}>
          設定へ戻る
        </button>
      </div>
    );
  },
}));

vi.mock("../../components/Settings/SettingsNotificationPage", () => ({
  SettingsNotificationPage: ({
    openSettingsTopScreen,
  }: {
    openSettingsTopScreen: () => void;
  }): React.ReactElement => {
    if (uiState.settingsNotificationShouldThrow) {
      throw new Error("settings notification crash");
    }

    return (
      <div>
        <h1>通知設定</h1>
        <button type="button" onClick={openSettingsTopScreen}>
          設定へ戻る
        </button>
      </div>
    );
  },
}));

function resetMockedUiState(): void {
  uiState.homeShouldThrow = false;
  uiState.fullscreenShouldThrow = false;
  uiState.settingsTopShouldThrow = false;
  uiState.settingsTimerShouldThrow = false;
  uiState.settingsAutoRunShouldThrow = false;
  uiState.settingsNotificationShouldThrow = false;
}

async function openSettingsTop(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "設定を開く" }));
}

async function openTimerSettingsFromSettingsTop(
  user: ReturnType<typeof userEvent.setup>,
): Promise<void> {
  await user.click(screen.getByRole("button", { name: "タイマー設定" }));
}

describe("ErrorBoundary integration", () => {
  beforeAll(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    vi.clearAllMocks();
    resetMockedUiState();
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

  it("shows the fullscreen fallback, logs the fullscreen scope, and retries successfully", async () => {
    const user = userEvent.setup();
    uiState.fullscreenShouldThrow = true;
    act(() => {
      useTimerStore.setState({ isFullscreenBreak: true });
    });

    render(<AppRoot />);

    expect(
      await screen.findByText("表示エラーが発生しました"),
    ).toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({
        boundary_scope: "fullscreen_break",
        failure_message: "fullscreen crash",
      }),
    );

    uiState.fullscreenShouldThrow = false;
    await user.click(screen.getByRole("button", { name: "再表示を試みる" }));

    expect(await screen.findByText("休憩")).toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "fullscreen_break" },
    );
  });

  it("contains settings crashes within the settings boundary and returns home", async () => {
    const user = userEvent.setup();

    render(<AppRoot />);
    await openSettingsTop(user);
    uiState.settingsTimerShouldThrow = true;
    await openTimerSettingsFromSettingsTop(user);

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
        failure_message: "settings timer crash",
      }),
    );

    uiState.settingsTimerShouldThrow = false;
    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(await screen.findByText("Pomolice")).toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
    expect(errorLoggerMock).not.toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({ boundary_scope: "app" }),
    );
  });

  it("shows the app fallback for home-screen crashes and retries successfully", async () => {
    const user = userEvent.setup();
    uiState.homeShouldThrow = true;

    render(<AppRoot />);

    expect(
      await screen.findByText("アプリケーションエラーが発生しました"),
    ).toBeInTheDocument();
    expect(errorLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryBoundaryRenderErrorCaught,
      expect.any(Error),
      expect.objectContaining({
        boundary_scope: "app",
        failure_message: "home crash",
      }),
    );

    uiState.homeShouldThrow = false;
    await user.click(screen.getByRole("button", { name: "再表示を試みる" }));

    expect(await screen.findByText("Pomolice")).toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "app" },
    );
  });

  it("auto-resets the settings boundary when the current settings screen changes", async () => {
    const user = userEvent.setup();

    render(<AppRoot />);
    await openSettingsTop(user);
    uiState.settingsTimerShouldThrow = true;
    await openTimerSettingsFromSettingsTop(user);

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();

    uiState.settingsTimerShouldThrow = false;
    fireEvent.keyDown(window, { key: "Escape" });

    await waitFor(() => {
      expect(screen.getByText("設定")).toBeInTheDocument();
    });
    expect(
      screen.queryByText("設定画面の表示中にエラーが発生しました"),
    ).toBeNull();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });
});
