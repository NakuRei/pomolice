import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useSettingsStore } from "@/store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "@/store/timerStore";
import { resetSettingsStoreForTest } from "@/test/resetSettingsStore";
import { SettingsNotificationPage } from "./SettingsNotificationPage";
import { SettingsTimerPage } from "./SettingsTimerPage";
import { playNotificationSoundPreview } from "@/services/notifications";

vi.mock("@/services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(() =>
    Promise.resolve({ status: "played" }),
  ),
}));

describe("SettingsNotificationPage", () => {
  beforeEach(() => {
    resetSettingsStoreForTest();
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
      bootAutoStartPromptWindow: null,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
    vi.mocked(playNotificationSoundPreview).mockResolvedValue({
      status: "played",
    });
  });

  it("saves notification settings and keeps timer settings unchanged", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      workDuration: 27,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(
      <SettingsNotificationPage
        openSettingsTopScreen={openSettingsTopScreen}
      />,
    );

    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    fireEvent.change(volumeSlider, { target: { value: "35" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(35);
    expect(useSettingsStore.getState().workDuration).toBe(27);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("discards notification draft on cancel", async () => {
    const user = userEvent.setup();
    const openSettingsTopScreen = vi.fn();

    useSettingsStore.setState({
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    render(
      <SettingsNotificationPage
        openSettingsTopScreen={openSettingsTopScreen}
      />,
    );

    await user.click(screen.getByRole("button", { name: "通知音" }));
    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    await user.click(screen.getByRole("button", { name: "通知音" }));
    fireEvent.change(volumeSlider, { target: { value: "80" } });
    await user.click(screen.getByRole("button", { name: "キャンセル" }));

    expect(useSettingsStore.getState().notificationSoundMuted).toBe(false);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(50);
    expect(openSettingsTopScreen).toHaveBeenCalledTimes(1);
  });

  it("disables volume and preview button when notification is muted", async () => {
    const user = userEvent.setup();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await user.click(screen.getByRole("button", { name: "通知音" }));

    expect(screen.getByRole("slider", { name: "通知音量" })).toBeDisabled();
    expect(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    ).toBeDisabled();
    expect(
      screen.getByText("通知音がミュート中のため再生しませんでした。"),
    ).toBeInTheDocument();
  });

  it("initializes notification preview ui state on each mount", async () => {
    const user = userEvent.setup();

    const firstRender = render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    await user.selectOptions(
      screen.getByRole("combobox", { name: "テストする通知音の種類" }),
      "long_break",
    );
    await user.click(screen.getByRole("button", { name: "通知音" }));
    firstRender.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    const previewTypeSelect = screen.getByRole("combobox", {
      name: "テストする通知音の種類",
    });
    expect(previewTypeSelect).toHaveValue("work");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveAttribute("aria-live", "polite");
    expect(
      screen.getByTestId("settings-notification-sound-preview-status"),
    ).toHaveTextContent("");
  });

  it("shows unavailable message when preview sound file is not available", async () => {
    const user = userEvent.setup();
    vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({
      status: "unavailable",
    });

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await user.click(
      screen.getByRole("button", { name: "通知音をテスト再生する" }),
    );

    expect(playNotificationSoundPreview).toHaveBeenCalledTimes(1);
    expect(
      screen.getByText(
        "通知音ファイルを読み込めなかったため再生できませんでした。",
      ),
    ).toBeInTheDocument();
  });

  it("focuses the notification heading on mount", async () => {
    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "通知設定" })).toHaveFocus();
    });
  });

  it("does not persist unsaved timer draft when notification settings are saved", async () => {
    const user = userEvent.setup();

    useSettingsStore.setState({
      workDuration: 25,
      notificationSoundMuted: false,
      notificationSoundVolumePercent: 50,
    });

    const timerPageRenderResult = render(
      <SettingsTimerPage openSettingsTopScreen={() => undefined} />,
    );
    const workInput = screen.getByRole("spinbutton", { name: "作業時間" });
    await user.clear(workInput);
    await user.type(workInput, "40");
    await user.click(screen.getByRole("button", { name: "キャンセル" }));
    timerPageRenderResult.unmount();

    render(
      <SettingsNotificationPage openSettingsTopScreen={() => undefined} />,
    );
    const volumeSlider = screen.getByRole("slider", { name: "通知音量" });
    fireEvent.change(volumeSlider, { target: { value: "30" } });
    await user.click(screen.getByRole("button", { name: "保存" }));

    expect(useSettingsStore.getState().workDuration).toBe(25);
    expect(useSettingsStore.getState().notificationSoundVolumePercent).toBe(30);
  });
});
