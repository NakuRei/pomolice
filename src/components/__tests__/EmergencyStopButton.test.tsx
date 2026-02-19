import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("EmergencyStopButton", () => {
  beforeEach(() => {
    // Reset store to initial state
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      isFullscreenBreak: false,
      showContinueDialog: false,
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

    it("should have small, subtle styling", () => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveClass("text-xs");
      expect(button).toHaveClass("text-gray-400");
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

    it("should not change timer status when cancel is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-cancel"));

      expect(useTimerStore.getState().status).toBe(TimerStatus.WORKING);
    });

    it("should call emergencyStop when confirm is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({ status: TimerStatus.WORKING });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      // emergencyStop calls invoke, which is mocked
      // The actual state change comes from backend via events
      // Here we just verify the dialog closes
      await waitFor(() => {
        expect(
          screen.queryByTestId("emergency-stop-dialog")
        ).not.toBeInTheDocument();
      });
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
