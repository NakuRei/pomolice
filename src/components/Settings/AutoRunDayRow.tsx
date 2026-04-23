import { useId } from "react";
import { WEEKDAY_LABELS_BY_KEY } from "@/constants/weekdayLabels";
import {
  type DayAutoRunSlot,
  type DayScheduleValidationError,
  type WeekdayKey,
} from "@/types/weeklyAutoRun";

interface AutoRunDayRowProps {
  weekday: WeekdayKey;
  autoRunFeatureEnabled: boolean;
  daySlot: DayAutoRunSlot;
  dayValidationError?: DayScheduleValidationError;
  onEnabledChange: (enabled: boolean) => void;
  onStartTimeChange: (timeText: string) => void;
  onEndTimeChange: (timeText: string) => void;
}

const TIME_TEXT_PATTERN = /^(?:[01]\d|2[0-3]):[0-5]\d$/u;

function parseTimeToMinutesSinceStartOfDay(timeText: string): number | null {
  if (!TIME_TEXT_PATTERN.test(timeText)) {
    return null;
  }

  const [hourText, minuteText] = timeText.split(":");
  return Number(hourText) * 60 + Number(minuteText);
}

function daySlotCrossesMidnight(daySlot: DayAutoRunSlot): boolean {
  const startMinutesSinceStartOfDay = parseTimeToMinutesSinceStartOfDay(
    daySlot.startTime,
  );
  const endMinutesSinceStartOfDay = parseTimeToMinutesSinceStartOfDay(
    daySlot.endTime,
  );
  if (
    startMinutesSinceStartOfDay === null ||
    endMinutesSinceStartOfDay === null
  ) {
    return false;
  }

  return startMinutesSinceStartOfDay > endMinutesSinceStartOfDay;
}

interface TimeInputInvalidity {
  start: boolean;
  end: boolean;
}

const TIME_INPUT_INVALIDITY_NONE = {
  start: false,
  end: false,
} as const satisfies TimeInputInvalidity;

const TIME_INPUT_INVALIDITY_BY_ERROR_CODE = {
  start_time_format_invalid: { start: true, end: false },
  end_time_format_invalid: { start: false, end: true },
  start_time_equals_end_time: { start: true, end: true },
} as const satisfies Record<
  DayScheduleValidationError["code"],
  TimeInputInvalidity
>;

interface TimeInputAccessibilityAttributes {
  "aria-invalid"?: true;
  "aria-describedby"?: string;
}

function getTimeInputAccessibilityAttributes(
  isInvalid: boolean,
  dayErrorId: string,
): TimeInputAccessibilityAttributes {
  if (!isInvalid) {
    return {};
  }

  return {
    "aria-invalid": true,
    "aria-describedby": dayErrorId,
  };
}

function getTimeInputAccessibilityAttributesBySlot(
  dayValidationError: DayScheduleValidationError | undefined,
  dayErrorId: string,
): Record<"start" | "end", TimeInputAccessibilityAttributes> {
  const timeInputInvalidity =
    dayValidationError === undefined
      ? TIME_INPUT_INVALIDITY_NONE
      : TIME_INPUT_INVALIDITY_BY_ERROR_CODE[dayValidationError.code];

  return {
    start: getTimeInputAccessibilityAttributes(
      timeInputInvalidity.start,
      dayErrorId,
    ),
    end: getTimeInputAccessibilityAttributes(
      timeInputInvalidity.end,
      dayErrorId,
    ),
  };
}

export function AutoRunDayRow({
  weekday,
  autoRunFeatureEnabled,
  daySlot,
  dayValidationError,
  onEnabledChange,
  onStartTimeChange,
  onEndTimeChange,
}: AutoRunDayRowProps): React.ReactElement {
  const generatedElementId = useId();
  const daySlotShowsOvernightLabel =
    daySlot.enabled && daySlotCrossesMidnight(daySlot);
  const dayTimeInputIsDisabled = !autoRunFeatureEnabled || !daySlot.enabled;
  const dayTimeInputClassName = `rounded border px-2 py-1 text-sm transition-colors ${
    dayTimeInputIsDisabled
      ? "cursor-not-allowed border-disabled-border bg-disabled-bg text-disabled-text"
      : "border-border bg-surface-raised text-on-surface"
  }`;
  const dayErrorId = `${generatedElementId}-error`;
  const timeInputAccessibilityAttributesBySlot =
    getTimeInputAccessibilityAttributesBySlot(dayValidationError, dayErrorId);

  return (
    <div
      className={`rounded-md border px-2 py-2 transition-colors ${
        dayTimeInputIsDisabled
          ? "border-border bg-surface"
          : "border-border bg-surface-raised"
      }`}
      data-testid={`settings-auto-run-day-row-${weekday}`}
    >
      <div className="grid grid-cols-[3rem_1fr_1fr_auto] items-center gap-2">
        <label className="inline-flex items-center gap-1 text-xs text-text">
          <input
            type="checkbox"
            checked={daySlot.enabled}
            disabled={!autoRunFeatureEnabled}
            onChange={(event) => {
              onEnabledChange(event.target.checked);
            }}
            data-testid={`settings-auto-run-day-enabled-${weekday}`}
            aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の自動実行を有効にする`}
          />
          <span>{WEEKDAY_LABELS_BY_KEY[weekday].short}</span>
        </label>

        <input
          type="time"
          step={60}
          value={daySlot.startTime}
          onChange={(event) => {
            onStartTimeChange(event.target.value);
          }}
          disabled={dayTimeInputIsDisabled}
          className={dayTimeInputClassName}
          data-testid={`settings-auto-run-day-start-${weekday}`}
          aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の開始時刻`}
          {...timeInputAccessibilityAttributesBySlot.start}
        />

        <input
          type="time"
          step={60}
          value={daySlot.endTime}
          onChange={(event) => {
            onEndTimeChange(event.target.value);
          }}
          disabled={dayTimeInputIsDisabled}
          className={dayTimeInputClassName}
          data-testid={`settings-auto-run-day-end-${weekday}`}
          aria-label={`${WEEKDAY_LABELS_BY_KEY[weekday].full}の終了時刻`}
          {...timeInputAccessibilityAttributesBySlot.end}
        />

        <span
          className={`text-xs ${
            daySlotShowsOvernightLabel ? "text-primary-500" : "text-transparent"
          }`}
          data-testid={`settings-auto-run-day-overnight-${weekday}`}
        >
          翌日まで
        </span>
      </div>
      {dayValidationError && (
        <p
          id={dayErrorId}
          className="mt-1 text-xs text-danger-300"
          data-testid={`settings-auto-run-day-error-${weekday}`}
          role="alert"
        >
          {dayValidationError.message}
        </p>
      )}
    </div>
  );
}
