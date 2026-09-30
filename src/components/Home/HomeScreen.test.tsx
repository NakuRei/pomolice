import { describe, it, expect, beforeEach, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { HomeScreen, type HomeScreenProps } from "./HomeScreen";
import { useErrorStore } from "@/store/errorStore";
import { useTimerStore } from "@/store/timerStore";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";

function createProps(
  overrides: Partial<HomeScreenProps> = {},
): HomeScreenProps {
  return {
    notificationSoundMuted: false,
    isAutoRunWindowActive: false,
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
    resetTimerStoreForTest();
    useTimerStore.setState({
      settings: createSettingsForTest(),
      status: "idle",
      remainingSeconds: 25 * 60,
      sessionType: "work",
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
      isConnected: false,
      bootAutoStartOffer: null,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
  });

  it.each([
    { sessionType: "work", label: "FOCUS", name: "作業時間" },
    { sessionType: "short_break", label: "BREAK", name: "短い休憩" },
    { sessionType: "long_break", label: "LONG", name: "長い休憩" },
  ] as const)(
    "labels the $sessionType session as $label, named $name for assistive technology",
    ({ sessionType, label, name }) => {
      useTimerStore.setState({ sessionType });

      render(<HomeScreen {...createProps()} />);

      expect(screen.getByText(label)).toBeInTheDocument();
      expect(screen.getByLabelText(name)).toBeInTheDocument();
    },
  );

  it("shows the remaining time as a polite live region that follows the timer", () => {
    render(<HomeScreen {...createProps()} />);

    const timer = screen.getByText("25:00");
    expect(timer).toHaveAttribute("aria-live", "polite");
    expect(timer).toHaveAttribute("aria-atomic", "true");

    act(() => {
      useTimerStore.setState({ remainingSeconds: 1499 });
    });
    expect(screen.getByText("24:59")).toBeInTheDocument();
  });

  it("keeps the phase label whether or not the timer runs", () => {
    useTimerStore.setState({ status: "working" });
    render(<HomeScreen {...createProps()} />);
    expect(screen.getByText("FOCUS")).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ status: "idle" });
    });
    expect(screen.getByText("FOCUS")).toBeInTheDocument();
  });

  it("tells the user when the saved settings could not be read", () => {
    useTimerStore.setState({ settingsFileStatus: "unusable" });

    render(<HomeScreen {...createProps()} />);

    expect(screen.getByTestId("timer-display")).toHaveTextContent("25:00");
    expect(screen.getByRole("status")).toHaveTextContent(
      /保存済みの設定を読み込めなかった/u,
    );
  });

  it("shows no settings notice while the settings file is usable", () => {
    render(<HomeScreen {...createProps()} />);

    expect(
      screen.queryByTestId("settings-file-notice"),
    ).not.toBeInTheDocument();
  });

  it("renders the timer and top-level controls", () => {
    render(<HomeScreen {...createProps()} />);

    expect(screen.getByTestId("timer-display")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "設定を開く" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通知音" })).toBeInTheDocument();
  });

  it.each(["idle", "working"] as const)(
    "does not show break emergency release guidance while %s",
    (status) => {
      useTimerStore.setState({ status });
      render(<HomeScreen {...createProps()} />);
      expect(screen.queryByText(/緊急解除/u)).not.toBeInTheDocument();
    },
  );

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

  it("hides the mute toggle until the settings have arrived", () => {
    render(<HomeScreen {...createProps({ notificationSoundMuted: null })} />);

    expect(
      screen.queryByRole("button", { name: "通知音" }),
    ).not.toBeInTheDocument();
  });

  it("shows why a settings save failed", () => {
    useErrorStore.getState().reportError({
      id: "update_settings",
      message: "データの保存に失敗しました",
      technicalDetail: "persistence_error",
      severity: "recoverable",
      origin: "backend_command",
      canRetry: false,
      retryFn: null,
    });

    render(<HomeScreen {...createProps()} />);

    expect(screen.getByText("データの保存に失敗しました")).toBeInTheDocument();
  });

  it("shows the scheduled auto-run state alongside the continue mode", () => {
    render(
      <HomeScreen
        {...createProps({
          isAutoRunWindowActive: true,
          isAutoContinueEnabled: true,
        })}
      />,
    );

    expect(screen.getByTestId("header-auto-run")).toHaveTextContent(
      "SCHEDULED",
    );
    expect(screen.getByText(/AUTO_CONTINUE/u)).toBeInTheDocument();
    expect(screen.queryByText(/ASK_EACH_TIME/u)).not.toBeInTheDocument();
  });

  it("shows ASK_EACH_TIME when the confirmation policy is active", () => {
    render(
      <HomeScreen
        {...createProps({
          requiresManualContinue: true,
        })}
      />,
    );

    expect(screen.getByText(/ASK_EACH_TIME/u)).toBeInTheDocument();
    expect(screen.queryByText(/AUTO_CONTINUE/u)).not.toBeInTheDocument();
  });

  it("shows AUTO_CONTINUE when auto-continue is on", () => {
    render(
      <HomeScreen
        {...createProps({
          isAutoContinueEnabled: true,
        })}
      />,
    );

    expect(screen.getByText(/AUTO_CONTINUE/u)).toBeInTheDocument();
    expect(screen.queryByText(/ASK_EACH_TIME/u)).not.toBeInTheDocument();
  });

  it("shows SCHEDULED only while an auto-run window is active", () => {
    const { rerender } = render(
      <HomeScreen {...createProps({ isAutoRunWindowActive: false })} />,
    );
    expect(screen.getByTestId("header-auto-run")).toHaveTextContent("MANUAL");

    rerender(<HomeScreen {...createProps({ isAutoRunWindowActive: true })} />);
    expect(screen.getByTestId("header-auto-run")).toHaveTextContent(
      "SCHEDULED",
    );
  });

  it("no longer duplicates the cycle count as text beside the bar", () => {
    render(<HomeScreen {...createProps()} />);

    expect(
      screen.queryByText(/^ポモドーロ \d+\/\d+$/u),
    ).not.toBeInTheDocument();
    expect(screen.getByTestId("cycle-progress")).toBeInTheDocument();
  });
});
