import { describe, it, expect, beforeEach, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { useErrorStore } from "../../store/errorStore";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("EmergencyStopButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to initial state
    useErrorStore.getState().clearAllErrors();
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
    });
  });

  describe("Visibility", () => {
    it("should not render when timer is idle", () => {
      render(<EmergencyStopButton />);
      const button = screen.queryByTestId("emergency-stop-button");
      expect(button).not.toBeInTheDocument();
    });

    it("should render when timer is working", () => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toBeInTheDocument();
    });
  });

  describe("Button Appearance", () => {
    it("should have correct text", () => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveTextContent("緊急停止");
    });

    it("should have aria-label", () => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveAttribute("aria-label", "緊急停止");
    });
  });

  describe("Confirmation Dialog", () => {
    it("should show dialog when button is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      const button = screen.getByTestId("emergency-stop-button");
      await user.click(button);

      const dialog = screen.getByTestId("emergency-stop-dialog");
      expect(dialog).toBeInTheDocument();
    });

    it("should show correct dialog title", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(screen.getByText("緊急停止しますか？")).toBeInTheDocument();
    });

    it("should expose the confirmation panel as an accessible dialog", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      const dialog = screen.getByRole("dialog");

      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(dialog).toHaveAttribute("aria-labelledby", "emergency-stop-dialog-title");
    });

    it("should show warning message", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(
        screen.getByText(/現在のポモドーロセッションは記録されません/)
      ).toBeInTheDocument();
    });

    it("should have cancel and confirm buttons", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(screen.getByTestId("emergency-stop-cancel")).toBeInTheDocument();
      expect(screen.getByTestId("emergency-stop-confirm")).toBeInTheDocument();
    });
  });

  describe("Dialog Actions", () => {
    it("should close dialog when cancel is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      expect(screen.getByTestId("emergency-stop-dialog")).toBeInTheDocument();

      await user.click(screen.getByTestId("emergency-stop-cancel"));

      await waitFor(() => {
        expect(
          screen.queryByTestId("emergency-stop-dialog")
        ).not.toBeInTheDocument();
      });
    });

    it("should focus the cancel button when the dialog opens", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      await waitFor(() => {
        expect(screen.getByTestId("emergency-stop-cancel")).toHaveFocus();
      });
    });

    it("should close the dialog on Escape without calling the backend", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => {
        expect(
          screen.queryByTestId("emergency-stop-dialog")
        ).not.toBeInTheDocument();
      });
      expect(invoke).not.toHaveBeenCalled();
    });

    it("should not change timer status when cancel is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-cancel"));

      expect(useTimerStore.getState().status).toBe(TimerStatus.WORKING);
    });

    it("should close dialog after confirming", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      await waitFor(() => {
        expect(
          screen.queryByTestId("emergency-stop-dialog")
        ).not.toBeInTheDocument();
      });
    });

    it("should keep the dialog open and show an error when confirm fails", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockRejectedValueOnce("Failed to lock state: busy");

      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      await waitFor(() => {
        expect(screen.getByTestId("emergency-stop-dialog")).toBeInTheDocument();
      });
      expect(screen.getByTestId("inline-error-emergency_stop")).toHaveTextContent(
        "緊急停止に失敗しました。もう一度試してください"
      );
    });
  });

  describe("Emergency Stop Behavior", () => {
    it("should call backend emergency_stop command", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      useTimerStore.setState({
        status: TimerStatus.WORKING,
        remainingSeconds: 500,
        currentCycle: 2,
      });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      expect(invoke).toHaveBeenCalledWith("emergency_stop");
    });

    it("should preserve pomodoro count (handled by backend)", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({
        status: TimerStatus.WORKING,
        completedPomodoros: 3,
        currentCycle: 3,
      });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      // The backend handles state, so pomodoro count is preserved until backend responds
      const state = useTimerStore.getState();
      expect(state.completedPomodoros).toBe(3);
    });
  });
});
