import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HomeScreen, type HomeScreenProps } from "./HomeScreen";
import { useErrorStore } from "@/store/errorStore";
import { useSettingsStore } from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";

function createProps(
  overrides: Partial<HomeScreenProps> = {},
): HomeScreenProps {
  return {
    notificationSoundMuted: false,
    autoRunFeatureEnabled: false,
    isAutoContinueEnabled: false,
    requiresManualContinue: false,
    onToggleNotificationSoundMuted: vi.fn(),
    onOpenSettingsTopScreen: vi.fn(),
    ...overrides,
  };
}

describe("HomeScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useErrorStore.getState().clearAllErrors();
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
      isConnected: false,
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
  });

  it("renders the home heading and top-level controls", () => {
    render(<HomeScreen {...createProps()} />);

    expect(
      screen.getByRole("heading", { name: "Pomolice" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "設定を開く" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通知音" })).toBeInTheDocument();
  });

  it("calls onOpenSettingsTopScreen when settings button is clicked", async () => {
    const user = userEvent.setup();
    const onOpenSettingsTopScreen = vi.fn();

    render(<HomeScreen {...createProps({ onOpenSettingsTopScreen })} />);

    await user.click(screen.getByRole("button", { name: "設定を開く" }));

    expect(onOpenSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("calls onToggleNotificationSoundMuted when mute button is clicked", async () => {
    const user = userEvent.setup();
    const onToggleNotificationSoundMuted = vi.fn();

    render(<HomeScreen {...createProps({ onToggleNotificationSoundMuted })} />);

    await user.click(screen.getByRole("button", { name: "通知音" }));

    expect(onToggleNotificationSoundMuted).toHaveBeenCalledTimes(1);
  });

  it("renders auto-run and auto-continue badges when both are enabled", () => {
    render(
      <HomeScreen
        {...createProps({
          autoRunFeatureEnabled: true,
          isAutoContinueEnabled: true,
        })}
      />,
    );

    expect(screen.getByText("自動実行ON")).toBeInTheDocument();
    expect(screen.getByText("自動継続ON")).toBeInTheDocument();
    expect(screen.queryByText("毎回確認")).not.toBeInTheDocument();
  });

  it("renders only manual-continue badge when confirmation policy is active", () => {
    render(
      <HomeScreen
        {...createProps({
          requiresManualContinue: true,
        })}
      />,
    );

    expect(screen.getByText("毎回確認")).toBeInTheDocument();
    expect(screen.queryByText("自動実行ON")).not.toBeInTheDocument();
    expect(screen.queryByText("自動継続ON")).not.toBeInTheDocument();
  });

  it("renders only auto-continue badge when auto-continue is enabled", () => {
    render(
      <HomeScreen
        {...createProps({
          isAutoContinueEnabled: true,
        })}
      />,
    );

    expect(screen.getByText("自動継続ON")).toBeInTheDocument();
    expect(screen.queryByText("自動実行ON")).not.toBeInTheDocument();
    expect(screen.queryByText("毎回確認")).not.toBeInTheDocument();
  });
});
