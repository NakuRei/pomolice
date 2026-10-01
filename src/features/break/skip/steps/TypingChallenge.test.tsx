import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TypingChallenge from "./TypingChallenge";
import { useSkipFlowStore } from "@/features/break/skipFlowStore";
import {
  SHORT_TYPING_PHRASES,
  LONG_TYPING_PHRASES,
} from "@/features/break/skip/typingPhrases";

/**
 * Reads the phrase currently rendered on screen by matching against the
 * difficulty's phrase bank. Returning the matched bank entry doubles as an
 * assertion that the displayed phrase belongs to that bank, without coupling
 * to how the phrase is split into character spans.
 */
function renderedPhraseFrom(bank: readonly string[]): string {
  const phrase = bank.find(
    (candidate) =>
      screen.queryAllByText((_, element) => element?.textContent === candidate)
        .length > 0,
  );

  if (phrase === undefined) {
    throw new Error("rendered phrase was not found in the expected bank");
  }

  return phrase;
}

function firstCharThatDiffers(phrase: string): string {
  return phrase.startsWith("Z") ? "Y" : "Z";
}

describe("TypingChallenge", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    useSkipFlowStore.getState().resetSkipFlow();
  });

  afterEach(() => {
    if (vi.isFakeTimers()) {
      act(() => {
        vi.runOnlyPendingTimers();
      });
    }
    vi.useRealTimers();
  });

  it("renders a short-bank phrase and focuses the input on mount", () => {
    render(
      <TypingChallenge
        difficulty="short"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    const input = screen.getByPlaceholderText("Start typing here...");

    renderedPhraseFrom(SHORT_TYPING_PHRASES);
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute("autoComplete", "off");
    expect(input).toHaveAttribute("autoCorrect", "off");
    expect(input).toHaveAttribute("spellCheck", "false");
  });

  it("renders a long-bank phrase when the long difficulty is selected", () => {
    render(
      <TypingChallenge
        difficulty="long"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    renderedPhraseFrom(LONG_TYPING_PHRASES);
  });

  it("retries after a typo and clears the input after the reset timeout", () => {
    // Fake timers are opt-in per test because userEvent clicks deadlock here under Vitest, even with advanceTimers.
    vi.useFakeTimers();

    render(
      <TypingChallenge
        difficulty="short"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    const phrase = renderedPhraseFrom(SHORT_TYPING_PHRASES);
    const input = screen.getByPlaceholderText("Start typing here...");
    fireEvent.change(input, {
      target: { value: firstCharThatDiffers(phrase) },
    });

    expect(screen.getByText(/Typo detected/iu)).toBeInTheDocument();
    expect(input).toBeDisabled();

    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(input).toHaveValue("");
    expect(input).not.toBeDisabled();
    expect(input).toHaveFocus();
  });

  it("calls onComplete when the full phrase matches exactly", () => {
    render(
      <TypingChallenge
        difficulty="short"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    const phrase = renderedPhraseFrom(SHORT_TYPING_PHRASES);
    fireEvent.change(screen.getByPlaceholderText("Start typing here..."), {
      target: { value: phrase },
    });

    expect(mockOnComplete).toHaveBeenCalledTimes(1);
  });

  it("calls onCancel when the cancel button is clicked", async () => {
    const user = userEvent.setup();

    render(
      <TypingChallenge
        difficulty="short"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    await user.click(
      screen.getByRole("button", { name: /Cancel and Take Break/iu }),
    );

    expect(mockOnCancel).toHaveBeenCalledTimes(1);
  });
});
