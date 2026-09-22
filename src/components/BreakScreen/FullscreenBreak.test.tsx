import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

const mockInvoke = vi.fn<InvokeCommand>();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>): Promise<unknown> =>
    mockInvoke(command, args),
}));

import { FullscreenBreak } from "./FullscreenBreak";
import { useEmergencyShortcutStore } from "@/store/emergencyShortcutStore";
import { useSettingsStore } from "@/store/settingsStore";
import { useTimerStore, SessionType, TimerStatus } from "@/store/timerStore";
import { useSkipFlowStore } from "@/store/skipFlowStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { breakMotivationalMessages } from "@/data/motivationalMessages";
import {
  LONG_TYPING_PHRASES,
  SHORT_TYPING_PHRASES,
} from "@/data/typingPhrases";

function getBreakTimerDisplay(timeText: string): HTMLElement {
  return screen.getByText(timeText);
}

function getVisibleBreakMotivationalMessages(): string[] {
  return breakMotivationalMessages.filter(
    (message) => screen.queryByText(message) !== null,
  );
}

function getVisibleBreakMotivationalMessage(): string {
  const visibleMessages = getVisibleBreakMotivationalMessages();
  expect(visibleMessages).toHaveLength(1);

  const visibleMessage = visibleMessages[0];
  if (visibleMessage === undefined) {
    throw new Error("Expected one break motivational message to be visible.");
  }

  return visibleMessage;
}

async function renderFullscreenBreakWithSkipFlowOpen(): Promise<void> {
  const user = userEvent.setup();
  render(<FullscreenBreak />);

  await user.click(screen.getByRole("button", { name: "休憩をスキップ" }));
}

function renderedPhraseFrom(bank: readonly string[]): string {
  const phrase = bank.find(
    (candidate) =>
      screen.queryAllByText((_, element) => element?.textContent === candidate)
        .length > 0,
  );
  if (phrase === undefined) {
    throw new Error("Expected a typing challenge phrase");
  }
  return phrase;
}

function solveDisplayedProblem(): number {
  const problemText = screen.getByText(/[=]\s*\?/u).textContent;
  const groups = /(?<left>-?\d+)\s*(?<operator>[+\-×])\s*(?<right>-?\d+)/u.exec(
    problemText,
  )?.groups;
  if (groups?.operator === undefined) {
    throw new Error(
      `Expected a solvable math problem, received ${problemText}`,
    );
  }

  const left = Number(groups.left);
  const right = Number(groups.right);
  switch (groups.operator) {
    case "+":
      return left + right;
    case "-":
      return left - right;
    case "×":
      return left * right;
    default:
      throw new Error(`Unexpected math operator ${groups.operator}`);
  }
}

function completeTypingChallenge(bank: readonly string[]): void {
  fireEvent.change(screen.getByPlaceholderText("Start typing here..."), {
    target: { value: renderedPhraseFrom(bank) },
  });
}

function completeMathChallenge(): void {
  const answerInput = screen.getByPlaceholderText("Your answer");
  fireEvent.change(answerInput, {
    target: { value: String(solveDisplayedProblem()) },
  });
  const answerForm = answerInput.closest("form");
  if (answerForm === null) {
    throw new Error("Expected the answer input inside a form");
  }
  fireEvent.submit(answerForm);
}

describe("FullscreenBreak", () => {
  beforeEach(() => {
    useEmergencyShortcutStore.setState({
      status: { kind: "input_monitor" },
      connectionFailed: false,
    });
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue(undefined);
    resetTimerStoreForTest();
    useSkipFlowStore.getState().resetSkipFlow();
    useSettingsStore.getState().resetToDefaults();
    useSettingsStore.setState({
      durationSettingsApplicationStatus: "confirmed",
      automationSettingsApplicationStatus: "confirmed",
    });
    useTimerStore.setState({
      sessionType: SessionType.SHORT_BREAK,
      remainingSeconds: 300,
      status: TimerStatus.ON_BREAK,
      isFullscreenBreak: true,
      isConnected: true,
      isSettingsSynchronizationInFlight: false,
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe("Rendering", () => {
    it.each([SessionType.SHORT_BREAK, SessionType.LONG_BREAK])(
      "shows emergency release instructions during %s without opening the skip flow",
      (sessionType) => {
        useTimerStore.setState({ sessionType });
        render(<FullscreenBreak />);
        const guidance = screen.getByText(/緊急解除/u);
        expect(guidance).toHaveTextContent("Ctrl + Alt + Shift + E");
      },
    );

    it("should render the initial short-break ui", () => {
      render(<FullscreenBreak />);

      expect(getBreakTimerDisplay("05:00")).toBeInTheDocument();
      expect(screen.getByText("短い休憩")).toBeInTheDocument();
      expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "休憩をスキップ" }),
      ).toBeInTheDocument();
      getVisibleBreakMotivationalMessage();
    });

    it("should display 'long break' for long break", () => {
      useTimerStore.setState({ sessionType: SessionType.LONG_BREAK });

      render(<FullscreenBreak />);

      expect(screen.getByText("長い休憩")).toBeInTheDocument();
    });

    it("should keep the initially visible motivational message when timer rerenders", () => {
      const { rerender } = render(<FullscreenBreak />);

      const initialMessage = getVisibleBreakMotivationalMessage();

      act(() => {
        useTimerStore.setState({ remainingSeconds: 299 });
      });
      rerender(<FullscreenBreak />);

      expect(getVisibleBreakMotivationalMessage()).toBe(initialMessage);
    });

    it("keeps unresolved settings visible without blocking the skip flow", async () => {
      useSettingsStore.setState({
        automationSettingsApplicationStatus: "unconfirmed",
      });

      await renderFullscreenBreakWithSkipFlowOpen();

      expect(screen.getByRole("alert")).toHaveTextContent(
        /適用を確認できませんでした/u,
      );
      expect(
        screen.queryByRole("button", { name: "設定の同期を再試行" }),
      ).not.toBeInTheDocument();
      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
      expect(screen.getByText(/No, Take Break/iu)).toBeInTheDocument();
    });

    it("keeps settings recovery informational while fullscreen remains enforced", () => {
      useSettingsStore.setState({
        automationSettingsApplicationStatus: "unconfirmed",
      });
      useTimerStore.getState().handleTimerTick({
        status: TimerStatus.IDLE,
        remaining_seconds: 300,
        session_type: SessionType.SHORT_BREAK,
        current_cycle: 1,
        completed_pomodoros: 0,
        break_interaction_count: 0,
        is_fullscreen_break: true,
        away_during_break: false,
      });

      render(<FullscreenBreak />);

      expect(useTimerStore.getState().status).toBe(TimerStatus.ON_BREAK);
      expect(screen.getByRole("alert")).toHaveTextContent(/休憩終了後/u);
      expect(
        screen.queryByRole("button", { name: "設定の同期を再試行" }),
      ).not.toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "休憩をスキップ" }),
      ).toBeInTheDocument();
    });

    it("keeps the complete skip flow usable while settings remain unresolved", () => {
      vi.useFakeTimers();
      useSettingsStore.setState({
        automationSettingsApplicationStatus: "unconfirmed",
      });
      render(<FullscreenBreak />);

      fireEvent.click(screen.getByRole("button", { name: "休憩をスキップ" }));
      for (let step = 1; step <= 4; step += 1) {
        expect(
          screen.getByText(`Step ${String(step)} of 10`),
        ).toBeInTheDocument();
        fireEvent.click(screen.getByRole("button", { name: "Yes, Continue" }));
      }

      completeTypingChallenge(SHORT_TYPING_PHRASES);
      completeMathChallenge();
      completeTypingChallenge(LONG_TYPING_PHRASES);
      completeMathChallenge();

      const longPressButton = screen.getByRole("button", {
        name: /Press and Hold/iu,
      });
      act(() => {
        fireEvent.mouseDown(longPressButton);
        vi.advanceTimersByTime(15_100);
      });

      expect(screen.getByText("Final Warning")).toBeInTheDocument();
      expect(screen.getByRole("alert")).toHaveTextContent(
        /適用を確認できませんでした/u,
      );
      fireEvent.click(screen.getByRole("button", { name: "Yes, Skip Anyway" }));

      expect(mockInvoke).toHaveBeenCalledTimes(1);
      expect(mockInvoke).toHaveBeenCalledWith("emergency_stop", undefined);
    });
  });

  describe("Timer Display", () => {
    it("should update when remaining seconds changes", () => {
      const { rerender } = render(<FullscreenBreak />);

      expect(getBreakTimerDisplay("05:00")).toBeInTheDocument();

      act(() => {
        useTimerStore.setState({ remainingSeconds: 60 }); // 1:00
      });
      rerender(<FullscreenBreak />);

      expect(screen.getByText("01:00")).toBeInTheDocument();
    });

    it("should handle single digit seconds", () => {
      useTimerStore.setState({ remainingSeconds: 5 });

      render(<FullscreenBreak />);

      expect(getBreakTimerDisplay("00:05")).toBeInTheDocument();
    });
  });

  describe("Skip Button", () => {
    it("should show skip flow modal when clicked", async () => {
      await renderFullscreenBreakWithSkipFlowOpen();

      // Phase 5: Shows 10-step skip flow instead of simple dialog
      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
      expect(
        screen.getByText(/Are you sure you want to skip/iu),
      ).toBeInTheDocument();
    });
  });

  describe("Skip Confirmation Dialog", () => {
    // Phase 5: Skip flow modal with 10 steps
    it("should show skip flow progress indicator", async () => {
      await renderFullscreenBreakWithSkipFlowOpen();

      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
      expect(screen.getByText(/of 10/iu)).toBeInTheDocument();
    });

    it("should have take break button", async () => {
      await renderFullscreenBreakWithSkipFlowOpen();

      expect(screen.getByText(/No, Take Break/iu)).toBeInTheDocument();
    });
  });

  describe("Away During Break", () => {
    function enableAutomaticPostBreakPolicy(): void {
      useSettingsStore.setState({
        postBreakTransitionPolicy: "start_next_work_automatically",
      });
    }

    it("offers the away action only when the next work would start automatically", () => {
      useSettingsStore.setState({
        postBreakTransitionPolicy: "require_manual_continue",
      });
      const { unmount } = render(<FullscreenBreak />);
      expect(
        screen.queryByRole("button", { name: "離席する" }),
      ).not.toBeInTheDocument();
      unmount();

      enableAutomaticPostBreakPolicy();
      render(<FullscreenBreak />);
      expect(
        screen.getByRole("button", { name: "離席する" }),
      ).toBeInTheDocument();
    });

    it("marks the user away without any confirmation step", async () => {
      enableAutomaticPostBreakPolicy();
      const user = userEvent.setup();
      render(<FullscreenBreak />);

      await user.click(screen.getByRole("button", { name: "離席する" }));

      expect(mockInvoke).toHaveBeenCalledWith("set_away_during_break", {
        away: true,
      });
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    });

    it("shows the away notice with a return action while away", async () => {
      enableAutomaticPostBreakPolicy();
      useTimerStore.setState({ isAwayDuringBreak: true });
      const user = userEvent.setup();
      render(<FullscreenBreak />);

      expect(screen.getByRole("status")).toHaveTextContent(
        /休憩後は自動で作業を開始しません/u,
      );
      expect(
        screen.queryByRole("button", { name: "離席する" }),
      ).not.toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: "戻りました" }));

      expect(mockInvoke).toHaveBeenCalledWith("set_away_during_break", {
        away: false,
      });
    });

    it("keeps the skip flow reachable while away", async () => {
      enableAutomaticPostBreakPolicy();
      useTimerStore.setState({ isAwayDuringBreak: true });

      await renderFullscreenBreakWithSkipFlowOpen();

      expect(screen.getByText(/Step/iu)).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("should expose the timer as a polite atomic live region", () => {
      render(<FullscreenBreak />);

      const timer = getBreakTimerDisplay("05:00");
      expect(timer).toHaveAttribute("aria-live", "polite");
      expect(timer).toHaveAttribute("aria-atomic", "true");
    });
  });
});
