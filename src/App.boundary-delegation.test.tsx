import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type * as FullscreenBreakModule from "./components/BreakScreen/FullscreenBreak";
import type * as SettingsTimerPageModule from "./components/Settings/SettingsTimerPage";
import { useSettingsStore } from "./store/settingsStore";
import { useTimerStore } from "./store/timerStore";
import { resetTimerStoreForTest } from "./test/resetTimerStore";

const { crash } = vi.hoisted(() => ({
  crash: {
    fullscreenBreak: false,
    settingsTimerPage: false,
  },
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(vi.fn()),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
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

vi.mock("./hooks/useTimerSync", () => ({
  useTimerSync: vi.fn(),
}));

vi.mock("./components/BreakScreen/FullscreenBreak", async (orig) => {
  const actual = await orig<typeof FullscreenBreakModule>();
  const { throwOnRender } = await import("./test/throwOnRender");
  const ThrowingFullscreenBreak = throwOnRender("fullscreen break crash");
  const RealFullscreenBreak = actual.FullscreenBreak;

  function FullscreenBreak(): React.ReactElement {
    return crash.fullscreenBreak ? (
      <ThrowingFullscreenBreak />
    ) : (
      <RealFullscreenBreak />
    );
  }

  return { ...actual, FullscreenBreak };
});

vi.mock("./components/Settings/SettingsTimerPage", async (orig) => {
  const actual = await orig<typeof SettingsTimerPageModule>();
  const { throwOnRender } = await import("./test/throwOnRender");
  const ThrowingSettingsTimerPage = throwOnRender("settings timer crash");
  const RealSettingsTimerPage = actual.SettingsTimerPage;

  function SettingsTimerPage(
    props: React.ComponentProps<typeof RealSettingsTimerPage>,
  ): React.ReactElement {
    return crash.settingsTimerPage ? (
      <ThrowingSettingsTimerPage />
    ) : (
      <RealSettingsTimerPage {...props} />
    );
  }

  return { ...actual, SettingsTimerPage };
});

function resetCrashFlags(): void {
  crash.fullscreenBreak = false;
  crash.settingsTimerPage = false;
}

async function openTimerSettings(): Promise<void> {
  const user = userEvent.setup();

  await user.click(screen.getByRole("button", { name: "設定を開く" }));
  await user.click(screen.getByRole("button", { name: /^タイマー設定/u }));
}

describe("App boundary delegation", () => {
  beforeEach(() => {
    vi.spyOn(console, "error").mockImplementation(() => undefined);
    resetCrashFlags();
    useSettingsStore.getState().reset();
    resetTimerStoreForTest();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("contains a fullscreen render error within the fullscreen boundary", () => {
    crash.fullscreenBreak = true;
    useTimerStore.setState({ isFullscreenBreak: true });

    render(<App />);

    expect(
      within(screen.getByRole("alert")).getByText("表示エラーが発生しました"),
    ).toBeInTheDocument();
  });

  it("contains a settings render error within the settings boundary", async () => {
    render(<App />);
    crash.settingsTimerPage = true;

    await openTimerSettings();

    expect(
      within(screen.getByRole("alert")).getByText(
        "設定画面の表示中にエラーが発生しました",
      ),
    ).toBeInTheDocument();
  });
});
