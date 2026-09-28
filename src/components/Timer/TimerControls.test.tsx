import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { TimerControls } from "./TimerControls";
import { useTimerStore } from "@/store/timerStore";
import { TimerStatus, SessionType } from "@/types/timer";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

function getInvokeCall(callIndex: number): {
  command: string;
  payload: Record<string, unknown> | undefined;
} {
  const invokeCall: (typeof mockInvoke.mock.calls)[number] | undefined =
    mockInvoke.mock.calls[callIndex];
  expect(invokeCall).toBeDefined();
  if (invokeCall === undefined) {
    throw new Error(`Expected invoke call at index ${String(callIndex)}`);
  }

  const [command, payload] = invokeCall;
  return { command, payload };
}

describe("TimerControls", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(createTimerSnapshotForTest());
    // Reset store to initial state before each test
    useTimerStore.setState({
      status: TimerStatus.IDLE,
      remainingSeconds: 25 * 60,
      sessionType: SessionType.WORK,
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
      isConnected: false,
    });
  });

  describe("Initial State (IDLE)", () => {
    it("should render a named start button when timer is idle", () => {
      render(<TimerControls />);
      const startButton = screen.getByRole("button", {
        name: "タイマーを開始",
      });
      expect(startButton).toHaveTextContent("開始");
      expect(startButton).toBeInTheDocument();
    });

    it("should not render stop button when timer is idle", () => {
      render(<TimerControls />);
      const stopButton = screen.queryByRole("button", { name: "緊急停止" });
      expect(stopButton).not.toBeInTheDocument();
    });

    it("should call startWork when start button is clicked", async () => {
      const user = userEvent.setup();
      render(<TimerControls />);

      const startButton = screen.getByRole("button", {
        name: "タイマーを開始",
      });
      await user.click(startButton);

      const { command, payload } = getInvokeCall(0);
      expect(command).toBe("start_work");
      // The backend starts the session with the settings it owns.
      expect(payload).toBeUndefined();
    });
  });

  describe("Working State", () => {
    beforeEach(() => {
      useTimerStore.setState({ status: TimerStatus.WORKING });
    });

    it("should not render start button when timer is working", () => {
      render(<TimerControls />);
      const startButton = screen.queryByRole("button", {
        name: "タイマーを開始",
      });
      expect(startButton).not.toBeInTheDocument();
    });

    it("should render emergency stop button when timer is working", () => {
      render(<TimerControls />);
      const emergencyStopButton = screen.getByRole("button", {
        name: "緊急停止",
      });
      expect(emergencyStopButton).toHaveTextContent("緊急停止");
      expect(emergencyStopButton).toBeInTheDocument();
    });

    it("should open emergency stop dialog when button is clicked", async () => {
      const user = userEvent.setup();
      render(<TimerControls />);

      await user.click(screen.getByRole("button", { name: "緊急停止" }));

      expect(
        screen.getByRole("dialog", { name: "緊急停止しますか？" }),
      ).toBeInTheDocument();
    });
  });

  describe("State Transitions", () => {
    it("should hide start button after starting (backend updates state)", () => {
      const { rerender } = render(<TimerControls />);

      // Initially shows start button
      expect(
        screen.getByRole("button", { name: "タイマーを開始" }),
      ).toBeInTheDocument();

      // Simulate backend response updating state
      act(() => {
        useTimerStore.setState({ status: TimerStatus.WORKING });
      });

      // Rerender to reflect state change
      rerender(<TimerControls />);

      // Start button is hidden (no buttons shown during working)
      expect(
        screen.queryByRole("button", { name: "タイマーを開始" }),
      ).not.toBeInTheDocument();
    });

    it("should show start button again after emergency stop (backend updates state)", () => {
      const { rerender } = render(<TimerControls />);

      // Simulate working state from backend
      act(() => {
        useTimerStore.setState({ status: TimerStatus.WORKING });
      });
      rerender(<TimerControls />);
      expect(
        screen.queryByRole("button", { name: "タイマーを開始" }),
      ).not.toBeInTheDocument();

      // Simulate emergency stop response from backend
      act(() => {
        useTimerStore.setState({ status: TimerStatus.IDLE });
      });
      rerender(<TimerControls />);

      // Start button is visible again
      expect(
        screen.getByRole("button", { name: "タイマーを開始" }),
      ).toBeInTheDocument();
    });
  });
});
