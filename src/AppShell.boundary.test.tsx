import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import {
  afterAll,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from "vitest";
import { AppShell } from "./AppShell";
import { AppErrorBoundary } from "@/app/ui/AppErrorBoundary";
import { FRONTEND_LOG_EVENT_IDS } from "./logging/logEvents";
import { throwOnRender } from "./test/throwOnRender";
import type { AppScreen } from "./types/appScreen";

const { errorLoggerMock, infoLoggerMock } = vi.hoisted(() => ({
  errorLoggerMock: vi.fn(),
  infoLoggerMock: vi.fn(),
}));

vi.mock("./logging/logger", () => ({
  createLogger: vi.fn(() => ({
    debug: vi.fn(),
    error: errorLoggerMock,
    info: infoLoggerMock,
    trace: vi.fn(),
    warn: vi.fn(),
  })),
}));

const ThrowingHomeContent = throwOnRender("home crash");
const ThrowingFullscreenBreakContent = throwOnRender("fullscreen crash");
const ThrowingSettingsContent = throwOnRender("settings crash");

let settingsShouldCrash = false;

function RetryableSettingsContent(): React.ReactElement {
  if (settingsShouldCrash) {
    throw new Error("settings crash");
  }

  return <div>Recovered settings content</div>;
}

function AppShellBackToHomeHarness(): React.ReactElement {
  const [currentScreen, setCurrentScreen] =
    useState<AppScreen>("settingsTimer");

  return (
    <AppErrorBoundary>
      <AppShell
        currentScreen={currentScreen}
        isFullscreenBreak={false}
        onBackToHome={() => {
          setCurrentScreen("home");
        }}
        renderHomeScreen={() => <div>Home content</div>}
        renderFullscreenBreakScreen={() => <div>Break content</div>}
        renderSettingsScreen={() => <RetryableSettingsContent />}
      />
    </AppErrorBoundary>
  );
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

describe("AppShell boundary policy", () => {
  beforeAll(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
  });

  afterAll(() => {
    vi.restoreAllMocks();
  });

  beforeEach(() => {
    settingsShouldCrash = false;
    errorLoggerMock.mockClear();
    infoLoggerMock.mockClear();
  });

  it("contains fullscreen break render errors within the fullscreen break boundary", async () => {
    render(
      <AppErrorBoundary>
        <AppShell
          currentScreen="home"
          isFullscreenBreak={true}
          onBackToHome={vi.fn()}
          renderHomeScreen={() => <div>Home content</div>}
          renderFullscreenBreakScreen={() => <ThrowingFullscreenBreakContent />}
          renderSettingsScreen={() => <div>Settings content</div>}
        />
      </AppErrorBoundary>,
    );

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

  it("contains settings render errors within the settings boundary and logs the current settings screen", async () => {
    render(
      <AppErrorBoundary>
        <AppShell
          currentScreen="settingsTimer"
          isFullscreenBreak={false}
          onBackToHome={vi.fn()}
          renderHomeScreen={() => <div>Home content</div>}
          renderFullscreenBreakScreen={() => <div>Break content</div>}
          renderSettingsScreen={() => <ThrowingSettingsContent />}
        />
      </AppErrorBoundary>,
    );

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

  it("lets home render errors reach the app boundary", async () => {
    render(
      <AppErrorBoundary>
        <AppShell
          currentScreen="home"
          isFullscreenBreak={false}
          onBackToHome={vi.fn()}
          renderHomeScreen={() => <ThrowingHomeContent />}
          renderFullscreenBreakScreen={() => <div>Break content</div>}
          renderSettingsScreen={() => <div>Settings content</div>}
        />
      </AppErrorBoundary>,
    );

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

  it("resets the settings boundary and shows home content from the settings fallback", async () => {
    const user = userEvent.setup();
    settingsShouldCrash = true;

    render(<AppShellBackToHomeHarness />);

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();

    settingsShouldCrash = false;
    await user.click(screen.getByRole("button", { name: "ホームへ戻る" }));

    expect(await screen.findByText("Home content")).toBeInTheDocument();
    expect(
      screen.queryByText("設定画面の表示中にエラーが発生しました"),
    ).not.toBeInTheDocument();
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });

  it("resets the settings boundary when the settings screen changes", async () => {
    settingsShouldCrash = true;
    const { rerender } = render(
      <AppErrorBoundary>
        <AppShell
          currentScreen="settingsTimer"
          isFullscreenBreak={false}
          onBackToHome={vi.fn()}
          renderHomeScreen={() => <div>Home content</div>}
          renderFullscreenBreakScreen={() => <div>Break content</div>}
          renderSettingsScreen={() => <RetryableSettingsContent />}
        />
      </AppErrorBoundary>,
    );

    expect(
      await screen.findByText("設定画面の表示中にエラーが発生しました"),
    ).toBeInTheDocument();

    settingsShouldCrash = false;
    rerender(
      <AppErrorBoundary>
        <AppShell
          currentScreen="settingsTop"
          isFullscreenBreak={false}
          onBackToHome={vi.fn()}
          renderHomeScreen={() => <div>Home content</div>}
          renderFullscreenBreakScreen={() => <div>Break content</div>}
          renderSettingsScreen={() => <RetryableSettingsContent />}
        />
      </AppErrorBoundary>,
    );

    expect(
      await screen.findByText("Recovered settings content"),
    ).toBeInTheDocument();
    expect(
      screen.queryByText("設定画面の表示中にエラーが発生しました"),
    ).not.toBeInTheDocument();
    await waitFor(() => {
      expect(infoLoggerMock).toHaveBeenCalledTimes(1);
    });
    expect(infoLoggerMock).toHaveBeenCalledWith(
      FRONTEND_LOG_EVENT_IDS.errorBoundaryRenderErrorReset,
      { boundary_scope: "settings" },
    );
  });
});
