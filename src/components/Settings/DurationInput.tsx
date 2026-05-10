import { useId, type ChangeEvent } from "react";
import { parseDurationInputText } from "./durationInputValue";
/**
 * DurationInput Component
 * Reusable input component for duration settings with validation
 */

function isInputValid(
  inputValue: string,
  min: number,
  max: number,
  allowDecimal: boolean,
): boolean {
  return parseDurationInputText(inputValue, min, max, allowDecimal) !== null;
}

function createElementTestId(
  testId: string | undefined,
  suffix: "input" | "error",
): string | undefined {
  return testId !== undefined ? `${testId}-${suffix}` : undefined;
}

function getStepValue(allowDecimal: boolean): "0.01" | "1" {
  return allowDecimal ? "0.01" : "1";
}

function getInputStateClassName(isValid: boolean): string {
  return isValid
    ? "border-border focus:ring-primary-600"
    : "border-danger-500 focus:ring-danger-500";
}

interface DurationInputProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  min: number;
  max: number;
  unit?: string;
  testId?: string;
  disabled?: boolean;
  allowDecimal?: boolean;
}

export function DurationInput({
  label,
  value,
  onChange,
  min,
  max,
  unit = "分",
  testId,
  disabled = false,
  allowDecimal = true,
}: DurationInputProps): React.ReactElement {
  const generatedElementId = useId();

  const handleChange = (e: ChangeEvent<HTMLInputElement>): void => {
    onChange(e.target.value);
  };

  const isValid = isInputValid(value, min, max, allowDecimal);
  const inputStateClassName = getInputStateClassName(isValid);
  const inputTestId = createElementTestId(testId, "input");
  const errorTestId = createElementTestId(testId, "error");
  const inputId = `${generatedElementId}-input`;
  const rangeHintId = `${generatedElementId}-hint`;
  const errorId = `${generatedElementId}-error`;
  const describedBy = isValid ? rangeHintId : `${rangeHintId} ${errorId}`;

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <label htmlFor={inputId} className="text-sm font-medium text-text">
        {label}
      </label>
      <div className="flex items-center gap-2">
        <input
          id={inputId}
          type="number"
          value={value}
          onChange={handleChange}
          min={min}
          max={max}
          step={getStepValue(allowDecimal)}
          disabled={disabled}
          aria-invalid={isValid ? undefined : true}
          aria-describedby={describedBy}
          className={`flex-1 rounded-lg border bg-surface-raised px-3 py-2 text-on-surface focus:ring-2 focus:outline-none disabled:cursor-not-allowed disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text ${inputStateClassName}`}
          data-testid={inputTestId}
        />
        <span className="text-sm text-text-muted">{unit}</span>
      </div>
      {!isValid && (
        <p
          id={errorId}
          className="text-xs text-danger-300"
          data-testid={errorTestId}
        >
          {min}〜{max}
          {unit}の範囲で入力してください
        </p>
      )}
      <p id={rangeHintId} className="text-xs text-text-muted">
        範囲: {min}〜{max}
        {unit}
      </p>
    </div>
  );
}
