import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, act, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContinueDialog } from "../ContinueDialog/ContinueDialog";
import { useTimerStore, TimerStatus, SessionType } from "../../store/timerStore";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

describe("ContinueDialog", () => {
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
      requiresManualContinueDecision: false,
      isConnected: false,
    });
  });

  describe("Visibility", () => {
    it("should not render when requiresManualContinueDecision is false", () => {
      render(<ContinueDialog />);
      const dialog = screen.queryByTestId("continue-dialog");
      expect(dialog).not.toBeInTheDocument();
    });

    it("should render when requiresManualContinueDecision is true", () => {
      useTimerStore.setState({ requiresManualContinueDecision: true });
      render(<ContinueDialog />);
      const dialog = screen.getByTestId("continue-dialog");
      expect(dialog).toBeInTheDocument();
    });

    it("should expose the panel as an accessible dialog with a labelled title", () => {
      useTimerStore.setState({ requiresManualContinueDecision: true });
      render(<ContinueDialog />);

      const dialog = screen.getByRole("dialog");

      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(dialog).toHaveAttribute("aria-labelledby", "continue-dialog-title");
    });
  });

  describe("Dialog Content", () => {
    beforeEach(() => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        completedPomodoros: 3,
      });
    });

    it("should display completion message", () => {
      render(<ContinueDialog />);
      expect(screen.getByText("1サイクル完了しました！")).toBeInTheDocument();
    });

    it("should display completion confirmation", () => {
      render(<ContinueDialog />);
      expect(screen.getByText("お疲れ様でした。休憩が完了しました。")).toBeInTheDocument();
    });

    it("should display completed pomodoros count", () => {
      render(<ContinueDialog />);
      expect(screen.getByText(/完了したポモドーロ:/)).toBeInTheDocument();
      expect(screen.getByText("3")).toBeInTheDocument();
    });

    it("should display continue question", () => {
      render(<ContinueDialog />);
      expect(screen.getByText("次のポモドーロを続けますか？")).toBeInTheDocument();
    });

    it("should display manual decision pending explanation", () => {
      render(<ContinueDialog />);
      expect(
        screen.getByTestId("continue-dialog-manual-decision-message")
      ).toHaveTextContent(
        "現在、手動判断待ちです。続けるか終了を選んでください。"
      );
    });

    it("should have continue and end buttons", () => {
      render(<ContinueDialog />);
      expect(screen.getByTestId("continue-dialog-continue")).toBeInTheDocument();
      expect(screen.getByTestId("continue-dialog-end")).toBeInTheDocument();
    });
  });

  describe("Button Actions", () => {
    beforeEach(() => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        status: TimerStatus.IDLE,
        currentCycle: 2,
        completedPomodoros: 2,
      });
    });

    it("should call resolvePostBreakDecision with continue when continue button is clicked", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const continueButton = screen.getByTestId("continue-dialog-continue");
      await user.click(continueButton);

      // Backend receives the unified manual decision command.
      expect(invoke).toHaveBeenCalledWith("resolve_post_break_decision", {
        action: "continue",
      });
    });

    it("should call resolvePostBreakDecision with stop_cycle when end button is clicked", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const endButton = screen.getByTestId("continue-dialog-end");
      await user.click(endButton);

      // Backend receives the unified manual decision command.
      expect(invoke).toHaveBeenCalledWith("resolve_post_break_decision", {
        action: "stop_cycle",
      });
    });

    it("should focus the continue button when the dialog opens", async () => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
      });
      render(<ContinueDialog />);

      const continueButton = screen.getByTestId("continue-dialog-continue");

      await waitFor(() => {
        expect(continueButton).toHaveFocus();
      });
    });

    it("should treat Escape as choosing stop_cycle", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      useTimerStore.setState({
        requiresManualContinueDecision: true,
      });
      render(<ContinueDialog />);

      fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => {
        expect(invoke).toHaveBeenCalledWith("resolve_post_break_decision", {
          action: "stop_cycle",
        });
      });
    });
  });

  describe("Dynamic Updates", () => {
    it("should update pomodoro count display", () => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        completedPomodoros: 5,
      });
      const { rerender } = render(<ContinueDialog />);

      expect(screen.getByText("5")).toBeInTheDocument();

      // Update count
      act(() => {
        useTimerStore.setState({ completedPomodoros: 10 });
      });
      rerender(<ContinueDialog />);

      expect(screen.getByText("10")).toBeInTheDocument();
    });

    it("should hide when requiresManualContinueDecision becomes false", () => {
      useTimerStore.setState({ requiresManualContinueDecision: true });
      const { rerender } = render(<ContinueDialog />);

      expect(screen.getByTestId("continue-dialog")).toBeInTheDocument();

      // Hide dialog
      act(() => {
        useTimerStore.setState({ requiresManualContinueDecision: false });
      });
      rerender(<ContinueDialog />);

      expect(screen.queryByTestId("continue-dialog")).not.toBeInTheDocument();
    });
  });

  describe("Button Labels", () => {
    beforeEach(() => {
      useTimerStore.setState({ requiresManualContinueDecision: true });
    });

    it("should have correct text on continue button", () => {
      render(<ContinueDialog />);
      const continueButton = screen.getByTestId("continue-dialog-continue");
      expect(continueButton).toHaveTextContent("続ける");
    });

    it("should have correct text on end button", () => {
      render(<ContinueDialog />);
      const endButton = screen.getByTestId("continue-dialog-end");
      expect(endButton).toHaveTextContent("終了");
    });
  });

  describe("Edge Cases", () => {
    it("should handle zero completed pomodoros", () => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        completedPomodoros: 0,
      });
      render(<ContinueDialog />);

      expect(screen.getByText("0")).toBeInTheDocument();
    });

    it("should handle large pomodoro counts", () => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        completedPomodoros: 100,
      });
      render(<ContinueDialog />);

      expect(screen.getByText("100")).toBeInTheDocument();
    });
  });
});
