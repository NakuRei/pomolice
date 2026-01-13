/**
 * DurationInput Component
 * Reusable input component for duration settings with validation
 */

interface DurationInputProps {
  label: string;
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  unit?: string;
  testId?: string;
}

export function DurationInput({
  label,
  value,
  onChange,
  min,
  max,
  unit = '分',
  testId,
}: DurationInputProps) {
  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newValue = parseInt(e.target.value, 10);
    if (!isNaN(newValue)) {
      onChange(newValue);
    }
  };

  const isValid = value >= min && value <= max;

  return (
    <div className="flex flex-col gap-2" data-testid={testId}>
      <label className="text-sm font-medium text-gray-700">
        {label}
      </label>
      <div className="flex items-center gap-2">
        <input
          type="number"
          value={value}
          onChange={handleChange}
          min={min}
          max={max}
          className={`flex-1 px-3 py-2 border rounded-lg focus:outline-none focus:ring-2 ${
            isValid
              ? 'border-gray-300 focus:ring-blue-500'
              : 'border-red-500 focus:ring-red-500'
          }`}
          data-testid={`${testId}-input`}
        />
        <span className="text-sm text-gray-600">{unit}</span>
      </div>
      {!isValid && (
        <p className="text-xs text-red-600" data-testid={`${testId}-error`}>
          {min}〜{max}{unit}の範囲で入力してください
        </p>
      )}
      <p className="text-xs text-gray-500">
        範囲: {min}〜{max}{unit}
      </p>
    </div>
  );
}
