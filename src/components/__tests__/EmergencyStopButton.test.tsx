import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmergencyStopButton } from "../EmergencyStop/EmergencyStopButton";
import { useTimerStore, TimerStatus } from "../../store/timerStore";

describe("EmergencyStopButton", () => {
  beforeEach(() => {
    useTimerStore.getState().reset();
  });

  describe("Visibility", () => {
    it("should not render when timer is idle", () => {
      render(<EmergencyStopButton />);
      const button = screen.queryByTestId("emergency-stop-button");
      expect(button).not.toBeInTheDocument();
    });

    it("should render when timer is running", () => {
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toBeInTheDocument();
    });
  });

  describe("Button Appearance", () => {
    it("should have correct text", () => {
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveTextContent("緊急停止");
    });

    it("should have aria-label", () => {
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveAttribute("aria-label", "緊急停止");
    });

    it("should have small, subtle styling", () => {
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);
      const button = screen.getByTestId("emergency-stop-button");
      expect(button).toHaveClass("text-xs");
      expect(button).toHaveClass("text-gray-400");
    });
  });

  describe("Confirmation Dialog", () => {
    it("should show dialog when button is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      const button = screen.getByTestId("emergency-stop-button");
      await user.click(button);

      const dialog = screen.getByTestId("emergency-stop-dialog");
      expect(dialog).toBeInTheDocument();
    });

    it("should show correct dialog title", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(screen.getByText("緊急停止しますか？")).toBeInTheDocument();
    });

    it("should show warning message", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(
        screen.getByText(/現在のポモドーロセッションは記録されません/)
      ).toBeInTheDocument();
    });

    it("should have cancel and confirm buttons", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));

      expect(screen.getByTestId("emergency-stop-cancel")).toBeInTheDocument();
      expect(screen.getByTestId("emergency-stop-confirm")).toBeInTheDocument();
    });
  });

  describe("Dialog Actions", () => {
    it("should close dialog when cancel is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
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

    it("should not stop timer when cancel is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-cancel"));

      expect(useTimerStore.getState().status).toBe(TimerStatus.RUNNING);
    });

    it("should stop timer when confirm is clicked", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      expect(useTimerStore.getState().status).toBe(TimerStatus.IDLE);
    });

    it("should close dialog after confirming", async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
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
    it("should reset timer to idle state", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({
        status: TimerStatus.RUNNING,
        remainingSeconds: 500,
        currentCycle: 2,
      });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      const state = useTimerStore.getState();
      expect(state.status).toBe(TimerStatus.IDLE);
      expect(state.remainingSeconds).toBe(25 * 60); // Reset to default
    });

    it("should preserve pomodoro count for statistics", async () => {
      const user = userEvent.setup();
      useTimerStore.setState({
        status: TimerStatus.RUNNING,
        completedPomodoros: 3,
        currentCycle: 3,
      });
      render(<EmergencyStopButton />);

      await user.click(screen.getByTestId("emergency-stop-button"));
      await user.click(screen.getByTestId("emergency-stop-confirm"));

      const state = useTimerStore.getState();
      expect(state.completedPomodoros).toBe(3); // Preserved
      expect(state.currentCycle).toBe(3); // Preserved
    });
  });
});
