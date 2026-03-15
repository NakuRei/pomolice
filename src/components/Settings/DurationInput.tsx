import { ChangeEvent, useEffect, useState } from "react";
/**
 * DurationInput Component
 * Reusable input component for duration settings with validation
 */

interface DurationInputProps {
  label: string;
  value: number | null;
  onChange: (value: number | null) => void;
  min: number;
  max: number;
  unit?: string;
  testId?: string;
  disabled?: boolean;
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
}: DurationInputProps) {
  const [inputValue, setInputValue] = useState(
    value !== null && Number.isFinite(value) ? String(value) : "",
  );

  useEffect(() => {
    if (value !== null && Number.isFinite(value)) {
      setInputValue(String(value));
    }
  }, [value]);

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    const { value: raw } = e.target;
    setInputValue(raw);

    if (raw.trim() === "") {
      onChange(null);
      return;
    }

    const parsedValue = Number(raw);

    if (!Number.isFinite(parsedValue)) {
      onChange(null);
      return;
    }

    if (parsedValue < min || parsedValue > max) {
      onChange(null);
      return;
    }

    onChange(parsedValue);
  };

  const parsedInput = Number(inputValue);
  const isValid =
    inputValue.trim() !== "" &&
    Number.isFinite(parsedInput) &&
    parsedInput >= min &&
    parsedInput <= max;

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <label className="text-sm font-medium text-text">{label}</label>
      <div className="flex items-center gap-2">
        <input
          type="number"
          value={inputValue}
          onChange={handleChange}
          min={min}
          max={max}
          step="0.01"
          disabled={disabled}
          className={`flex-1 rounded-lg border bg-surface-raised px-3 py-2 text-on-surface focus:outline-none focus:ring-2 disabled:cursor-not-allowed disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text ${
            isValid
              ? "border-border focus:ring-primary-600"
              : "border-danger-500 focus:ring-danger-500"
          }`}
          data-testid={`${testId}-input`}
        />
        <span className="text-sm text-text-muted">{unit}</span>
      </div>
      {!isValid && (
        <p className="text-xs text-danger-300" data-testid={`${testId}-error`}>
          {min}〜{max}
          {unit}の範囲で入力してください
        </p>
      )}
      <p className="text-xs text-text-muted">
        範囲: {min}〜{max}
        {unit}
      </p>
    </div>
  );
}
