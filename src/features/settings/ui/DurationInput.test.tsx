import type { ComponentProps } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { DurationInput } from "./DurationInput";
import {
  formatDurationInputText,
  parseDurationInputText,
} from "./durationInputValue";

const defaultProps = {
  label: "作業時間",
  value: "25",
  onChange: vi.fn(),
  testId: "work-duration",
};

function renderDurationInput(
  overrides: Partial<ComponentProps<typeof DurationInput>> = {},
): void {
  render(<DurationInput {...defaultProps} {...overrides} />);
}

function getDurationInput(
  accessibleName = defaultProps.label,
): HTMLInputElement {
  return screen.getByRole("spinbutton", {
    name: accessibleName,
  });
}

describe("DurationInput", () => {
  describe("Rendering", () => {
    it("renders a labelled input with its current value and unit", () => {
      renderDurationInput();

      expect(getDurationInput()).toHaveValue(25);
      expect(screen.getByText("分")).toBeInTheDocument();
    });

    it("reflects updates to the controlled value prop", () => {
      const { rerender } = render(<DurationInput {...defaultProps} />);

      expect(getDurationInput()).toHaveValue(25);

      rerender(<DurationInput {...defaultProps} value="40" />);

      expect(getDurationInput()).toHaveValue(40);
    });

    it("renders a custom unit", () => {
      renderDurationInput({ unit: "回" });

      expect(screen.getByText("回")).toBeInTheDocument();
    });
  });

  describe("Value Changes", () => {
    it.each(["30", "25.5", "999", ""])(
      "emits the raw input text %s",
      (rawValue) => {
        const onChange = vi.fn();
        renderDurationInput({ onChange });

        fireEvent.change(getDurationInput(), {
          target: { value: rawValue },
        });

        expect(onChange).toHaveBeenCalledWith(rawValue);
      },
    );

    it("leaves decimal input visible when configured for whole numbers only", () => {
      renderDurationInput({
        value: "2.5",
        allowDecimal: false,
        testId: "interval",
      });

      expect(getDurationInput().value).toBe("2.5");
    });
  });

  describe("Parsing", () => {
    it.each([
      { rawValue: "30", expectedValue: 30 },
      { rawValue: "25.5", expectedValue: 25.5 },
      { rawValue: "999", expectedValue: 999 },
    ])("parses numeric input $rawValue", ({ rawValue, expectedValue }) => {
      expect(parseDurationInputText(rawValue, true)).toBe(expectedValue);
    });

    it.each(["", "abc", "Infinity", "1e309"])(
      "returns null for non-numeric input %s",
      (rawValue) => {
        expect(parseDurationInputText(rawValue, true)).toBeNull();
      },
    );

    it("rejects decimal values when whole numbers are required", () => {
      expect(parseDurationInputText("2.5", false)).toBeNull();
    });
  });

  describe("Formatting", () => {
    it.each([
      { value: 25, expectedText: "25" },
      { value: null, expectedText: "" },
      { value: Number.POSITIVE_INFINITY, expectedText: "" },
    ])("formats $value as '$expectedText'", ({ value, expectedText }) => {
      expect(formatDurationInputText(value)).toBe(expectedText);
    });
  });

  describe("Validation", () => {
    it("asks for a number when the input is empty", () => {
      renderDurationInput({ value: "" });

      expect(screen.getByText("数値を入力してください。")).toBeInTheDocument();
    });

    it("asks for a whole number when decimals are not allowed", () => {
      renderDurationInput({ value: "2.5", allowDecimal: false });

      expect(screen.getByText("整数を入力してください。")).toBeInTheDocument();
    });

    it("shows the backend's reason and marks the input invalid", () => {
      renderDurationInput({
        value: "999",
        errorMessage: "1秒〜120分の範囲で入力してください。",
      });

      const input = getDurationInput();
      expect(input).toHaveAttribute("aria-invalid", "true");
      expect(input).toHaveAccessibleDescription(
        "1秒〜120分の範囲で入力してください。",
      );
    });

    it("shows no error for a number the backend has not rejected", () => {
      renderDurationInput({ value: "999" });

      const input = getDurationInput();
      expect(input).not.toHaveAttribute("aria-invalid", "true");
      expect(input).not.toHaveAccessibleDescription();
    });
  });

  describe("Input Attributes", () => {
    it("uses a decimal step by default", () => {
      renderDurationInput();

      const input = getDurationInput();
      expect(input).toHaveAttribute("type", "number");
      expect(input).toHaveAttribute("step", "0.01");
    });

    it("uses an integer step size when decimals are disabled", () => {
      renderDurationInput({ allowDecimal: false, testId: "interval" });

      expect(getDurationInput()).toHaveAttribute("step", "1");
    });
  });
});
