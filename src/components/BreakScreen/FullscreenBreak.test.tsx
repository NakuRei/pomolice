import { useTimerObservationStore } from "@/store/timerObservationStore";
import { answerSettingsSavesForTest } from "@/test/settingsSaveFixture";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
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
import { useTimerOperationStore } from "@/store/timerOperationStore";

import { useSkipFlowStore } from "@/store/skipFlowStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { createSettingsForTest } from "@/test/settingsFixture";
import type { Settings } from "@/types/settings";
import {
  LONG_TYPING_PHRASES,
  SHORT_TYPING_PHRASES,
} from "@/data/typingPhrases";

function setSettings(overrides: Partial<Settings> = {}): void {
  useTimerObservationStore.setState({
    settings: createSettingsForTest(overrides),
  });
}

function getBreakTimerDisplay(timeText: string): HTMLElement {
  return screen.getByText(timeText);
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
    mockInvoke.mockResolvedValue({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    });
    resetTimerStoreForTest();
    useSkipFlowStore.getState().resetSkipFlow();
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
    setSettings();
    useTimerObservationStore.setState({
      sessionType: "short_break",
      remainingSeconds: 300,
      sessionDurationSeconds: 300,
      currentCycle: 0,
      isAwayDuringBreak: false,
      status: "on_break",
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe("Rendering", () => {
    it("keeps release guidance and the skip flow available before timer details arrive", async () => {
      resetTimerStoreForTest();
      const user = userEvent.setup();
      render(<FullscreenBreak />);

      expect(screen.getByText(/緊急解除/u)).toHaveTextContent(
        "Ctrl + Alt + Shift + E",
      );
      expect(screen.getByTestId("timer-display")).toHaveTextContent("--:--");
      expect(screen.queryByText("BREAK")).not.toBeInTheDocument();
      expect(screen.queryByTestId("cycle-progress")).not.toBeInTheDocument();
      await user.click(screen.getByRole("button", { name: "休憩をスキップ" }));
      expect(
        screen.getByText(/Are you sure you want to skip/iu),
      ).toBeInTheDocument();
    });

    it.each(["short_break", "long_break"] as const)(
      "shows emergency release instructions during %s without opening the skip flow",
      (sessionType) => {
        useTimerObservationStore.setState({ sessionType });
        render(<FullscreenBreak />);
        const guidance = screen.getByText(/緊急解除/u);
        expect(guidance).toHaveTextContent("Ctrl + Alt + Shift + E");
      },
    );

    it("should render the initial short-break ui", () => {
      useTimerObservationStore.setState({ currentCycle: 2 });

      render(<FullscreenBreak />);

      expect(getBreakTimerDisplay("05:00")).toBeInTheDocument();
      expect(screen.getByText("BREAK")).toBeInTheDocument();
      expect(screen.getByLabelText("短い休憩")).toBeInTheDocument();
      expect(
        screen.getByRole("img", { name: "ポモドーロ 2 / 4" }),
      ).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "休憩をスキップ" }),
      ).toBeInTheDocument();
    });

    it("should display 'long break' for long break", () => {
      useTimerObservationStore.setState({ sessionType: "long_break" });

      render(<FullscreenBreak />);

      expect(screen.getByText("LONG BREAK")).toBeInTheDocument();
      expect(screen.getByLabelText("長い休憩")).toBeInTheDocument();
    });

    it("saves the notification sound mute from the break screen", async () => {
      const user = userEvent.setup();
      mockInvoke.mockImplementation(answerSettingsSavesForTest);
      render(<FullscreenBreak />);

      const muteToggle = screen.getByRole("button", { name: "通知音" });
      expect(muteToggle).toHaveAttribute("aria-pressed", "false");

      await user.click(muteToggle);

      expect(mockInvoke).toHaveBeenCalledWith("update_settings", {
        changes: { notification_sound_muted: true },
      });
      expect(muteToggle).toHaveAttribute("aria-pressed", "true");
    });

    it("hides the mute toggle until the settings have arrived", () => {
      useTimerObservationStore.setState({ settings: null });

      render(<FullscreenBreak />);

      expect(
        screen.queryByRole("button", { name: "通知音" }),
      ).not.toBeInTheDocument();
    });

    it("shows why a settings save from the break screen failed", async () => {
      const user = userEvent.setup();
      mockInvoke.mockResolvedValue({
        status: "rejected",
        error: {
          code: "persistence_error",
          message: "Persistence error",
        },
      });
      render(<FullscreenBreak />);

      await user.click(screen.getByRole("button", { name: "通知音" }));

      expect(
        await screen.findByText("データの保存に失敗しました"),
      ).toBeInTheDocument();
    });

    it("does not rewrite the reported timer phase when displaying break controls", () => {
      useTimerObservationStore.getState().observeTimerTick({
        status: "idle",
        remaining_seconds: 300,
        session_duration_secs: 300,
        session_type: "short_break",
        current_cycle: 1,
        completed_pomodoros: 0,
        break_interaction_count: 0,
        away_during_break: false,
      });

      render(<FullscreenBreak />);

      expect(useTimerObservationStore.getState().status).toBe("idle");
      expect(
        screen.getByRole("button", { name: "休憩をスキップ" }),
      ).toBeInTheDocument();
    });

    it("ends the break with an emergency stop once the whole skip flow is done", () => {
      vi.useFakeTimers();
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
        useTimerObservationStore.setState({ remainingSeconds: 60 }); // 1:00
      });
      rerender(<FullscreenBreak />);

      expect(screen.getByText("01:00")).toBeInTheDocument();
    });

    it("should handle single digit seconds", () => {
      useTimerObservationStore.setState({ remainingSeconds: 5 });

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
      setSettings({
        postBreakTransitionPolicy: "start_next_work_automatically",
      });
    }

    function receiveBreakTick(awayDuringBreak: boolean): void {
      useTimerObservationStore.getState().observeTimerTick({
        status: "on_break",
        remaining_seconds: 300,
        session_duration_secs: 300,
        session_type: "short_break",
        current_cycle: 1,
        completed_pomodoros: 1,
        break_interaction_count: 0,
        away_during_break: awayDuringBreak,
      });
    }

    it("offers the away action only when the next work would start automatically", () => {
      setSettings({
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
      receiveBreakTick(true);
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

      act(() => {
        receiveBreakTick(false);
      });

      expect(screen.queryByRole("status")).not.toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "離席する" }),
      ).toBeInTheDocument();
    });

    it("keeps the skip flow reachable while away", async () => {
      enableAutomaticPostBreakPolicy();
      receiveBreakTick(true);

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
