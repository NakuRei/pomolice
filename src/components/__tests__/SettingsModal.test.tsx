import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { playNotificationSoundPreview } from "../../services/notifications";
import { SettingsModal } from "../Settings/SettingsModal";
import { useSettingsStore } from "../../store/settingsStore";
import { SessionType, TimerStatus, useTimerStore } from "../../store/timerStore";

vi.mock("../../services/notifications", () => ({
  playNotificationSoundPreview: vi.fn(async () => ({ status: "played" })),
}));

describe("SettingsModal", () => {
  const mockOnClose = vi.fn();

  beforeEach(() => {
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
      isConnected: true,
      shouldPromptStartOnBoot: false,
      activeWindowSummary: null,
      infoMessage: null,
      infoMessageShownAtById: {},
    });
    mockOnClose.mockClear();
    vi.mocked(playNotificationSoundPreview).mockResolvedValue({ status: "played" });
  });

  describe("Visibility", () => {
    it("should not render when isOpen is false", () => {
      render(<SettingsModal isOpen={false} onClose={mockOnClose} />);
      const modal = screen.queryByTestId("settings-modal");
      expect(modal).not.toBeInTheDocument();
    });

    it("should render when isOpen is true", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toBeInTheDocument();
    });
  });

  describe("Initial Values", () => {
    it("should display current settings from store", () => {
      useSettingsStore.setState({
        workDuration: 30,
        shortBreakDuration: 10,
        longBreakDuration: 20,
        longBreakInterval: 3,
        notificationSoundVolumePercent: 35,
        post_break_transition_policy: "start_next_work_automatically",
      });

      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      const notificationSoundVolume = screen.getByTestId(
        "settings-notification-sound-volume"
      ) as HTMLInputElement;
      const postBreakPolicyAuto = screen.getByTestId(
        "settings-post-break-policy-auto"
      ) as HTMLInputElement;

      expect(workInput).toHaveValue(30);
      expect(shortBreakInput).toHaveValue(10);
      expect(longBreakInput).toHaveValue(20);
      expect(intervalInput).toHaveValue(3);
      expect(notificationSoundVolume).toHaveValue("35");
      expect(postBreakPolicyAuto).toBeChecked();
    });

    it("should display default settings", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      const notificationSoundVolume = screen.getByTestId(
        "settings-notification-sound-volume"
      ) as HTMLInputElement;
      const postBreakPolicyManual = screen.getByTestId(
        "settings-post-break-policy-manual"
      ) as HTMLInputElement;

      expect(workInput).toHaveValue(25);
      expect(shortBreakInput).toHaveValue(5);
      expect(longBreakInput).toHaveValue(15);
      expect(intervalInput).toHaveValue(4);
      expect(notificationSoundVolume).toHaveValue("50");
      expect(postBreakPolicyManual).toBeChecked();
    });
  });

  describe("Modal Content", () => {
    it("should display modal title", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByText("設定")).toBeInTheDocument();
    });

    it("should display all input labels", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByText("作業時間")).toBeInTheDocument();
      expect(screen.getByText("短い休憩")).toBeInTheDocument();
      expect(screen.getByText("長い休憩")).toBeInTheDocument();
      expect(screen.getByText("長い休憩の間隔")).toBeInTheDocument();
      expect(screen.getByText("通知音量")).toBeInTheDocument();
      expect(screen.getByText("休憩後の遷移")).toBeInTheDocument();
      expect(screen.getByText("毎回確認する")).toBeInTheDocument();
      expect(
        screen.getByText("休憩後に自動で次の作業を開始する")
      ).toBeInTheDocument();
    });

    it("should render post-break policy radio group with fixed test ids", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      expect(
        screen.getByTestId("settings-post-break-policy-group")
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("settings-post-break-policy-manual")
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("settings-post-break-policy-auto")
      ).toBeInTheDocument();
    });

    it("should show enabled label when auto-continue policy is selected", () => {
      useSettingsStore.setState({
        post_break_transition_policy: "start_next_work_automatically",
      });

      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      expect(
        screen.getByTestId("settings-post-break-policy-enabled-label")
      ).toBeInTheDocument();
    });

    it("should hide enabled label when manual policy is selected", () => {
      useSettingsStore.setState({
        post_break_transition_policy: "require_manual_continue",
      });

      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      expect(
        screen.queryByTestId("settings-post-break-policy-enabled-label")
      ).not.toBeInTheDocument();
    });

    it("should have save and cancel buttons", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByTestId("settings-save")).toBeInTheDocument();
      expect(screen.getByTestId("settings-cancel")).toBeInTheDocument();
    });

    it("should display notification sound preview controls", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(
        screen.getByTestId("settings-notification-sound-preview-type"),
      ).toBeInTheDocument();
      expect(
        screen.getByTestId("settings-notification-sound-preview-button"),
      ).toBeInTheDocument();
    });
  });

  describe("Input Changes", () => {
    it("should update work duration input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      expect(workInput).toHaveValue(30);
    });

    it("should update short break input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      fireEvent.change(shortBreakInput, { target: { value: "10" } });

      expect(shortBreakInput).toHaveValue(10);
    });

    it("should update long break input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      fireEvent.change(longBreakInput, { target: { value: "20" } });

      expect(longBreakInput).toHaveValue(20);
    });

    it("should update interval input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      fireEvent.change(intervalInput, { target: { value: "3" } });

      expect(intervalInput).toHaveValue(3);
    });

    it("should update notification sound volume slider", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const soundVolumeInput = screen.getByTestId(
        "settings-notification-sound-volume"
      ) as HTMLInputElement;
      fireEvent.change(soundVolumeInput, { target: { value: "25" } });

      expect(soundVolumeInput).toHaveValue("25");
      expect(
        screen.getByTestId("settings-notification-sound-volume-value")
      ).toHaveTextContent("25%");
    });

    it("should switch post-break transition policy radio", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const manualRadio = screen.getByTestId(
        "settings-post-break-policy-manual"
      ) as HTMLInputElement;
      const autoRadio = screen.getByTestId(
        "settings-post-break-policy-auto"
      ) as HTMLInputElement;

      expect(manualRadio).toBeChecked();
      expect(autoRadio).not.toBeChecked();

      fireEvent.click(autoRadio);

      expect(manualRadio).not.toBeChecked();
      expect(autoRadio).toBeChecked();
    });

    it("should toggle notification mute button and save muted state", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const muteButton = screen.getByTestId("settings-notification-mute-button");
      fireEvent.click(muteButton);

      const soundVolumeInput = screen.getByTestId(
        "settings-notification-sound-volume"
      ) as HTMLInputElement;
      expect(soundVolumeInput).toBeDisabled();

      fireEvent.click(screen.getByTestId("settings-save"));
      expect(useSettingsStore.getState().notificationSoundMuted).toBe(true);
    });

    it("should play selected notification sound preview", async () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const soundTypeSelect = screen.getByTestId(
        "settings-notification-sound-preview-type",
      ) as HTMLSelectElement;
      fireEvent.change(soundTypeSelect, { target: { value: "long_break" } });

      const soundVolumeInput = screen.getByTestId(
        "settings-notification-sound-volume",
      ) as HTMLInputElement;
      fireEvent.change(soundVolumeInput, { target: { value: "25" } });

      fireEvent.click(screen.getByTestId("settings-notification-sound-preview-button"));

      expect(playNotificationSoundPreview).toHaveBeenCalledWith({
        soundType: "long_break",
        volumePercent: 25,
        muted: false,
      });
      expect(
        await screen.findByText("通知音を再生しました。"),
      ).toBeInTheDocument();
    });

    it("should show playback error status when preview playback fails", async () => {
      vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({ status: "error" });
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-notification-sound-preview-button"));

      expect(
        await screen.findByText("通知音の再生に失敗しました。"),
      ).toBeInTheDocument();
    });

    it("should show unavailable status when preview sound is unavailable", async () => {
      vi.mocked(playNotificationSoundPreview).mockResolvedValueOnce({
        status: "unavailable",
      });
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-notification-sound-preview-button"));

      expect(
        await screen.findByText(
          "通知音ファイルを読み込めなかったため再生できませんでした。",
        ),
      ).toBeInTheDocument();
    });

    it("should disable preview button while muted and show muted status", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-notification-mute-button"));

      expect(
        screen.getByTestId("settings-notification-sound-preview-button"),
      ).toBeDisabled();
      expect(screen.getByText("通知音がミュート中のため再生しませんでした。")).toBeInTheDocument();
    });
  });

  describe("Save Button", () => {
    it("should be enabled with valid inputs", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();
    });

    it("should be disabled with invalid work duration", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "0" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid short break", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      fireEvent.change(shortBreakInput, { target: { value: "100" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid long break", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      // Phase 3: Using 100 (above max of 60) instead of 3, since min is now 0.01
      fireEvent.change(longBreakInput, { target: { value: "100" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid interval", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      fireEvent.change(intervalInput, { target: { value: "1" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should save settings and close modal", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(30);
      expect(mockOnClose).toHaveBeenCalledTimes(1);
    });

    it("should save all settings correctly", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      const soundVolumeInput = screen.getByTestId(
        "settings-notification-sound-volume"
      ) as HTMLInputElement;
      const autoPolicyRadio = screen.getByTestId(
        "settings-post-break-policy-auto"
      ) as HTMLInputElement;

      fireEvent.change(workInput, { target: { value: "40" } });
      fireEvent.change(shortBreakInput, { target: { value: "8" } });
      fireEvent.change(longBreakInput, { target: { value: "20" } });
      fireEvent.change(intervalInput, { target: { value: "3" } });
      fireEvent.change(soundVolumeInput, { target: { value: "70" } });
      fireEvent.click(autoPolicyRadio);

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(40);
      expect(state.shortBreakDuration).toBe(8);
      expect(state.longBreakDuration).toBe(20);
      expect(state.longBreakInterval).toBe(3);
      expect(state.notificationSoundVolumePercent).toBe(70);
      expect(state.post_break_transition_policy).toBe(
        "start_next_work_automatically"
      );
    });
  });

  describe("Cancel Button", () => {
    it("should close modal without saving", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      const cancelButton = screen.getByTestId("settings-cancel");
      fireEvent.click(cancelButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(25); // Original value
      expect(state.notificationSoundVolumePercent).toBe(50);
      expect(mockOnClose).toHaveBeenCalledTimes(1);
    });

    it("should reset form values on cancel", () => {
      const { rerender } = render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });
      expect(workInput).toHaveValue(30);

      const cancelButton = screen.getByTestId("settings-cancel");
      fireEvent.click(cancelButton);

      // Reopen modal
      rerender(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInputAfter = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      expect(workInputAfter).toHaveValue(25); // Reset to original
    });
  });

  describe("Weekly Auto-Run Schedule", () => {
    it("should save weekday toggle and time inputs", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));
      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));

      const mondayStartInput = screen.getByTestId(
        "settings-auto-run-day-start-mon",
      ) as HTMLInputElement;
      const mondayEndInput = screen.getByTestId(
        "settings-auto-run-day-end-mon",
      ) as HTMLInputElement;

      fireEvent.change(mondayStartInput, { target: { value: "08:30" } });
      fireEvent.change(mondayEndInput, { target: { value: "17:30" } });

      fireEvent.click(screen.getByTestId("settings-save"));

      const settings = useSettingsStore.getState();
      expect(settings.autoRunFeatureEnabled).toBe(true);
      expect(settings.weeklyAutoRunConfig.mon.enabled).toBe(true);
      expect(settings.weeklyAutoRunConfig.mon.startTime).toBe("08:30");
      expect(settings.weeklyAutoRunConfig.mon.endTime).toBe("17:30");
    });

    it("should block save and show error when time text fails parseHhMm revalidation", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));
      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
      fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
        target: { value: "9:00" },
      });

      expect(
        screen.getByTestId("settings-auto-run-day-error-mon"),
      ).toHaveTextContent("Start time for mon must use HH:MM (24-hour).");
      expect(screen.getByTestId("settings-save")).toBeDisabled();
    });

    it("should show start==end validation and disable save", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));
      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
      fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
        target: { value: "12:00" },
      });
      fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
        target: { value: "12:00" },
      });

      expect(
        screen.getByTestId("settings-auto-run-day-error-mon"),
      ).toHaveTextContent("Start and end time cannot be identical for mon.");
      expect(screen.getByTestId("settings-save")).toBeDisabled();
    });

    it("should show weekly overlap error for overlapping weekdays", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));
      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
      fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
        target: { value: "22:00" },
      });
      fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
        target: { value: "02:00" },
      });

      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-tue"));
      fireEvent.change(screen.getByTestId("settings-auto-run-day-start-tue"), {
        target: { value: "01:00" },
      });
      fireEvent.change(screen.getByTestId("settings-auto-run-day-end-tue"), {
        target: { value: "05:00" },
      });

      expect(screen.getByTestId("settings-auto-run-weekly-errors")).toBeInTheDocument();
      expect(screen.getByText("Schedule windows overlap between mon and tue.")).toBeInTheDocument();
    });

    it("should show overnight label when start time is after end time", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));
      fireEvent.click(screen.getByTestId("settings-auto-run-day-enabled-mon"));
      fireEvent.change(screen.getByTestId("settings-auto-run-day-start-mon"), {
        target: { value: "22:00" },
      });
      fireEvent.change(screen.getByTestId("settings-auto-run-day-end-mon"), {
        target: { value: "02:00" },
      });

      expect(
        screen.getByTestId("settings-auto-run-day-overnight-mon"),
      ).toHaveTextContent("翌日まで");
    });

    it("should disable only duration inputs while non-idle and keep schedule editable", () => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      expect(screen.getByTestId("settings-work-duration-input")).toBeDisabled();
      expect(screen.getByTestId("settings-short-break-input")).toBeDisabled();
      expect(screen.getByTestId("settings-long-break-input")).toBeDisabled();
      expect(screen.getByTestId("settings-interval-input")).toBeDisabled();
      expect(
        screen.getByTestId("settings-duration-edit-disabled-message"),
      ).toBeInTheDocument();

      const featureToggle = screen.getByTestId("settings-auto-run-feature-toggle");
      expect(featureToggle).not.toBeDisabled();
      fireEvent.click(featureToggle);

      const mondayEnabledToggle = screen.getByTestId("settings-auto-run-day-enabled-mon");
      expect(mondayEnabledToggle).not.toBeDisabled();
      fireEvent.click(mondayEnabledToggle);
      expect(mondayEnabledToggle).toBeChecked();
    });

    it("should gray out weekday time inputs while the weekday is not enabled", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      fireEvent.click(screen.getByTestId("settings-auto-run-feature-toggle"));

      const mondayStartInput = screen.getByTestId(
        "settings-auto-run-day-start-mon",
      ) as HTMLInputElement;
      const mondayEndInput = screen.getByTestId(
        "settings-auto-run-day-end-mon",
      ) as HTMLInputElement;
      const mondayDayRow = screen.getByTestId("settings-auto-run-day-row-mon");

      expect(mondayStartInput).toBeDisabled();
      expect(mondayEndInput).toBeDisabled();
      expect(mondayStartInput).toHaveClass("text-gray-400");
      expect(mondayEndInput).toHaveClass("text-gray-400");
      expect(mondayDayRow).toHaveClass("bg-gray-50");
    });
  });

  describe("Modal Styling", () => {
    it("should have overlay background", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("settingsModalOverlay");
    });

    it("should be positioned as fixed overlay", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("settingsModalOverlay");
    });

    it("should have high z-index", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("settingsModalOverlay");
    });

    it("should render a scrollable modal body area", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const body = screen.getByTestId("settings-modal-body");
      expect(body).toHaveClass("settingsModalBody");
    });

    it("should keep footer as a non-scrolling area", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const footer = screen.getByTestId("settings-modal-footer");
      expect(footer).toHaveClass("settingsModalFooter");
    });

    it("should constrain dialog height for small viewports", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const dialog = screen.getByTestId("settings-modal-dialog");
      expect(dialog).toHaveClass("settingsModalDialog");
    });
  });

  describe("Accessibility", () => {
    it("should expose dialog role and modal attributes", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const dialog = screen.getByRole("dialog", { name: "設定" });
      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(dialog).toHaveAttribute("aria-labelledby");
    });

    it("should focus cancel button when opened", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const cancelButton = screen.getByTestId("settings-cancel");
      expect(cancelButton).toHaveFocus();
    });

    it("should close when Escape key is pressed", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const dialog = screen.getByRole("dialog", { name: "設定" });
      fireEvent.keyDown(dialog, { key: "Escape" });
      expect(mockOnClose).toHaveBeenCalledTimes(1);
    });

    it("should trap focus forward with Tab from last element to first element", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const saveButton = screen.getByTestId("settings-save");
      const workInput = screen.getByTestId("settings-work-duration-input");

      saveButton.focus();
      expect(saveButton).toHaveFocus();

      fireEvent.keyDown(saveButton, { key: "Tab" });
      expect(workInput).toHaveFocus();
    });

    it("should trap focus backward with Shift+Tab from first element to last element", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const saveButton = screen.getByTestId("settings-save");
      const workInput = screen.getByTestId("settings-work-duration-input");

      workInput.focus();
      expect(workInput).toHaveFocus();

      fireEvent.keyDown(workInput, { key: "Tab", shiftKey: true });
      expect(saveButton).toHaveFocus();
    });

    it("should expose live region for preview playback status", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const previewStatusRegion = screen.getByTestId(
        "settings-notification-sound-preview-status",
      );
      expect(previewStatusRegion).toHaveAttribute("aria-live", "polite");
    });
  });

  describe("Scroll Lock", () => {
    it("should lock body scroll while modal is open", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(document.body.style.overflow).toBe("hidden");
      expect(document.body.style.overscrollBehavior).toBe("contain");
    });

    it("should restore body scroll styles when modal closes", () => {
      const { rerender } = render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(document.body.style.overflow).toBe("hidden");
      expect(document.body.style.overscrollBehavior).toBe("contain");

      rerender(<SettingsModal isOpen={false} onClose={mockOnClose} />);
      expect(document.body.style.overflow).toBe("");
      expect(document.body.style.overscrollBehavior).toBe("");
    });
  });

  describe("Edge Cases", () => {
    it("should not save when save button is disabled", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "0" } });

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(25); // Unchanged
      expect(mockOnClose).not.toHaveBeenCalled();
    });

    it("should handle boundary values correctly", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      // Set to minimum valid values
      fireEvent.change(workInput, { target: { value: "1" } });
      fireEvent.change(intervalInput, { target: { value: "2" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();

      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(1);
      expect(state.longBreakInterval).toBe(2);
    });

    it("should handle maximum valid values", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      // Set to maximum valid values
      fireEvent.change(workInput, { target: { value: "120" } });
      fireEvent.change(intervalInput, { target: { value: "10" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();

      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(120);
      expect(state.longBreakInterval).toBe(10);
    });
  });
});
