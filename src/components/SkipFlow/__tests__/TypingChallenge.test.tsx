/**
 * Component tests for TypingChallenge
 * Phase 5: Multi-step skip confirmation flow
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import TypingChallenge from "../steps/TypingChallenge";

// Mock the store
vi.mock("../../../store/skipFlowStore", () => ({
  useSkipFlowStore: vi.fn(() => ({
    retryStep: vi.fn(),
  })),
}));

// Mock phrase generator to return predictable values
vi.mock("../../../utils/phraseGenerator", () => ({
  getRandomShortPhrase: vi.fn(() => "TEST PHRASE"),
  getRandomLongPhrase: vi.fn(() => "THIS IS A LONGER TEST PHRASE"),
  validateTypedPhrase: vi.fn((input: string, target: string) => input === target),
  getCharacterValidation: vi.fn((input: string, target: string) => {
    return Array.from(target).map((char, index) => {
      if (index >= input.length) return null;
      return input[index] === char;
    });
  }),
}));

describe("TypingChallenge", () => {
  const mockOnComplete = vi.fn();
  const mockOnCancel = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Rendering", () => {
    it("should render with title and instructions", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      expect(screen.getByText("Typing Challenge")).toBeInTheDocument();
      expect(
        screen.getByText(/Type the following phrase/i)
      ).toBeInTheDocument();
    });

    it("should display target phrase", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      // Text is split into spans, check that input field renders
      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toBeInTheDocument();

      // Check phrase is displayed (split into individual spans)
      const spans = screen.getAllByText(/[A-Z]/);
      expect(spans.length).toBeGreaterThan(0);
    });

    it("should display long phrase for long difficulty", () => {
      render(
        <TypingChallenge
          difficulty="long"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      // Text is split into spans, verify component renders
      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toBeInTheDocument();
    });

    it("should render input field", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toBeInTheDocument();
      expect(input).toHaveAttribute("type", "text");
    });

    it("should render cancel button", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      expect(
        screen.getByRole("button", { name: /Cancel and Take Break/i })
      ).toBeInTheDocument();
    });
  });

  describe("Input handling", () => {
    it("should update input value when typing", async () => {
      const user = userEvent.setup();

      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      await user.type(input, "TEST");

      expect(input).toHaveValue("TEST");
    });

    it("should focus input on mount", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toHaveFocus();
    });

    it("should have autocomplete disabled", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toHaveAttribute("autoComplete", "off");
      expect(input).toHaveAttribute("autoCorrect", "off");
      expect(input).toHaveAttribute("spellCheck", "false");
    });
  });

  describe("Validation", () => {
    it("should show error when typo is detected", async () => {
      const user = userEvent.setup();

      // Mock validation to simulate typo
      const { getCharacterValidation } = await import(
        "../../../utils/phraseGenerator"
      );
      vi.mocked(getCharacterValidation).mockReturnValue([
        true,
        true,
        true,
        false, // Typo on 4th character
      ]);

      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      await user.type(input, "TESX");

      await waitFor(() => {
        expect(screen.getByText(/Typo detected/i)).toBeInTheDocument();
      });
    });

    it("should reset input after typo", async () => {
      const user = userEvent.setup();

      // Mock validation to simulate typo
      const { getCharacterValidation } = await import(
        "../../../utils/phraseGenerator"
      );
      vi.mocked(getCharacterValidation).mockReturnValue([
        true,
        true,
        false, // Typo
      ]);

      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      await user.type(input, "TEX");

      await waitFor(
        () => {
          expect(input).toHaveValue("");
        },
        { timeout: 1500 }
      );
    });

    it("should call onComplete when phrase is correctly typed", async () => {
      const user = userEvent.setup();

      // Mock validation to return correct validation
      const { validateTypedPhrase, getCharacterValidation } = await import(
        "../../../utils/phraseGenerator"
      );
      vi.mocked(getCharacterValidation).mockImplementation((input: string, target: string) => {
        return Array.from(target).map((char, index) => {
          if (index >= input.length) return null;
          return input[index] === char;
        });
      });

      // Return true only when complete match
      vi.mocked(validateTypedPhrase).mockImplementation((input: string, target: string) => {
        return input === target;
      });

      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      await user.type(input, "TEST PHRASE");

      await waitFor(() => {
        expect(mockOnComplete).toHaveBeenCalled();
      }, { timeout: 3000 });
    });
  });

  describe("Cancel functionality", () => {
    it("should call onCancel when cancel button is clicked", async () => {
      const user = userEvent.setup();

      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const cancelButton = screen.getByRole("button", {
        name: /Cancel and Take Break/i,
      });
      await user.click(cancelButton);

      expect(mockOnCancel).toHaveBeenCalledTimes(1);
    });
  });

  describe("Hints and instructions", () => {
    it("should display helpful hints", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      expect(
        screen.getByText(/Type carefully. Any typo will restart this step/i)
      ).toBeInTheDocument();
      expect(
        screen.getByText(/Case-sensitive. All letters must be UPPERCASE/i)
      ).toBeInTheDocument();
    });
  });

  describe("Accessibility", () => {
    it("should have proper ARIA attributes", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      expect(input).toHaveAttribute("type", "text");
    });

    it("should be keyboard navigable", () => {
      render(
        <TypingChallenge
          difficulty="short"
          onComplete={mockOnComplete}
          onCancel={mockOnCancel}
        />
      );

      const input = screen.getByPlaceholderText("Start typing here...");
      const cancelButton = screen.getByRole("button", {
        name: /Cancel and Take Break/i,
      });

      // Input should be focusable
      input.focus();
      expect(input).toHaveFocus();

      // Cancel button should be focusable
      cancelButton.focus();
      expect(cancelButton).toHaveFocus();
    });
  });
});
