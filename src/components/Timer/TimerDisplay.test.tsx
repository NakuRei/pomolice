import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { TimerDisplay } from "./TimerDisplay";
import { useTimerStore, TimerStatus, SessionType } from "@/store/timerStore";

function getTimerDisplay(timeText: string): HTMLElement {
  return screen.getByText(timeText);
}

describe("TimerDisplay", () => {
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

  it("should render the initial work session timer state", () => {
    render(<TimerDisplay />);

    expect(getTimerDisplay("25:00")).toBeInTheDocument();
    expect(screen.getByText("FOCUS")).toBeInTheDocument();
  });

  it.each([
    { sessionType: SessionType.WORK, expectedText: "FOCUS" },
    { sessionType: SessionType.SHORT_BREAK, expectedText: "BREAK" },
    { sessionType: SessionType.LONG_BREAK, expectedText: "LONG" },
  ])(
    "should display the phase label $expectedText",
    ({ sessionType, expectedText }) => {
      useTimerStore.setState({ sessionType });
      render(<TimerDisplay />);

      expect(screen.getByText(expectedText)).toBeInTheDocument();
    },
  );

  it.each([
    { sessionType: SessionType.WORK, expectedName: "作業時間" },
    { sessionType: SessionType.SHORT_BREAK, expectedName: "短い休憩" },
    { sessionType: SessionType.LONG_BREAK, expectedName: "長い休憩" },
  ])(
    "should name the phase $expectedName for assistive technology",
    ({ sessionType, expectedName }) => {
      useTimerStore.setState({ sessionType });
      render(<TimerDisplay />);

      expect(screen.getByLabelText(expectedName)).toBeInTheDocument();
    },
  );

  it("should update display when remaining seconds change", () => {
    const { rerender } = render(<TimerDisplay />);

    // Initial state
    expect(getTimerDisplay("25:00")).toBeInTheDocument();

    // Update remaining seconds
    act(() => {
      useTimerStore.setState({ remainingSeconds: 1499 }); // 24:59
    });
    rerender(<TimerDisplay />);
    expect(getTimerDisplay("24:59")).toBeInTheDocument();

    // Another update
    act(() => {
      useTimerStore.setState({ remainingSeconds: 60 }); // 01:00
    });
    rerender(<TimerDisplay />);
    expect(getTimerDisplay("01:00")).toBeInTheDocument();
  });

  it("should display 00:00 when time reaches zero", () => {
    useTimerStore.setState({ remainingSeconds: 0 });
    render(<TimerDisplay />);
    expect(getTimerDisplay("00:00")).toBeInTheDocument();
  });

  it("should expose timer updates as a polite atomic live region", () => {
    render(<TimerDisplay />);

    const timerElement = getTimerDisplay("25:00");
    expect(timerElement).toHaveAttribute("aria-live", "polite");
    expect(timerElement).toHaveAttribute("aria-atomic", "true");
  });

  it("should keep the phase label independent of whether the timer runs", () => {
    useTimerStore.setState({ status: TimerStatus.WORKING });
    const { rerender } = render(<TimerDisplay />);
    expect(screen.getByText("FOCUS")).toBeInTheDocument();

    act(() => {
      useTimerStore.setState({ status: TimerStatus.IDLE });
    });
    rerender(<TimerDisplay />);
    expect(screen.getByText("FOCUS")).toBeInTheDocument();
  });
});
