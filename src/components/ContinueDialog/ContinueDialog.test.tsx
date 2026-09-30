import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  render,
  screen,
  act,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ContinueDialog } from "./ContinueDialog";
import { useTimerStore } from "@/store/timerStore";

import { resetTimerStoreForTest } from "@/test/resetTimerStore";

type ResolvePostBreakDecision = ReturnType<
  typeof useTimerStore.getState
>["resolvePostBreakDecision"];

function showContinueDialog(completedPomodoros = 3): void {
  useTimerStore.setState({
    requiresManualContinueDecision: true,
    completedPomodoros,
  });
}

function getContinueDialog(): HTMLElement {
  return screen.getByRole("dialog", { name: "1サイクル完了しました！" });
}

function replacePostBreakDecisionActionForTest(): ResolvePostBreakDecision {
  const resolvePostBreakDecision = vi
    .fn<ResolvePostBreakDecision>()
    .mockResolvedValue(undefined);
  useTimerStore.setState({ resolvePostBreakDecision });
  return resolvePostBreakDecision;
}

describe("ContinueDialog", () => {
  beforeEach(() => {
    resetTimerStoreForTest();
    useTimerStore.setState({
      status: "idle",
      remainingSeconds: 25 * 60,
      sessionType: "work",
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
      isConnected: false,
    });
  });

  describe("Visibility", () => {
    it("should not render when requiresManualContinueDecision is false", () => {
      render(<ContinueDialog />);
      const dialog = screen.queryByRole("dialog", {
        name: "1サイクル完了しました！",
      });
      expect(dialog).not.toBeInTheDocument();
    });

    it("should render an accessible dialog with a labelled title when a manual decision is required", () => {
      showContinueDialog();
      render(<ContinueDialog />);

      const dialog = getContinueDialog();

      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(dialog).toHaveAccessibleName("1サイクル完了しました！");
    });
  });

  describe("Dialog Content", () => {
    beforeEach(() => {
      showContinueDialog(3);
    });

    it("should display the completion summary, manual decision notice, and actions", () => {
      render(<ContinueDialog />);
      const dialog = getContinueDialog();

      expect(dialog).toHaveTextContent("お疲れ様でした。休憩が完了しました。");
      expect(dialog).toHaveTextContent(
        "現在、手動判断待ちです。続けるか終了を選んでください。",
      );
      expect(dialog).toHaveTextContent("完了したポモドーロ: 3");
      expect(
        within(dialog).getByText("次のポモドーロを続けますか？"),
      ).toBeInTheDocument();
      expect(
        within(dialog).getByRole("button", { name: "続ける" }),
      ).toBeInTheDocument();
      expect(
        within(dialog).getByRole("button", { name: "終了" }),
      ).toBeInTheDocument();
    });
  });

  describe("Button Actions", () => {
    beforeEach(() => {
      useTimerStore.setState({
        requiresManualContinueDecision: true,
        status: "idle",
        currentCycle: 2,
        completedPomodoros: 2,
      });
    });

    it("should let the user continue after the break", async () => {
      const resolvePostBreakDecision = replacePostBreakDecisionActionForTest();
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const continueButton = screen.getByRole("button", { name: "続ける" });
      await user.click(continueButton);

      expect(resolvePostBreakDecision).toHaveBeenCalledWith("continue");
    });

    it("should let the user end the cycle after the break", async () => {
      const resolvePostBreakDecision = replacePostBreakDecisionActionForTest();
      const user = userEvent.setup();
      render(<ContinueDialog />);

      const endButton = screen.getByRole("button", { name: "終了" });
      await user.click(endButton);

      expect(resolvePostBreakDecision).toHaveBeenCalledWith("stop_cycle");
    });

    it("should focus the continue button when the dialog opens", async () => {
      showContinueDialog();
      render(<ContinueDialog />);

      const continueButton = screen.getByRole("button", { name: "続ける" });

      await waitFor(() => {
        expect(continueButton).toHaveFocus();
      });
    });

    it("should treat Escape as choosing to end the cycle", async () => {
      const resolvePostBreakDecision = replacePostBreakDecisionActionForTest();
      showContinueDialog();
      render(<ContinueDialog />);

      fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => {
        expect(resolvePostBreakDecision).toHaveBeenCalledWith("stop_cycle");
      });
    });
  });

  describe("Dynamic Updates", () => {
    it("should update pomodoro count display", () => {
      showContinueDialog(5);
      render(<ContinueDialog />);

      expect(getContinueDialog()).toHaveTextContent("完了したポモドーロ: 5");

      act(() => {
        useTimerStore.setState({ completedPomodoros: 10 });
      });

      expect(getContinueDialog()).toHaveTextContent("完了したポモドーロ: 10");
    });

    it("should hide when requiresManualContinueDecision becomes false", () => {
      showContinueDialog();
      render(<ContinueDialog />);

      expect(getContinueDialog()).toBeInTheDocument();

      act(() => {
        useTimerStore.setState({ requiresManualContinueDecision: false });
      });

      expect(
        screen.queryByRole("dialog", { name: "1サイクル完了しました！" }),
      ).not.toBeInTheDocument();
    });
  });

  describe("Edge Cases", () => {
    it("should handle zero completed pomodoros", () => {
      showContinueDialog(0);
      render(<ContinueDialog />);

      expect(getContinueDialog()).toHaveTextContent("完了したポモドーロ: 0");
    });

    it("should handle large pomodoro counts", () => {
      showContinueDialog(100);
      render(<ContinueDialog />);

      expect(getContinueDialog()).toHaveTextContent("完了したポモドーロ: 100");
    });
  });
});
