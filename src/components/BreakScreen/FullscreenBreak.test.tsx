import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { render, screen, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { FullscreenBreak } from "./FullscreenBreak";
import { useTimerStore, SessionType } from "@/store/timerStore";
import { resetTimerStoreForTest } from "@/test/resetTimerStore";
import { motivationalMessages } from "@/data/motivationalMessages";

function getBreakTimerDisplay(timeText: string): HTMLElement {
  return screen.getByText(timeText);
}

function getVisibleMotivationalMessages(): string[] {
  return motivationalMessages.filter(
    (message) => screen.queryByText(message) !== null,
  );
}

async function renderFullscreenBreakWithSkipFlowOpen(): Promise<void> {
  const user = userEvent.setup();
  render(<FullscreenBreak />);

  await user.click(screen.getByRole("button", { name: "休憩をスキップ" }));
}

describe("FullscreenBreak", () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    resetTimerStoreForTest();
    useTimerStore.setState({
      sessionType: SessionType.SHORT_BREAK,
      remainingSeconds: 300,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe("Rendering", () => {
    it("should render the initial short-break ui", () => {
      render(<FullscreenBreak />);

      expect(getBreakTimerDisplay("05:00")).toBeInTheDocument();
      expect(screen.getByText("短い休憩")).toBeInTheDocument();
      expect(screen.getByText("今日のポモドーロ:")).toBeInTheDocument();
      expect(
        screen.getByRole("button", { name: "休憩をスキップ" }),
      ).toBeInTheDocument();
      expect(getVisibleMotivationalMessages()).toHaveLength(1);
    });

    it("should display 'long break' for long break", () => {
      useTimerStore.setState({ sessionType: SessionType.LONG_BREAK });

      render(<FullscreenBreak />);

      expect(screen.getByText("長い休憩")).toBeInTheDocument();
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

  describe("Accessibility", () => {
    it("should expose the timer as a polite atomic live region", () => {
      render(<FullscreenBreak />);

      const timer = getBreakTimerDisplay("05:00");
      expect(timer).toHaveAttribute("aria-live", "polite");
      expect(timer).toHaveAttribute("aria-atomic", "true");
    });
  });
});
