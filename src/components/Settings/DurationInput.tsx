import { useId, type ChangeEvent } from "react";
import { parseDurationInputText } from "./durationInputValue";
/**
 * DurationInput Component
 * Numeric input for a timer setting. It checks only that a number was typed;
 * range errors come from the backend through `errorMessage`.
 */

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

function getFormErrorMessage(
  value: string,
  allowDecimal: boolean,
): string | undefined {
  if (parseDurationInputText(value, allowDecimal) !== null) {
    return undefined;
  }
  return allowDecimal ? "数値を入力してください。" : "整数を入力してください。";
}

interface DurationInputProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  unit?: string;
  testId?: string;
  disabled?: boolean;
  allowDecimal?: boolean;
  /** Why the backend rejected the last saved value, if it did. */
  errorMessage?: string;
}

export function DurationInput({
  label,
  value,
  onChange,
  unit = "分",
  testId,
  disabled = false,
  allowDecimal = true,
  errorMessage,
}: DurationInputProps): React.ReactElement {
  const generatedElementId = useId();

  const handleChange = (e: ChangeEvent<HTMLInputElement>): void => {
    onChange(e.target.value);
  };

  const shownErrorMessage =
    getFormErrorMessage(value, allowDecimal) ?? errorMessage;
  const isValid = shownErrorMessage === undefined;
  const inputStateClassName = getInputStateClassName(isValid);
  const inputTestId = createElementTestId(testId, "input");
  const errorTestId = createElementTestId(testId, "error");
  const inputId = `${generatedElementId}-input`;
  const errorId = `${generatedElementId}-error`;

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
          step={getStepValue(allowDecimal)}
          disabled={disabled}
          aria-invalid={isValid ? undefined : true}
          aria-describedby={isValid ? undefined : errorId}
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
          {shownErrorMessage}
        </p>
      )}
    </div>
  );
}
