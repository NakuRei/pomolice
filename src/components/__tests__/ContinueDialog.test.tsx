import { describe, it, expect, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContinueDialog } from "../ContinueDialog/ContinueDialog";
import { useTimerStore, TimerStatus } from "../../store/timerStore";

describe("ContinueDialog", () => {
  beforeEach(() => {
    useTimerStore.getState().reset();
  });

  describe("Visibility", () => {
    it("should not render when showContinueDialog is false", () => {
      render(<ContinueDialog />);
      const dialog = screen.queryByTestId("continue-dialog");
      expect(dialog).not.toBeInTheDocument();
    });

    it("should render when showContinueDialog is true", () => {
      useTimerStore.setState({ showContinueDialog: true });
      render(<ContinueDialog />);
      const dialog = screen.getByTestId("continue-dialog");
      expect(dialog).toBeInTheDocument();
    });
  });

  describe("Dialog Content", () => {
    beforeEach(() => {
      useTimerStore.setState({
        showContinueDialog: true,
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

    it("should have continue and end buttons", () => {
      render(<ContinueDialog />);
      expect(screen.getByTestId("continue-dialog-continue")).toBeInTheDocument();
      expect(screen.getByTestId("continue-dialog-end")).toBeInTheDocument();
    });
  });

  describe("Button Actions", () => {
    beforeEach(() => {
      useTimerStore.setState({
        showContinueDialog: true,
        status: TimerStatus.IDLE,
        currentCycle: 2,
        completedPomodoros: 2,
      });
    });

    it("should start next session when continue button is clicked", async () => {
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const continueButton = screen.getByTestId("continue-dialog-continue");
      await user.click(continueButton);

      const state = useTimerStore.getState();
      expect(state.status).toBe(TimerStatus.RUNNING);
      expect(state.currentCycle).toBe(3); // Incremented
      expect(state.showContinueDialog).toBe(false);
    });

    it("should end cycle when end button is clicked", async () => {
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const endButton = screen.getByTestId("continue-dialog-end");
      await user.click(endButton);

      const state = useTimerStore.getState();
      expect(state.status).toBe(TimerStatus.IDLE);
      expect(state.showContinueDialog).toBe(false);
      expect(state.completedPomodoros).toBe(2); // Preserved
    });
  });

  describe("Dynamic Updates", () => {
    it("should update pomodoro count display", () => {
      useTimerStore.setState({
        showContinueDialog: true,
        completedPomodoros: 5,
      });
      const { rerender } = render(<ContinueDialog />);

      expect(screen.getByText("5")).toBeInTheDocument();

      // Update count
      useTimerStore.setState({ completedPomodoros: 10 });
      rerender(<ContinueDialog />);

      expect(screen.getByText("10")).toBeInTheDocument();
    });

    it("should hide when showContinueDialog becomes false", () => {
      useTimerStore.setState({ showContinueDialog: true });
      const { rerender } = render(<ContinueDialog />);

      expect(screen.getByTestId("continue-dialog")).toBeInTheDocument();

      // Hide dialog
      useTimerStore.setState({ showContinueDialog: false });
      rerender(<ContinueDialog />);

      expect(screen.queryByTestId("continue-dialog")).not.toBeInTheDocument();
    });
  });

  describe("Button Labels", () => {
    beforeEach(() => {
      useTimerStore.setState({ showContinueDialog: true });
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

  describe("Dialog Styling", () => {
    beforeEach(() => {
      useTimerStore.setState({ showContinueDialog: true });
    });

    it("should have overlay background", () => {
      render(<ContinueDialog />);
      const dialog = screen.getByTestId("continue-dialog");
      expect(dialog).toHaveClass("bg-black/50");
    });

    it("should be positioned as fixed overlay", () => {
      render(<ContinueDialog />);
      const dialog = screen.getByTestId("continue-dialog");
      expect(dialog).toHaveClass("fixed");
      expect(dialog).toHaveClass("inset-0");
    });

    it("should have high z-index", () => {
      render(<ContinueDialog />);
      const dialog = screen.getByTestId("continue-dialog");
      expect(dialog).toHaveClass("z-50");
    });
  });

  describe("Edge Cases", () => {
    it("should handle zero completed pomodoros", () => {
      useTimerStore.setState({
        showContinueDialog: true,
        completedPomodoros: 0,
      });
      render(<ContinueDialog />);

      expect(screen.getByText("0")).toBeInTheDocument();
    });

    it("should handle large pomodoro counts", () => {
      useTimerStore.setState({
        showContinueDialog: true,
        completedPomodoros: 100,
      });
      render(<ContinueDialog />);

      expect(screen.getByText("100")).toBeInTheDocument();
    });
  });
});
