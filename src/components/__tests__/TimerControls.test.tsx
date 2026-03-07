import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimerControls } from "../Timer/TimerControls";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("TimerControls", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
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

  describe("Initial State (IDLE)", () => {
    it("should render start button when timer is idle", () => {
      render(<TimerControls />);
      const startButton = screen.getByTestId("start-button");
      expect(startButton).toBeInTheDocument();
      expect(startButton).toHaveTextContent("開始");
    });

    it("should not render stop button when timer is idle", () => {
      render(<TimerControls />);
      const stopButton = screen.queryByTestId("stop-button");
      expect(stopButton).not.toBeInTheDocument();
    });

    it("should call startWork when start button is clicked", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      render(<TimerControls />);

      const startButton = screen.getByTestId("start-button");
      await user.click(startButton);

      expect(invoke).toHaveBeenCalledWith(
        "start_work",
        expect.objectContaining({
          workDurationSecs: expect.any(Number),
          shortBreakDurationSecs: expect.any(Number),
          longBreakDurationSecs: expect.any(Number),
          longBreakInterval: expect.any(Number),
          postBreakTransitionPolicy: "require_manual_continue",
        })
      );
    });
  });

  describe("Working State", () => {
    beforeEach(() => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
    });

    it("should not render start button when timer is working", () => {
      render(<TimerControls />);
      const startButton = screen.queryByTestId("start-button");
      expect(startButton).not.toBeInTheDocument();
    });
  });

  describe("Button Accessibility", () => {
    it("should have aria-label on start button", () => {
      render(<TimerControls />);
      const startButton = screen.getByTestId("start-button");
      expect(startButton).toHaveAttribute("aria-label", "タイマーを開始");
    });
  });

  describe("State Transitions", () => {
    it("should hide start button after starting (backend updates state)", async () => {
      const { rerender } = render(<TimerControls />);

      // Initially shows start button
      expect(screen.getByTestId("start-button")).toBeInTheDocument();

      // Simulate backend response updating state
      act(() => {
        useTimerStore.setState({ status: TimerStatus.WORKING });
      });

      // Rerender to reflect state change
      rerender(<TimerControls />);

      // Start button is hidden (no buttons shown during working)
      expect(screen.queryByTestId("start-button")).not.toBeInTheDocument();
    });

    it("should show start button again after emergency stop (backend updates state)", async () => {
      const { rerender } = render(<TimerControls />);

      // Simulate working state from backend
      act(() => {
        useTimerStore.setState({ status: TimerStatus.WORKING });
      });
      rerender(<TimerControls />);
      expect(screen.queryByTestId("start-button")).not.toBeInTheDocument();

      // Simulate emergency stop response from backend
      act(() => {
        useTimerStore.setState({ status: TimerStatus.IDLE });
      });
      rerender(<TimerControls />);

      // Start button is visible again
      expect(screen.getByTestId("start-button")).toBeInTheDocument();
    });
  });
});
