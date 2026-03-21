import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { DurationInput } from "./DurationInput";

function getDurationInputByTestId(testId: string): HTMLInputElement {
  return screen.getByTestId(testId);
}

describe("DurationInput", () => {
  const defaultProps = {
    label: "作業時間",
    value: 25,
    onChange: vi.fn(),
    min: 1,
    max: 120,
    testId: "work-duration",
  };

  describe("Rendering", () => {
    it("should render with label", () => {
      render(<DurationInput {...defaultProps} />);
      expect(screen.getByText("作業時間")).toBeInTheDocument();
    });

    it("should render with current value", () => {
      render(<DurationInput {...defaultProps} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveValue(25);
    });

    it("should render with default unit (分)", () => {
      render(<DurationInput {...defaultProps} />);
      expect(screen.getByText("分")).toBeInTheDocument();
    });

    it("should render with custom unit", () => {
      render(<DurationInput {...defaultProps} unit="回" />);
      expect(screen.getByText("回")).toBeInTheDocument();
    });

    it("should display min-max range hint", () => {
      render(<DurationInput {...defaultProps} />);
      expect(screen.getByText("範囲: 1〜120分")).toBeInTheDocument();
    });
  });

  describe("Value Changes", () => {
    it("should call onChange when value is changed", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      // Simulate user changing the value
      fireEvent.change(input, { target: { value: "30" } });

      expect(onChange).toHaveBeenCalledWith(30);
    });

    it("should handle multiple digit input", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      // Simulate user changing the value
      fireEvent.change(input, { target: { value: "100" } });

      expect(onChange).toHaveBeenCalledWith(100);
    });

    it("should accept decimal values by default", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      fireEvent.change(input, { target: { value: "25.5" } });

      expect(onChange).toHaveBeenCalledWith(25.5);
    });

    it("should emit null with non-numeric input", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      // Simulate user entering non-numeric value
      fireEvent.change(input, { target: { value: "abc" } });

      expect(onChange).toHaveBeenCalledWith(null);
    });

    it("should emit null with Infinity", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      fireEvent.change(input, { target: { value: "Infinity" } });

      expect(onChange).toHaveBeenCalledWith(null);
    });

    it("should emit null with overflow values", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      fireEvent.change(input, { target: { value: "1e309" } });

      expect(onChange).toHaveBeenCalledWith(null);
    });

    it("should emit null when clearing input", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = getDurationInputByTestId("work-duration-input");

      fireEvent.change(input, { target: { value: "" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("");
    });

    it("should emit null when value is out of range", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} max={60} />);

      const input = getDurationInputByTestId("work-duration-input");

      fireEvent.change(input, { target: { value: "999" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("999");
    });

    it("should reject decimal values when configured for whole numbers only", () => {
      const onChange = vi.fn();
      render(
        <DurationInput
          {...defaultProps}
          onChange={onChange}
          allowDecimal={false}
          testId="interval"
        />,
      );

      const input = getDurationInputByTestId("interval-input");

      fireEvent.change(input, { target: { value: "2.5" } });

      expect(onChange).toHaveBeenCalledWith(null);
      expect(input.value).toBe("2.5");
    });
  });

  describe("Validation", () => {
    it("should show valid state when value is within range", () => {
      render(<DurationInput {...defaultProps} value={50} />);
      expect(
        screen.queryByTestId("work-duration-error"),
      ).not.toBeInTheDocument();
    });

    it("should show error when value is below minimum", () => {
      render(<DurationInput {...defaultProps} value={0} />);
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
      expect(
        screen.getByText("1〜120分の範囲で入力してください"),
      ).toBeInTheDocument();
    });

    it("should show error when value is above maximum", () => {
      render(<DurationInput {...defaultProps} value={150} />);
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
    });

    it("should validate at minimum boundary", () => {
      render(<DurationInput {...defaultProps} value={1} />);
      expect(
        screen.queryByTestId("work-duration-error"),
      ).not.toBeInTheDocument();
    });

    it("should validate at maximum boundary", () => {
      render(<DurationInput {...defaultProps} value={120} />);
      expect(
        screen.queryByTestId("work-duration-error"),
      ).not.toBeInTheDocument();
    });
  });

  describe("Input Attributes", () => {
    it("should have correct type attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveAttribute("type", "number");
    });

    it("should have min attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveAttribute("min", "1");
    });

    it("should have max attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveAttribute("max", "120");
    });

    it("should default to a decimal step size", () => {
      render(<DurationInput {...defaultProps} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveAttribute("step", "0.01");
    });

    it("should use an integer step size when decimals are disabled", () => {
      render(
        <DurationInput
          {...defaultProps}
          allowDecimal={false}
          testId="interval"
        />,
      );
      const input = getDurationInputByTestId("interval-input");
      expect(input).toHaveAttribute("step", "1");
    });
  });

  describe("Edge Cases", () => {
    it("should handle zero value", () => {
      render(<DurationInput {...defaultProps} value={0} min={0} max={120} />);
      const input = getDurationInputByTestId("work-duration-input");
      expect(input).toHaveValue(0);
    });

    it("should handle negative value with validation", () => {
      render(<DurationInput {...defaultProps} value={-5} />);
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
    });

    it("should handle very large numbers", () => {
      render(<DurationInput {...defaultProps} value={9999} />);
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
    });
  });

  describe("Different Validation Ranges", () => {
    it("should work with small range", () => {
      render(
        <DurationInput
          {...defaultProps}
          value={3}
          min={2}
          max={10}
          testId="interval"
        />,
      );
      expect(screen.getByText("範囲: 2〜10分")).toBeInTheDocument();
    });

    it("should validate against custom range", () => {
      render(
        <DurationInput
          {...defaultProps}
          value={1}
          min={5}
          max={60}
          testId="break"
        />,
      );
      expect(screen.getByTestId("break-error")).toBeInTheDocument();
      expect(
        screen.getByText("5〜60分の範囲で入力してください"),
      ).toBeInTheDocument();
    });
  });
});
