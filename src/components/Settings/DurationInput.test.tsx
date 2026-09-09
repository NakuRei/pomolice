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
  min: 1,
  max: 120,
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
    it("renders a labelled input with its current value and range hint", () => {
      renderDurationInput();

      expect(getDurationInput()).toHaveValue(25);
      expect(screen.getByText("分")).toBeInTheDocument();
      expect(screen.getByText("範囲: 1〜120分")).toBeInTheDocument();
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

    it("leaves out-of-range input visible", () => {
      renderDurationInput({ max: 60, value: "999" });

      const input = getDurationInput();

      expect(input.value).toBe("999");
    });

    it("leaves decimal input visible when configured for whole numbers only", () => {
      renderDurationInput({
        value: "2.5",
        allowDecimal: false,
        testId: "interval",
      });

      const input = getDurationInput();

      expect(input.value).toBe("2.5");
    });
  });

  describe("Parsing", () => {
    it.each([
      { rawValue: "30", expectedValue: 30 },
      { rawValue: "25.5", expectedValue: 25.5 },
    ])("parses valid input $rawValue", ({ rawValue, expectedValue }) => {
      expect(parseDurationInputText(rawValue, 1, 120, true)).toBe(
        expectedValue,
      );
    });

    it.each(["", "abc", "Infinity", "1e309", "0", "150"])(
      "returns null for invalid input %s",
      (rawValue) => {
        expect(parseDurationInputText(rawValue, 1, 120, true)).toBeNull();
      },
    );

    it("rejects decimal values when whole numbers are required", () => {
      expect(parseDurationInputText("2.5", 1, 120, false)).toBeNull();
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
    it.each([
      { value: "0", expectedMessage: "1〜120分の範囲で入力してください" },
      { value: "150", expectedMessage: "1〜120分の範囲で入力してください" },
    ])(
      "shows an error when the initial value is outside the range: $value",
      ({ value, expectedMessage }) => {
        renderDurationInput({ value });

        expect(screen.getByText(expectedMessage)).toBeInTheDocument();
      },
    );

    it.each([1, 120])(
      "does not show an error at the configured boundary %s",
      (value) => {
        renderDurationInput({ value: String(value) });

        expect(
          screen.queryByText("1〜120分の範囲で入力してください"),
        ).not.toBeInTheDocument();
      },
    );

    it("supports zero when zero is allowed", () => {
      renderDurationInput({ value: "0", min: 0, max: 120 });

      expect(getDurationInput()).toHaveValue(0);
      expect(
        screen.queryByText("0〜120分の範囲で入力してください"),
      ).not.toBeInTheDocument();
    });

    it("validates against a custom range", () => {
      renderDurationInput({
        value: "1",
        min: 5,
        max: 60,
        testId: "break",
      });

      expect(
        screen.getByText("5〜60分の範囲で入力してください"),
      ).toBeInTheDocument();
    });

    it("associates invalid input with the range hint and error message", () => {
      renderDurationInput({ value: "0" });

      const input = getDurationInput();
      expect(input).toHaveAttribute("aria-invalid", "true");
      expect(input).toHaveAccessibleDescription(
        "範囲: 1〜120分 1〜120分の範囲で入力してください",
      );
    });

    it("associates valid input only with the static range hint", () => {
      renderDurationInput({ value: "25" });

      const input = getDurationInput();
      expect(input).not.toHaveAttribute("aria-invalid", "true");
      expect(input).toHaveAccessibleDescription("範囲: 1〜120分");
      expect(
        screen.queryByText("1〜120分の範囲で入力してください"),
      ).not.toBeInTheDocument();
    });

    it("keeps each input associated with its own hint and error when labels match", () => {
      render(
        <>
          <DurationInput
            label="作業時間"
            value="100"
            onChange={() => undefined}
            min={5}
            max={60}
          />
          <DurationInput
            label="作業時間"
            value="0"
            onChange={() => undefined}
            min={1}
            max={120}
          />
        </>,
      );

      expect(
        screen.getByRole("spinbutton", {
          name: "作業時間",
          description: "範囲: 1〜120分 1〜120分の範囲で入力してください",
        }),
      ).toHaveValue(0);
      expect(
        screen.getByRole("spinbutton", {
          name: "作業時間",
          description: "範囲: 5〜60分 5〜60分の範囲で入力してください",
        }),
      ).toHaveValue(100);
    });
  });

  describe("Input Attributes", () => {
    it("applies numeric input attributes from props", () => {
      renderDurationInput();

      const input = getDurationInput();
      expect(input).toHaveAttribute("type", "number");
      expect(input).toHaveAttribute("min", "1");
      expect(input).toHaveAttribute("max", "120");
      expect(input).toHaveAttribute("step", "0.01");
    });

    it("uses an integer step size when decimals are disabled", () => {
      renderDurationInput({ allowDecimal: false, testId: "interval" });

      expect(getDurationInput()).toHaveAttribute("step", "1");
    });
  });
});
