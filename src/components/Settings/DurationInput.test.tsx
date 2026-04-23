import type { ComponentProps } from "react";
import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { DurationInput } from "./DurationInput";
import { getAriaDescribedByIds } from "@/test/ariaDescription";

const defaultProps = {
  label: "作業時間",
  value: 25,
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

function getArrayItem<T>(items: readonly T[], index: number): T {
  const item = items[index];
  if (item === undefined) {
    throw new Error(`Expected item at index ${String(index)}.`);
  }

  return item;
}

function getElementById(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (element === null) {
    throw new Error(`Expected element with id "${id}".`);
  }

  return element;
}

function getDescribedTextContents(element: HTMLElement): string[] {
  return getAriaDescribedByIds(element).map(
    (descriptionId) => getElementById(descriptionId).textContent ?? "",
  );
}

describe("DurationInput", () => {
  describe("Rendering", () => {
    it("renders a labelled input with its current value and range hint", () => {
      renderDurationInput();

      expect(getDurationInput()).toHaveValue(25);
      expect(screen.getByText("分")).toBeInTheDocument();
      expect(screen.getByText("範囲: 1〜120分")).toBeInTheDocument();
    });

    it("renders a custom unit", () => {
      renderDurationInput({ unit: "回" });

      expect(screen.getByText("回")).toBeInTheDocument();
    });
  });

  describe("Value Changes", () => {
    it.each([
      { rawValue: "30", expectedValue: 30 },
      { rawValue: "25.5", expectedValue: 25.5 },
    ])(
      "emits the parsed value for valid input $rawValue",
      ({ rawValue, expectedValue }) => {
        const onChange = vi.fn();
        renderDurationInput({ onChange });

        fireEvent.change(getDurationInput(), {
          target: { value: rawValue },
        });

        expect(onChange).toHaveBeenCalledWith(expectedValue);
      },
    );

    it.each(["abc", "Infinity", "1e309"])(
      "emits null for invalid input %s",
      (rawValue) => {
        const onChange = vi.fn();
        renderDurationInput({ onChange });

        fireEvent.change(getDurationInput(), {
          target: { value: rawValue },
        });

        expect(onChange).toHaveBeenCalledWith(null);
      },
    );

    it("emits null and leaves the field blank when cleared", () => {
      const onChange = vi.fn();
      renderDurationInput({ onChange });

      const input = getDurationInput();
      fireEvent.change(input, { target: { value: "" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("");
    });

    it("emits null when the next value is out of range", () => {
      const onChange = vi.fn();
      renderDurationInput({ onChange, max: 60 });

      const input = getDurationInput();
      fireEvent.change(input, { target: { value: "999" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("999");
    });

    it("rejects decimal values when configured for whole numbers only", () => {
      const onChange = vi.fn();
      renderDurationInput({
        onChange,
        allowDecimal: false,
        testId: "interval",
      });

      const input = getDurationInput();
      fireEvent.change(input, { target: { value: "2.5" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("2.5");
    });
  });

  describe("Validation", () => {
    it.each([
      { value: 0, expectedMessage: "1〜120分の範囲で入力してください" },
      { value: 150, expectedMessage: "1〜120分の範囲で入力してください" },
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
        renderDurationInput({ value });

        expect(
          screen.queryByText("1〜120分の範囲で入力してください"),
        ).not.toBeInTheDocument();
      },
    );

    it("supports zero when zero is allowed", () => {
      renderDurationInput({ value: 0, min: 0, max: 120 });

      expect(getDurationInput()).toHaveValue(0);
      expect(
        screen.queryByText("0〜120分の範囲で入力してください"),
      ).not.toBeInTheDocument();
    });

    it("validates against a custom range", () => {
      renderDurationInput({
        value: 1,
        min: 5,
        max: 60,
        testId: "break",
      });

      expect(
        screen.getByText("5〜60分の範囲で入力してください"),
      ).toBeInTheDocument();
    });

    it("associates invalid input with the range hint and error message", () => {
      renderDurationInput({ value: 0 });

      const input = getDurationInput();
      const describedByIds = getAriaDescribedByIds(input);

      expect(input).toHaveAttribute("aria-invalid", "true");
      expect(describedByIds).toHaveLength(2);
      expect(getDescribedTextContents(input)).toEqual(
        expect.arrayContaining([
          "範囲: 1〜120分",
          "1〜120分の範囲で入力してください",
        ]),
      );
    });

    it("associates valid input only with the static range hint", () => {
      renderDurationInput({ value: 25 });

      const input = getDurationInput();
      const describedByIds = getAriaDescribedByIds(input);

      expect(input).not.toHaveAttribute("aria-invalid", "true");
      expect(describedByIds).toHaveLength(1);
      expect(getDescribedTextContents(input)).toEqual(["範囲: 1〜120分"]);
      expect(
        screen.queryByText("1〜120分の範囲で入力してください"),
      ).not.toBeInTheDocument();
    });

    it("generates distinct description ids for inputs with the same label", () => {
      render(
        <>
          <DurationInput
            label="作業時間"
            value={0}
            onChange={() => undefined}
            min={1}
            max={120}
          />
          <DurationInput
            label="作業時間"
            value={0}
            onChange={() => undefined}
            min={1}
            max={120}
          />
        </>,
      );

      const inputs = screen.getAllByRole("spinbutton", { name: "作業時間" });
      expect(inputs).toHaveLength(2);

      const firstInputDescriptionIds = getAriaDescribedByIds(
        getArrayItem(inputs, 0),
      );
      const secondInputDescriptionIds = getAriaDescribedByIds(
        getArrayItem(inputs, 1),
      );

      expect(firstInputDescriptionIds).toHaveLength(2);
      expect(secondInputDescriptionIds).toHaveLength(2);
      for (const descriptionId of firstInputDescriptionIds) {
        expect(secondInputDescriptionIds).not.toContain(descriptionId);
      }
      for (const descriptionId of [
        ...firstInputDescriptionIds,
        ...secondInputDescriptionIds,
      ]) {
        expect(getElementById(descriptionId)).toBeInTheDocument();
      }
    });
  });

  describe("Input Attributes", () => {
    it("applies numeric input attributes from props", () => {
      renderDurationInput();

      const input = getDurationInput();
      const label = screen.getByText(defaultProps.label);

      expect(input).toHaveAttribute("type", "number");
      expect(label).toHaveAttribute("for", input.id);
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
