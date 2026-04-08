import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TypingChallenge from "./TypingChallenge";

const mockRetryStep = vi.fn();

vi.mock("@/store/skipFlowStore", () => ({
  useSkipFlowStore: vi.fn(
    (selector: (state: { retryStep: typeof mockRetryStep }) => unknown) =>
      selector({
        retryStep: mockRetryStep,
      }),
  ),
}));

interface PhraseGeneratorActualModule {
  getConfirmationMessage: (step: number) => string;
  validateTypedPhrase: (input: string, target: string) => boolean;
  getCharacterValidation: (input: string, target: string) => (boolean | null)[];
}

vi.mock("@/utils/phraseGenerator", async (importOriginal) => {
  const actual: PhraseGeneratorActualModule = await importOriginal();

  return {
    ...actual,
    getRandomShortPhrase: vi.fn(() => "TEST PHRASE"),
    getRandomLongPhrase: vi.fn(() => "THIS IS A LONGER TEST PHRASE"),
  };
});

function expectRenderedPhrase(phrase: string): void {
  expect(
    screen.getAllByText((_, element) => element?.textContent === phrase),
  ).not.toHaveLength(0);
}

describe("TypingChallenge", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    mockRetryStep.mockReset();
  });

  afterEach(() => {
    if (vi.isFakeTimers()) {
      act(() => {
        vi.runOnlyPendingTimers();
      });
    }
    vi.useRealTimers();
  });

  it("renders the short phrase and focuses the input on mount", () => {
    render(
      <TypingChallenge
        difficulty="short"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    const input = screen.getByPlaceholderText("Start typing here...");

    expectRenderedPhrase("TEST PHRASE");
    expect(input).toHaveFocus();
    expect(input).toHaveAttribute("autoComplete", "off");
    expect(input).toHaveAttribute("autoCorrect", "off");
    expect(input).toHaveAttribute("spellCheck", "false");
  });

  it("renders the long phrase when the long difficulty is selected", () => {
    render(
      <TypingChallenge
        difficulty="long"
        onComplete={mockOnComplete}
        onCancel={mockOnCancel}
      />,
    );

    expectRenderedPhrase("THIS IS A LONGER TEST PHRASE");
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

    const input = screen.getByPlaceholderText("Start typing here...");
    fireEvent.change(input, { target: { value: "TEST X" } });

    expect(mockRetryStep).toHaveBeenCalledTimes(1);
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

    fireEvent.change(screen.getByPlaceholderText("Start typing here..."), {
      target: { value: "TEST PHRASE" },
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
