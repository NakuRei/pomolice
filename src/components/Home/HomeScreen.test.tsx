import { describe, it, expect, beforeEach, vi } from "vitest";
import { act, render, screen } from "@testing-library/react";
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
    useSettingsStore.getState().reset();
    useSettingsStore.setState({
      durationSettingsApplicationStatus: "confirmed",
      automationSettingsApplicationStatus: "confirmed",
    });
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
      bootAutoStartPromptWindow: null,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
      isSettingsSynchronizationInFlight: false,
    });
  });

  it.each([
    { sessionType: SessionType.WORK, label: "FOCUS", name: "作業時間" },
    { sessionType: SessionType.SHORT_BREAK, label: "BREAK", name: "短い休憩" },
    { sessionType: SessionType.LONG_BREAK, label: "LONG", name: "長い休憩" },
  ])(
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
    useTimerStore.setState({ status: TimerStatus.WORKING });
    render(<HomeScreen {...createProps()} />);
    expect(screen.getByText("FOCUS")).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ status: TimerStatus.IDLE });
    });
    expect(screen.getByText("FOCUS")).toBeInTheDocument();
  });

  it("keeps the timer visible while settings remain unresolved", () => {
    useSettingsStore.setState({
      durationSettingsApplicationStatus: "pending",
      automationSettingsApplicationStatus: "unconfirmed",
    });

    render(<HomeScreen {...createProps()} />);

    expect(screen.getByTestId("timer-display")).toHaveTextContent("25:00");
  });

  it("shows unresolved settings and hides the notice after confirmation", () => {
    useSettingsStore.setState({
      durationSettingsApplicationStatus: "pending",
      automationSettingsApplicationStatus: "confirmed",
    });
    useTimerStore.setState({ isConnected: true });

    const { rerender } = render(<HomeScreen {...createProps()} />);

    expect(screen.getByRole("status")).toHaveTextContent(
      /適用はまだ確認されていません/u,
    );

    act(() => {
      useSettingsStore.setState({
        durationSettingsApplicationStatus: "confirmed",
      });
    });
    rerender(<HomeScreen {...createProps()} />);

    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("renders the timer and top-level controls", () => {
    render(<HomeScreen {...createProps()} />);

    expect(screen.getByTestId("timer-display")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "設定を開く" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通知音" })).toBeInTheDocument();
  });

  it.each([TimerStatus.IDLE, TimerStatus.WORKING])(
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
