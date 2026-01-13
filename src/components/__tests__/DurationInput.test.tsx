import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { DurationInput } from "../Settings/DurationInput";

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
      const input = screen.getByTestId("work-duration-input");
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

      const input = screen.getByTestId("work-duration-input") as HTMLInputElement;

      // Simulate user changing the value
      fireEvent.change(input, { target: { value: "30" } });

      expect(onChange).toHaveBeenCalledWith(30);
    });

    it("should handle multiple digit input", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = screen.getByTestId("work-duration-input") as HTMLInputElement;

      // Simulate user changing the value
      fireEvent.change(input, { target: { value: "100" } });

      expect(onChange).toHaveBeenCalledWith(100);
    });

    it("should not call onChange with invalid input", () => {
      const onChange = vi.fn();
      render(<DurationInput {...defaultProps} onChange={onChange} />);

      const input = screen.getByTestId("work-duration-input") as HTMLInputElement;

      // Simulate user entering non-numeric value
      fireEvent.change(input, { target: { value: "abc" } });

      // onChange should not be called with NaN
      expect(onChange).not.toHaveBeenCalled();
    });
  });

  describe("Validation", () => {
    it("should show valid state when value is within range", () => {
      render(<DurationInput {...defaultProps} value={50} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-gray-300");
      expect(screen.queryByTestId("work-duration-error")).not.toBeInTheDocument();
    });

    it("should show error when value is below minimum", () => {
      render(<DurationInput {...defaultProps} value={0} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-red-500");
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
      expect(screen.getByText("1〜120分の範囲で入力してください")).toBeInTheDocument();
    });

    it("should show error when value is above maximum", () => {
      render(<DurationInput {...defaultProps} value={150} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-red-500");
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
    });

    it("should validate at minimum boundary", () => {
      render(<DurationInput {...defaultProps} value={1} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-gray-300");
      expect(screen.queryByTestId("work-duration-error")).not.toBeInTheDocument();
    });

    it("should validate at maximum boundary", () => {
      render(<DurationInput {...defaultProps} value={120} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-gray-300");
      expect(screen.queryByTestId("work-duration-error")).not.toBeInTheDocument();
    });
  });

  describe("Input Attributes", () => {
    it("should have correct type attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = screen.getByTestId("work-duration-input");
      expect(input).toHaveAttribute("type", "number");
    });

    it("should have min attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = screen.getByTestId("work-duration-input");
      expect(input).toHaveAttribute("min", "1");
    });

    it("should have max attribute", () => {
      render(<DurationInput {...defaultProps} />);
      const input = screen.getByTestId("work-duration-input");
      expect(input).toHaveAttribute("max", "120");
    });
  });

  describe("Styling", () => {
    it("should apply valid styling classes", () => {
      render(<DurationInput {...defaultProps} value={50} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-gray-300");
      expect(input).toHaveClass("focus:ring-blue-500");
    });

    it("should apply error styling classes", () => {
      render(<DurationInput {...defaultProps} value={0} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-red-500");
      expect(input).toHaveClass("focus:ring-red-500");
    });
  });

  describe("Edge Cases", () => {
    it("should handle zero value", () => {
      render(<DurationInput {...defaultProps} value={0} min={0} max={120} />);
      const input = screen.getByTestId("work-duration-input");
      expect(input).toHaveValue(0);
    });

    it("should handle negative value with validation", () => {
      render(<DurationInput {...defaultProps} value={-5} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-red-500");
      expect(screen.getByTestId("work-duration-error")).toBeInTheDocument();
    });

    it("should handle very large numbers", () => {
      render(<DurationInput {...defaultProps} value={9999} />);
      const input = screen.getByTestId("work-duration-input");

      expect(input).toHaveClass("border-red-500");
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
        />
      );
      const input = screen.getByTestId("interval-input");

      expect(input).toHaveClass("border-gray-300");
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
        />
      );
      const input = screen.getByTestId("break-input");

      expect(input).toHaveClass("border-red-500");
      expect(screen.getByTestId("break-error")).toBeInTheDocument();
      expect(screen.getByText("5〜60分の範囲で入力してください")).toBeInTheDocument();
    });
  });
});
